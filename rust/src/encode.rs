// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The encoder: the main loop of specification section 6.4 and the candidate
//! scan of section 11.1.
//!
//! Every candidate is priced in characters against what block mode would have
//! charged for the same bytes from the same pending-bit state, and the
//! cheapest wins. Nothing here is a threshold: the tables in section 11.1 are
//! what the comparison happens to produce, not an input to it.

use crate::error::Result;
pub use crate::symbols::*;
use crate::tables::tuning;
use crate::tables::*;

/// What the scan found at one position, priced.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    class: u16,
    /// Bytes consumed.
    len: usize,
    /// Characters, flush field included.
    cost: usize,
    /// For `RUN`, the repeated byte. For `PT`, the mask and the profile.
    a: u32,
    b: u32,
}

/// The encoder state a chunk boundary has to agree on. Two encoders that hold
/// the same state at the same input offset write the same characters from
/// there on, which is what lets a parallel join splice (section 14.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    acc: Acc,
    binary_run: usize,
}

impl State {
    /// Whether this is the state a speculative worker assumed at its first
    /// byte: nothing owed, and a segment allowed to open immediately.
    #[inline]
    pub fn starts_a_chunk(&self) -> bool {
        self.acc.n == 0 && self.binary_run >= tuning::binary_run()
    }
}

/// Encoder state carried across the main loop.
pub struct Encoder {
    pub out: Vec<u8>,
    acc: Acc,
    binary_run: usize,
    /// Where the last segment ended, for the parallel seam of section 14.5.
    pub last_segment_end: usize,
    /// Characters produced up to that point.
    pub chars_at_last_segment: usize,
    /// Every (input offset, character offset) at which a segment closed, when
    /// recording is on. These are the points another encoder can join at.
    pub segment_ends: Vec<(usize, usize)>,
    record: bool,
}

impl Encoder {
    pub fn new() -> Self {
        Self {
            out: Vec::new(),
            acc: Acc::new(),
            binary_run: usize::MAX / 2,
            last_segment_end: 0,
            chars_at_last_segment: 0,
            segment_ends: Vec::new(),
            record: false,
        }
    }

    /// Record where segments close, for a parallel worker.
    pub fn recording(mut self) -> Self {
        self.record = true;
        self
    }

    #[inline]
    pub fn state(&self) -> State {
        State { acc: self.acc, binary_run: self.binary_run }
    }

    #[inline]
    pub fn set_state(&mut self, s: State) {
        self.acc = s.acc;
        self.binary_run = s.binary_run;
    }

    /// Emit the final group into a caller's buffer rather than `self.out`.
    pub fn finish_into(&mut self, out: &mut Vec<u8>) {
        let mut tail = Vec::new();
        self.acc.finish(&mut tail);
        out.extend_from_slice(&tail);
    }

    #[inline]
    fn flush_for_segment(&mut self) {
        let n_enc = self.acc.n;
        if n_enc == 0 {
            return;
        }
        let bits = self.acc.pending();
        if n_enc <= 6 {
            self.out.push(ALPHABET[bits as usize]);
        } else {
            put_pair(bits as u16, &mut self.out);
        }
    }

    /// The signal pair: class and the flush flag, specification section 7.1.
    #[inline]
    fn put_signal(&mut self, class: u16) {
        let hi = if self.acc.n >= 8 { 1u16 } else { 0 };
        put_pair(SIGNAL_MIN + 2 * class + hi, &mut self.out);
    }

    fn open_segment(&mut self, class: u16) {
        self.put_signal(class);
        self.flush_for_segment();
        self.acc.reset();
    }
}

/// Encode `data` as a complete stream.
pub fn encode(data: &[u8]) -> String {
    let mut enc = Encoder::new();
    // The block coder is the ceiling (section 11.2), so this is enough for any
    // input and the character pushes below never check capacity again. Without
    // it the encoder spends more time growing its buffer than scanning.
    enc.out.reserve(2 * (8 * data.len() + 12) / 13 + 2);
    encode_into(&mut enc, data, true);
    // Every byte emitted is an alphabet character, so this is ASCII by
    // construction; the check is one pass and only in debug builds.
    debug_assert!(enc.out.iter().all(|&b| VALUE_OF[b as usize] != 0xFF));
    unsafe { String::from_utf8_unchecked(enc.out) }
}

/// Encode into an existing encoder. `final_flush` is false for a worker of a
/// parallel encode, which must end on a group boundary rather than flush.
pub fn encode_into(enc: &mut Encoder, data: &[u8], final_flush: bool) {
    encode_region(enc, data, 0, data.len());
    if final_flush {
        enc.acc.finish(&mut enc.out);
    }
}

/// Encode from `start`, taking decisions with the whole of `data` in view but
/// committing nothing that begins at or after `commit_until`. Returns the
/// offset actually reached, which is at or past `commit_until` because the
/// last item committed may run over it.
///
/// The lookahead is what makes a parallel encode exact (section 14.5): a
/// worker whose view stopped at its own boundary would decline segments a
/// serial encoder takes, and its output could then never be spliced. Reading
/// past the boundary costs nothing -- the input is shared and read-only -- and
/// the commit limit is what keeps the pieces from overlapping.
pub fn encode_region(enc: &mut Encoder, data: &[u8], start: usize, commit_until: usize) -> usize {
    encode_region_until(enc, data, start, commit_until, &[]).0
}

/// The same, stopping early at the first offset in `resync` that this encoder
/// reaches with a segment boundary.
///
/// Two encoders that have both just closed a segment ending at the same input
/// offset are in the same state -- nothing owed, and a segment allowed to open
/// again -- whatever they did before that. That is what bounds the repair a
/// parallel join has to do: not a chunk, but the distance to the first place
/// the two paths provably agree.
pub fn encode_region_until(
    enc: &mut Encoder,
    data: &[u8],
    start: usize,
    commit_until: usize,
    resync: &[(usize, usize)],
) -> (usize, Option<usize>) {
    // The sweep knobs are read once per call, not per byte: a relaxed load is
    // cheap but it is opaque to the optimiser, and reading it in the hot loop
    // was costing more than the scan it gates.
    let min_binary_run = tuning::binary_run();
    let mut i = start;
    // The window whose verdict is in `block_window`, so the entropy of a
    // window is computed once rather than per byte.
    let mut verdict_for = usize::MAX;
    let mut window_is_block = false;
    while i < commit_until {
        // Decide per window whether anything is worth looking for at all.
        // Aligned to absolute offsets so that a parallel worker and the
        // sequential pass reach the same verdict for the same bytes.
        if tuning::detect_enabled() {
            let w = i / crate::detect::WINDOW;
            if w != verdict_for {
                verdict_for = w;
                let from = w * crate::detect::WINDOW;
                let to = (from + crate::detect::WINDOW).min(data.len());
                window_is_block = crate::detect::is_block(&data[from..to], from == 0);
            }
            if window_is_block {
                let end = (((i / crate::detect::WINDOW) + 1) * crate::detect::WINDOW)
                    .min(commit_until);
                block_bulk(&mut enc.acc, &mut enc.out, &data[i..end]);
                enc.binary_run += end - i;
                i = end;
                continue;
            }
        }
        // Ask once for a whole window whether anything can start in it. On a
        // compressed payload the answer is always no, and the scan below --
        // which is five sixths of the encoder's time on such input -- is
        // skipped for twenty-two positions at a time.
        // Ask once for a whole window which of its positions could open a
        // segment at all, and put the rest straight through block mode. On a
        // compressed payload the answer is "none of them", window after
        // window, and the scan below -- five sixths of the encoder's time on
        // such input -- is never entered.
        #[cfg(feature = "simd")]
        {
            let step = crate::simd::LANES - crate::simd::MARGIN;
            let margin = !0u32 << step;
            let mut end = i;
            // Windows are walked while they hold nothing, so a compressed
            // stretch of any length is one bulk call rather than one per
            // window. The walk stops at the first position a window does
            // report, and the scan below takes it from there.
            while end + crate::simd::LANES + 1 <= data.len() {
                let live = crate::simd::candidate_mask(data, end) & !margin;
                if live != 0 {
                    end += live.trailing_zeros() as usize;
                    break;
                }
                end += step;
            }
            let end = end.min(commit_until);
            if end > i {
                block_bulk(&mut enc.acc, &mut enc.out, &data[i..end]);
                enc.binary_run += end - i;
                i = end;
                continue;
            }
        }
        let cand = if enc.binary_run >= min_binary_run {
            scan(data, i, enc.acc.n)
        } else {
            None
        };
        match cand {
            Some(c) => {
                emit(enc, data, i, &c);
                i += c.len;
                enc.binary_run = 0;
                enc.last_segment_end = i;
                enc.chars_at_last_segment = enc.out.len();
                if enc.record {
                    enc.segment_ends.push((i, enc.out.len()));
                }
                if let Some(k) = resync.binary_search_by_key(&i, |r| r.0).ok() {
                    return (i, Some(k));
                }
            }
            None => {
                enc.acc.push(data[i] as u32, 8, &mut enc.out);
                enc.binary_run += 1;
                i += 1;
            }
        }
    }
    (i, None)
}

/// The block coder with the scan switched off: what encoding costs when no
/// class can carry anything, which is what a compressed payload looks like.
pub fn block_only(data: &[u8]) -> String {
    let mut acc = Acc::new();
    let mut out = Vec::with_capacity(2 * (8 * data.len() + 12) / 13 + 2);
    block_bulk(&mut acc, &mut out, data);
    acc.finish(&mut out);
    unsafe { String::from_utf8_unchecked(out) }
}

// ---------------------------------------------------------------------------
// The candidate scan
// ---------------------------------------------------------------------------

/// What block mode really charges for `len` bytes from `n` pending bits, and
/// what a segment covering the same bytes charges, both in thirteenths of a
/// character so that they can be compared exactly.
///
/// The subtlety is the bits block mode leaves behind. It emits only whole
/// symbols, so counting the characters it writes *understates* it: the
/// remainder is input it has consumed and not yet paid for, and it will pay
/// later. A segment leaves nothing pending, because Section 7.2 flushes.
/// Comparing written characters against written characters therefore favours
/// block mode by up to two characters, and on a short payload two characters
/// is the whole decision -- six digits went to block mode at eight characters
/// where `DEC` would have taken seven.
///
/// At the end of the input there is no "later", so the comparison is exact
/// there: block mode pays its final group and the segment pays nothing.
#[inline]
fn weigh(seg_chars: usize, len: usize, n: u32, at_end: bool) -> (usize, usize) {
    let bits = 8 * len as u64 + n as u64;
    if at_end {
        let whole = 2 * (bits / 13) as usize;
        let block = whole + flush_chars((bits % 13) as u32);
        (13 * seg_chars, 13 * block)
    } else {
        (13 * seg_chars, 2 * bits as usize)
    }
}

/// The cheapest segment that can open at `at`, or none if block mode wins.
fn scan(data: &[u8], at: usize, n: u32) -> Option<Candidate> {
    let families = tuning::families();
    let overhead = 2 + flush_chars(n); // signal, flush
    let mut best: Option<Candidate> = None;
    let total = data.len();

    let mut consider = |c: Candidate| {
        let (seg, blk) = weigh(c.cost, c.len, n, at + c.len == total);
        if seg >= blk {
            return;
        }
        // Ranked by what a candidate saves against block mode, then the lower
        // class, then the longer prefix: canonicity rules 1, 2 and 3 of
        // section 11.3.
        //
        // This is a greedy rule and it is not the best possible one. Ranking
        // by saving *per byte consumed* instead was tried, on the argument
        // that comparing candidates of different lengths by total saving
        // favours the longer -- a JWT is three base64url runs separated by
        // dots, passthrough reaches all of it and a packed base only the
        // first run. It is worse: 0.98013 against 0.97831 on the core corpus
        // and 0.9261 against 0.9252 on the short one, and the JWT itself goes
        // from 1.032 to 1.039. Neither criterion dominates, which is what
        // says the question is not the criterion but the greediness.
        let better = match &best {
            None => true,
            Some(b) => {
                let (bseg, bblk) = weigh(b.cost, b.len, n, at + b.len == total);
                let (gain, bgain) = (blk - seg, bblk - bseg);
                (gain, b.class, c.len) > (bgain, c.class, b.len)
            }
        };
        if better {
            best = Some(c);
        }
    };

    // --- runs, and runs with gaps -----------------------------------------
    let run = if families & tuning::F_RUN != 0 { run_length(data, at) } else { 0 };
    if run >= 2 {
        let capped = run.min(MAX_SEGMENT_BYTES);
        if data[at] == 0 {
            consider(Candidate {
                class: CLASS_ZRUN,
                len: capped,
                cost: overhead + length_chars(capped),
                a: 0,
                b: 0,
            });
        } else {
            consider(Candidate {
                class: CLASS_RUN,
                len: capped,
                cost: overhead + length_chars(capped) + 2,
                a: data[at] as u32,
                b: 0,
            });
        }
    }

    // --- packed bases ------------------------------------------------------
    let mut live = if families & tuning::F_PACKED != 0 {
        PACKED_MEMBERSHIP[data[at] as usize] & tuning::packed_mask()
    } else {
        0
    };
    if live != 0 {
        // One pass over the input, narrowing the set of classes still alive,
        // records where each class had to stop.
        let mut end = [at; 10];
        let mut j = at;
        let mut run = 1usize;
        let limit = data.len().min(at + MAX_SEGMENT_BYTES);
        while j < limit && live != 0 {
            if j > at {
                run = if data[j] == data[j - 1] { run + 1 } else { 1 };
                if run >= tuning::run_break(data[j] == 0) {
                    j -= run - 1;
                    break;
                }
            }
            let m = PACKED_MEMBERSHIP[data[j] as usize] & tuning::packed_mask();
            let dead = live & !m;
            let mut d = dead;
            while d != 0 {
                let c = d.trailing_zeros() as usize;
                end[c] = j;
                d &= d - 1;
            }
            live &= m;
            j += 1;
        }
        let mut d = live;
        while d != 0 {
            let c = d.trailing_zeros() as usize;
            end[c] = j;
            d &= d - 1;
        }
        for c in 0..10 {
            let len = end[c] - at;
            if len < 2 {
                continue;
            }
            let w = PACKED[c].w;
            consider(Candidate {
                class: CLASS_PACKED_FIRST + c as u16,
                len,
                cost: overhead + length_chars(len) + packed_chars(len, w),
                a: 0,
                b: 0,
            });
        }
    }

    // --- passthrough -------------------------------------------------------
    if families & tuning::F_PT != 0 {
    if let Some(c) = scan_passthrough(data, at, overhead) {
        consider(c);
    }
    }

    best
}

#[inline]
fn run_length(data: &[u8], at: usize) -> usize {
    let b = data[at];
    #[cfg(feature = "simd")]
    let mut j = crate::simd::run_end(data, at);
    #[cfg(not(feature = "simd"))]
    let mut j = at + 1;
    while j < data.len() && data[j] == b {
        j += 1;
    }
    j - at
}

/// The passthrough prefix scan of section 11.1: how far one segment reaches,
/// and which mask and profile describe it.
fn scan_passthrough(data: &[u8], at: usize, overhead: usize) -> Option<Candidate> {
    let limit = data.len().min(at + MAX_SEGMENT_BYTES);
    // A segment of one byte never pays, and on binary input the scan fails
    // here at almost every position -- before any donor bookkeeping.
    if !PT_CARRIABLE[data[at] as usize] || at + 1 >= limit || !PT_CARRIABLE[data[at + 1] as usize] {
        return None;
    }
    let mut mask: u8 = 0;
    let mut k: u8 = 0;
    // Per profile, the lowest donor rank any literal in the segment holds.
    let mut min_rank = [8u8; NUM_PROFILES];
    let mut j = at;
    let mut profile = 0usize;

    // How long the run ending at j-1 is, so the scan can hand a long one to
    // the run classes rather than carrying it at one character per byte.
    let mut run = 1usize;
    while j < limit {
        let byte = data[j];
        if j > at {
            run = if byte == data[j - 1] { run + 1 } else { 1 };
            if run >= tuning::run_break(byte == 0) {
                j -= run - 1;
                break;
            }
        }
        let (new_mask, new_k, mut new_min) = {
            let r = R_INDEX[byte as usize];
            if r != 0xFF {
                let bit = 1u8 << r;
                let nk = k + u8::from(mask & bit == 0);
                (mask | bit, nk, min_rank)
            } else if VALUE_OF[byte as usize] != 0xFF {
                let mut m = min_rank;
                for p in 0..NUM_PROFILES {
                    let rank = DONOR_RANK[p][byte as usize];
                    if rank < m[p] {
                        m[p] = rank;
                    }
                }
                (mask, k, m)
            } else {
                break; // not representable at all
            }
        };
        // The smallest profile that still has enough donors above every
        // literal already committed.
        let mut viable = None;
        for p in 0..NUM_PROFILES {
            if new_min[p] >= new_k {
                viable = Some(p);
                break;
            }
        }
        let Some(p) = viable else { break };
        mask = new_mask;
        k = new_k;
        std::mem::swap(&mut min_rank, &mut new_min);
        profile = p;
        j += 1;
    }

    let len = j - at;
    if len < 2 {
        return None;
    }
    // A shorthand saves the parameter pair where it applies (rule 6).
    let shorthand = if profile == 0 {
        SHORTHAND_MASK.iter().position(|&m| m == mask).map(|i| i as u16 + 1)
    } else {
        None
    };
    let (class, params) = match shorthand {
        Some(c) => (c, 0),
        None => (CLASS_PT, 2),
    };
    Some(Candidate {
        class,
        len,
        cost: overhead + params + length_chars(len) + len,
        a: mask as u32,
        b: profile as u32,
    })
}

// ---------------------------------------------------------------------------
// Emitting
// ---------------------------------------------------------------------------

fn emit(enc: &mut Encoder, data: &[u8], at: usize, c: &Candidate) {
    enc.open_segment(c.class);
    match c.class {
        CLASS_ZRUN => {
            put_length(c.len, &mut enc.out);
        }
        CLASS_RUN => {
            put_length(c.len, &mut enc.out);
            put_pair(c.a as u16, &mut enc.out);
        }
        CLASS_PACKED_FIRST..=CLASS_PACKED_LAST => {
            let ci = (c.class - CLASS_PACKED_FIRST) as usize;
            let w = PACKED[ci].w;
            put_length(c.len, &mut enc.out);
            let mut a = Acc::new();
            for &b in &data[at..at + c.len] {
                a.push(PACKED_INDEX[ci][b as usize] as u32, w, &mut enc.out);
            }
            a.finish_padded(&mut enc.out);
        }
        _ => {
            // Passthrough: class 0 carries mask and profile, 1..=6 imply them.
            if c.class == CLASS_PT {
                put_pair((c.a + 256 * c.b) as u16, &mut enc.out);
            }
            put_length(c.len, &mut enc.out);
            let donors = donor_table(c.a as u8, c.b as usize);
            for &b in &data[at..at + c.len] {
                let r = R_INDEX[b as usize];
                enc.out.push(if r != 0xFF && (c.a as u8) & (1 << r) != 0 {
                    donors[r as usize]
                } else {
                    b
                });
            }
        }
    }
}

/// Which alphabet character stands in for each set bit of `mask`.
pub fn donor_table(mask: u8, profile: usize) -> [u8; R_LEN] {
    let mut t = [0u8; R_LEN];
    let mut rank = 0usize;
    for j in 0..R_LEN {
        if mask & (1 << j) != 0 {
            t[j] = PROFILES[profile][rank];
            rank += 1;
        }
    }
    t
}

impl Acc {
    /// A packed payload pads its last symbol with zero bits rather than
    /// emitting a short final group: specification section 9.
    pub fn finish_padded(&mut self, out: &mut Vec<u8>) {
        if self.n > 0 {
            let width = SYMBOL_BITS - self.n;
            self.push(0, width, out);
        }
        self.reset();
    }
}

pub fn _unused(_: Result<()>) {}
