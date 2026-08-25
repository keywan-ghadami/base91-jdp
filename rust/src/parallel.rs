// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Encoding on several threads, producing exactly what one thread produces.
//!
//! Specification section 14.5 is the argument, and it rests on one property of
//! the fixed thirteen-bit symbol: thirteen bytes are 104 bits are eight
//! symbols, so block mode returns to an empty accumulator at every thirteenth
//! byte and nowhere else. Cut the input there and a worker needs to know
//! nothing about the worker before it -- provided the worker before it really
//! did end with an empty accumulator.
//!
//! Segments are what can break that: a segment consumes bytes without passing
//! them through the accumulator, so the block-mode bytes after the last
//! segment in a chunk are what have to be a multiple of thirteen, not the
//! chunk. This encoder therefore **verifies rather than assumes**. Each worker
//! encodes its chunk speculatively from an empty state and records the state
//! it ended in; the join walks the chunks in order carrying the true state,
//! splices a worker's output only where its assumption provably held, and
//! re-encodes the chunk sequentially where it did not.
//!
//! The result is byte-identical to [`crate::encode`] whatever the thread count
//! and whatever the chunking, because a splice happens only when the two paths
//! were in the same state. [`ParallelStats`] reports how often that was true,
//! which is the number that says whether the arrangement is worth anything on
//! a given kind of input.

use std::thread;

use crate::encode::{encode, encode_region, encode_region_until, Encoder, State};
use crate::tables::PARALLEL_ALIGN;

/// Below this a chunk is not worth a thread.
pub const MIN_PARALLEL_CHUNK: usize = 1 << 18;

/// What the join had to do. `spliced + repaired` is the chunk count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParallelStats {
    /// Chunks whose speculative output was used as it stood.
    pub spliced: usize,
    /// Chunks the join had to encode again, sequentially.
    pub repaired: usize,
    /// Bytes the join had to encode again.
    pub repaired_bytes: usize,
    /// Repairs that met the worker again at a shared segment boundary, so only
    /// part of the chunk had to be redone.
    pub rejoined: usize,
}

/// Encode on up to `threads` threads. Identical output to [`encode`].
pub fn encode_parallel(data: &[u8], threads: usize) -> String {
    encode_parallel_stats(data, threads).0
}

/// The same, and what the join had to do to get there.
pub fn encode_parallel_stats(data: &[u8], threads: usize) -> (String, ParallelStats) {
    let threads = threads.max(1);
    if threads == 1 || data.len() < 2 * MIN_PARALLEL_CHUNK {
        return (encode(data), ParallelStats::default());
    }
    let chunk = ((data.len() / threads).max(MIN_PARALLEL_CHUNK) / PARALLEL_ALIGN) * PARALLEL_ALIGN;
    encode_with_chunk_stats(data, chunk)
}

/// The same with the chunking handed in, so a test can reach the seam with
/// small chunks instead of megabyte ones.
pub fn encode_with_chunk(data: &[u8], chunk: usize) -> String {
    encode_with_chunk_stats(data, chunk).0
}

pub fn encode_with_chunk_stats(data: &[u8], chunk: usize) -> (String, ParallelStats) {
    assert!(chunk > 0 && chunk % PARALLEL_ALIGN == 0, "chunks are whole symbol groups");
    if data.is_empty() {
        return (String::new(), ParallelStats::default());
    }

    let bounds: Vec<(usize, usize)> = (0..data.len())
        .step_by(chunk)
        .map(|s| (s, (s + chunk).min(data.len())))
        .collect();

    // Each worker encodes its chunk as though it were a stream of its own: an
    // empty accumulator, and a binary run long enough that a segment may open
    // at its first byte. Both are what the join has to confirm before the
    // output can be used. Its decisions are taken with all of `data` in view,
    // so they are the decisions a serial encoder takes; only where it stops
    // committing is its own.
    let parts: Vec<Part> = thread::scope(|scope| {
        let handles: Vec<_> = bounds
            .iter()
            .map(|&(from, to)| scope.spawn(move || worker(data, from, to)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut out: Vec<u8> = Vec::with_capacity(data.len() * 5 / 4);
    let mut stats = ParallelStats::default();
    let mut enc = Encoder::new();

    let mut settled = 0usize;
    for (i, part) in parts.into_iter().enumerate() {
        let from = bounds[i].0;
        if settled >= part.commit_until {
            // An earlier piece's last item ran past this whole chunk.
            continue;
        }
        // A splice is only sound where the join arrived exactly where the
        // worker started. An earlier item that ran over the boundary leaves
        // `settled` inside this chunk, and then the rest of it is repair work
        // -- not a chunk to skip, which would drop the bytes in between.
        if settled == from && enc.state().starts_a_chunk() {
            // The worker began in the state the join actually arrived in, so
            // its characters are the ones a serial encode would have written.
            out.extend_from_slice(&part.chars);
            enc.set_state(part.end);
            settled = part.end_input;
            stats.spliced += 1;
        } else {
            // Repair only as far as the first point where this encoder closes
            // a segment that the worker also closed. From there the two are in
            // the same state and the worker's remaining characters stand.
            enc.out.clear();
            let was = settled;
            let (reached, joined) =
                encode_region_until(&mut enc, data, settled, part.commit_until, &part.segment_ends);
            out.extend_from_slice(&enc.out);
            settled = reached;
            stats.repaired += 1;
            stats.repaired_bytes += settled - was;
            if let Some(k) = joined {
                out.extend_from_slice(&part.chars[part.segment_ends[k].1..]);
                enc.set_state(part.end);
                settled = part.end_input;
                stats.rejoined += 1;
            }
        }
    }
    if settled < data.len() {
        enc.out.clear();
        encode_region(&mut enc, data, settled, data.len());
        out.extend_from_slice(&enc.out);
    }
    enc.finish_into(&mut out);
    (unsafe { String::from_utf8_unchecked(out) }, stats)
}

struct Part {
    chars: Vec<u8>,
    end: State,
    /// Where this worker stopped committing, which is at or past its nominal
    /// boundary: the last item it took may run over.
    end_input: usize,
    commit_until: usize,
    segment_ends: Vec<(usize, usize)>,
}

fn worker(data: &[u8], from: usize, to: usize) -> Part {
    let mut enc = Encoder::new().recording();
    enc.out.reserve(2 * (8 * (to - from) + 12) / 13 + 2);
    let end_input = encode_region(&mut enc, data, from, to);
    Part {
        end: enc.state(),
        end_input,
        commit_until: to,
        segment_ends: std::mem::take(&mut enc.segment_ends),
        chars: std::mem::take(&mut enc.out),
    }
}
