// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Class 20, the compressed segment: specification section 10.1.
//!
//! The payload is one zstd frame, packed at `w = 8` through the block coder.
//! The format specifies nothing about the frame -- not its length, which the
//! segment's own length field carries, and not its checksum, which is zstd's.
//!
//! Two things here are decisions rather than transcription.
//!
//! **How much payload goes in one frame.** The specification leaves it to the
//! encoder and section 17.9 prices it: one frame over the whole input is the
//! smallest, 1 MiB per frame costs 0.2 %, and 64 KiB costs 6.3 %. A megabyte
//! is the default because it bounds what one damaged frame destroys and what
//! a decoder must hold, for a fifth of a percent.
//!
//! **Whether to compress at all.** Section 11.2 says the compressed candidate
//! is taken only if it beats what the scan and block mode produce together,
//! and that means building both. [`encode_auto`] does; [`encode_zstd`] does
//! not, and exists so that the compression path can be measured on its own.

use crate::encode::encode;
use crate::symbols::{block_bulk, length_chars, put_length};
use crate::tables::{CLASS_ZSTD, SIGNAL_MIN};

/// Payload per frame. Section 17.9 is why it is not the whole input.
pub const FRAME_PAYLOAD: usize = 1 << 20;

/// Below this, both candidates are built rather than one chosen. Building
/// both is cheap on a small input and the entropy sample is unreliable there;
/// above it the comparison is what costs. The crossover where a frame starts
/// to win at all is around a hundred bytes on real payloads, so this is
/// generous by a factor of forty.
pub const COMPARE_BELOW: usize = 4096;

/// A conservative ceiling on what one frame may expand to on decode, used
/// where the frame declares no content size. Specification section 16: the
/// expansion is attacker-controlled and the ceiling belongs on the total.
pub const DEFAULT_EXPANSION_LIMIT: usize = 1 << 30;

thread_local! {
    /// One frame buffer per thread, kept between calls.
    ///
    /// `zstd::bulk::Compressor::compress` allocates `compress_bound(len)` on
    /// every call and hands the `Vec` back; at a mebibyte of payload that is a
    /// megabyte-and-change allocation per segment, and at forty bytes it is a
    /// malloc per field. `compress_to_buffer` writes into a buffer the caller
    /// owns, so one buffer serves every segment of every call on this thread.
    static FRAME: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };

    /// One compressor per thread, kept between calls.
    ///
    /// `zstd::bulk::Compressor` holds the context zstd would otherwise
    /// allocate and initialise per frame, and on a field-sized payload that
    /// setup is the entire cost: building one per call put the short corpus at
    /// 2 MB/s, where the frames themselves are a few dozen bytes each. Keyed
    /// by level, because changing the level means a new context anyway.
    static COMPRESSOR: std::cell::RefCell<Option<(i32, zstd::bulk::Compressor<'static>)>> =
        const { std::cell::RefCell::new(None) };
}

fn with_compressor<T>(
    level: i32,
    f: impl FnOnce(&mut zstd::bulk::Compressor<'static>) -> std::io::Result<T>,
) -> std::io::Result<T> {
    COMPRESSOR.with(|cell| {
        let mut slot = cell.borrow_mut();
        let reuse = matches!(&*slot, Some((l, _)) if *l == level);
        if !reuse {
            *slot = Some((level, zstd::bulk::Compressor::new(level)?));
        }
        f(&mut slot.as_mut().unwrap().1)
    })
}

/// Encode as compressed segments, without weighing them against anything.
///
/// The frame is compressed into a buffer this thread keeps and then packed
/// out of it. There is no way to remove that buffer entirely: the length field
/// of Section 7.3 precedes the payload and its width depends on the length, so
/// nothing can be written until the frame is finished. What can be removed is
/// the *allocation*, and this does.
pub fn encode_zstd(data: &[u8], level: i32) -> std::io::Result<String> {
    let mut out: Vec<u8> = Vec::with_capacity(2 * (8 * data.len() + 12) / 13 + 64);
    for chunk in data.chunks(FRAME_PAYLOAD) {
        FRAME.with(|cell| -> std::io::Result<()> {
            let mut frame = cell.borrow_mut();
            frame.clear();
            frame.reserve(zstd::zstd_safe::compress_bound(chunk.len()));
            let n = with_compressor(level, |c| c.compress_to_buffer(chunk, &mut *frame))?;
            debug_assert_eq!(n, frame.len());

            // The signal, with an empty flush field: block mode is at a group
            // boundary before the first segment and after every one.
            crate::symbols::put_pair(SIGNAL_MIN + 2 * CLASS_ZSTD, &mut out);
            put_length(frame.len(), &mut out);
            // A payload pads its last symbol with zero bits rather than
            // emitting a short final group: specification section 9, and the
            // decoder computes the character count from the length field
            // before reading any of them. Closing with the final-group rule
            // instead makes a stream whose frame is one character short.
            let mut acc = crate::symbols::Acc::new();
            block_bulk(&mut acc, &mut out, &frame);
            acc.finish_padded(&mut out);
            Ok(())
        })?;
    }
    Ok(unsafe { String::from_utf8_unchecked(out) })
}

/// Build both candidates and return the shorter, which is what section 11.2
/// requires of a conforming encoder.
pub fn encode_auto(data: &[u8], level: i32) -> std::io::Result<String> {
    let plain = encode(data);
    let compressed = encode_zstd(data, level)?;
    Ok(if compressed.len() < plain.len() { compressed } else { plain })
}

/// Decide whether to compress from the same entropy sample that decides
/// whether to scan, and build only the candidate that decision names.
///
/// [`encode_auto`] is what section 11.2 asks for and it costs an order of
/// magnitude, because building the uncompressed candidate means running the
/// scan over data the scan has plenty to find in. The signal that says the
/// scan will be expensive is the same one that says compression will pay:
/// low entropy. One histogram over a kilobyte answers both.
///
/// The threshold is [`crate::detect::ENTROPY_BITS`], unchanged. On the core
/// corpus it agrees with `encode_auto` on all thirteen files.
pub fn encode_smart(data: &[u8], level: i32) -> std::io::Result<String> {
    if data.len() < COMPARE_BELOW {
        // Small enough that the comparison of section 11.2 is free, and small
        // enough that the entropy sample would be guessing. Below the
        // crossover a frame header is a large fraction of the payload and
        // compression loses badly -- 1.5250 characters per byte over the short
        // corpus against 0.9252 -- so this is not an optimisation but a
        // correctness fix: an encoder that compresses everything produces
        // output worse than Base64 on a field-sized payload.
        return encode_auto(data, level);
    }
    if crate::detect::is_block(data, true) {
        // Already compressed, or close enough that nothing will come off it.
        // The plain path then costs nothing either: section 11.5 skips the
        // scan for exactly this input.
        return Ok(encode(data));
    }
    encode_zstd(data, level)
}

/// Characters a compressed encode would occupy, without building it. Cheaper
/// than [`encode_zstd`] by the packing, which is what an encoder wanting only
/// the comparison of section 11.2 needs.
pub fn zstd_chars(data: &[u8], level: i32) -> std::io::Result<usize> {
    let mut chars = 0usize;
    for chunk in data.chunks(FRAME_PAYLOAD) {
        let n = FRAME.with(|cell| -> std::io::Result<usize> {
            let mut frame = cell.borrow_mut();
            frame.clear();
            frame.reserve(zstd::zstd_safe::compress_bound(chunk.len()));
            with_compressor(level, |c| c.compress_to_buffer(chunk, &mut *frame))
        })?;
        chars += 2 + length_chars(n) + 2 * ((8 * n + 12) / 13);
    }
    Ok(chars)
}
