// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The `simd` feature: vector fast-forwards for the two loops that dominate
//! encoding, behind `#![feature(portable_simd)]`.
//!
//! **These are accelerations, never decisions.** Each function answers a
//! question the scalar code would have answered the same way, and a
//! conservative answer costs a scalar step and nothing else. The two paths
//! therefore cannot disagree about the encoding, which the round-trip tests
//! assert by running both.
//!
//! What is accelerated:
//!
//! * [`run_end`] -- how far a run of one repeated byte reaches. The run
//!   classes of specification section 10.2 are the measured win of 0.4.0, and
//!   finding a run is a comparison against a splat.
//! [`dead_span`] is the one that pays, and it pays where it matters most: a
//! compressed payload has no runs, no packed alphabet and no passthrough, so
//! the scan of specification section 11.1 is entered once per byte and finds
//! nothing once per byte. Measured on high-entropy input, the block coder
//! alone runs at 323 MB/s and the whole encoder at 31 -- five sixths of the
//! time is a scan that fails.
//!
//! [`SkipSet`] is the same idea applied to the *passthrough* prefix scan, and
//! it is **not wired in, because it was measured and it loses.** Four
//! arrangements were tried against the scalar scan at 30-114 MB/s:
//!
//! | arrangement | result |
//! |---|---|
//! | static set, shuffle emulated with `to_array` | 0-20 MB/s |
//! | static set, real `swizzle_dyn`, repeats scanned separately | 5-56 MB/s |
//! | static set, repeats folded into the same vector step | 6-58 MB/s |
//! | ... and a guard against retrying the probe per byte | 18-87 MB/s |
//! | set rebuilt as the segment's state changes | 2-83 MB/s |
//!
//! The two results are not in tension, they are the same lesson from both
//! sides. A vector probe pays when it settles many bytes per call. Asking "can
//! anything start in the next thirty-two bytes" of a compressed payload
//! settles all thirty-two, every time. Asking "does the next byte change the
//! passthrough state" of English text settles two or three, because the bytes
//! that stop it -- the R-Set members and the donors -- are precisely the
//! frequent characters of the text the scan runs on.
//!
//! What would be worth trying for passthrough is vectorising the *donor
//! bookkeeping* rather than skipping it: the four per-profile minima are four
//! bytes, one `u32` lane each, and `min` across them is one instruction. That
//! is Base85N's section 11.2, and it accelerates the work instead of trying to
//! avoid it.

use std::simd::cmp::SimdPartialEq;
use std::simd::u8x32;


use crate::tables::{DONOR_RANK, NUM_PROFILES, R_INDEX, VALUE_OF};

/// How many bytes one vector step settles.
pub const LANES: usize = 32;
type V = u8x32;

/// The end of the run of `data[at]` that begins at `at`.
#[inline]
pub fn run_end(data: &[u8], at: usize) -> usize {
    let splat = V::splat(data[at]);
    let mut i = at + 1;
    while i + LANES <= data.len() {
        let m = V::from_slice(&data[i..i + LANES]).simd_eq(splat).to_bitmask();
        let all = u64::MAX >> (64 - LANES);
        if m != all {
            return i + m.trailing_ones() as usize;
        }
        i += LANES;
    }
    i
}

/// `1 << n` for the high nibbles a byte below 128 can have, zero for the rest.
const NIBBLE_BITS: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 0, 0, 0, 0, 0, 0, 0, 0];


/// The first position at or after `at`, and before `limit`, where the scan has
/// something to do: a byte that is not "plain" in the sense above, or a byte
/// equal to its predecessor, which the run break of section 11.1 has to see.
///
/// Both questions are one vector step. Answering the second with a scalar pass
/// over the same stretch -- as an earlier draft of this file did -- walks the
/// input twice and is slower than not vectorising at all. So is emulating
/// `swizzle_dyn` with `to_array` and a loop; the shuffle has to be a shuffle.
///
/// `at` must be greater than zero: the predecessor of the first byte compared
/// is `data[at - 1]`.
/// The nibble table duplicated into both halves of a 32-lane vector, because
/// `swizzle_dyn` indexes all thirty-two lanes. Built at compile time where it
/// can be: doing it per call is thirty-two scalar stores in the hot loop and
/// costs more than the shuffle saves.
const fn dup(t: [u8; 16]) -> [u8; LANES] {
    let mut out = [0u8; LANES];
    let mut i = 0;
    while i < LANES {
        out[i] = t[i & 15];
        i += 1;
    }
    out
}

const NIBBLE32: [u8; LANES] = dup(NIBBLE_BITS);

/// The set of bytes that leave the scan's state alone **as it now stands**.
///
/// A static set has to stop at every R-Set member, and R-Set members are a
/// fifth of ordinary text -- so it stops every few bytes and a thirty-two lane
/// step never pays. The set is not static, though: once a segment has
/// accounted for the space character, another space changes nothing and can be
/// skipped, and once a literal has pushed a profile's lowest donor rank down,
/// every byte at or above that rank is harmless too. The set therefore only
/// grows as a segment runs, and on prose it grows to almost everything within
/// the first few bytes.
///
/// Rebuilding is 128 table lookups and happens only when the state actually
/// changes: at most eight times for the mask, plus once per profile.
#[derive(Clone, Copy)]
pub struct SkipSet {
    lo32: [u8; LANES],
}

impl SkipSet {
    pub fn build(mask: u8, min_rank: [u8; NUM_PROFILES], k: u8) -> Self {
        let mut lo = [0u8; 16];
        for b in 0..128u16 {
            let b = b as u8;
            let r = R_INDEX[b as usize];
            let harmless = if r != 0xFF {
                // An R-Set member changes nothing once its bit is already set.
                mask & (1 << r) != 0
            } else if VALUE_OF[b as usize] != 0xFF {
                // A literal changes nothing while it lowers no profile's
                // minimum -- and a byte that is a set bit's donor cannot occur
                // in the segment at all, so it must stop the skip.
                let mut ok = true;
                let mut p = 0;
                while p < NUM_PROFILES {
                    let rank = DONOR_RANK[p][b as usize];
                    if rank < min_rank[p] || rank < k {
                        ok = false;
                        break;
                    }
                    p += 1;
                }
                ok
            } else {
                false
            };
            if harmless {
                lo[(b & 15) as usize] |= 1 << (b >> 4);
            }
        }
        Self { lo32: dup(lo) }
    }

    /// The first position at or after `at` where the scan has something to do:
    /// a byte outside this set, or one equal to its predecessor, which the run
    /// break of section 11.1 has to see.
    ///
    /// Both questions are one vector step. Answering the second with a scalar
    /// pass over the same stretch walks the input twice and is slower than not
    /// vectorising at all; so is emulating `swizzle_dyn` with `to_array`.
    #[inline]
    pub fn end(&self, data: &[u8], at: usize, limit: usize) -> usize {
        debug_assert!(at > 0);
        let lo32 = V::from_array(self.lo32);
        let hi32 = V::from_array(NIBBLE32);
        let all = u64::MAX >> (64 - LANES);
        let mut i = at;
        while i + LANES <= limit {
            let chunk = V::from_slice(&data[i..i + LANES]);
            let prev = V::from_slice(&data[i - 1..i - 1 + LANES]);
            let lo_sel = lo32.swizzle_dyn(chunk & V::splat(0x0F));
            let hi_sel = hi32.swizzle_dyn((chunk >> V::splat(4)) & V::splat(0x0F));
            let miss = (lo_sel & hi_sel).simd_eq(V::splat(0)).to_bitmask();
            let repeat = chunk.simd_eq(prev).to_bitmask();
            let stop = (miss | repeat) & all;
            if stop != 0 {
                return i + stop.trailing_zeros() as usize;
            }
            i += LANES;
        }
        i
    }
}

// ---------------------------------------------------------------------------
// The dead span
// ---------------------------------------------------------------------------

/// One bit per position: could a segment open there at all.
///
/// **This is where the vector work pays**, and it is the case a compressed
/// payload is: no run, no packed class, no passthrough, so the scan of
/// section 11.1 is entered once per byte and fails once per byte. Measured on
/// high-entropy input, the block coder alone runs at 534 MB/s and a scan at
/// every position brings the encoder to 31.
///
/// The mask is the union of three conditions, and it is deliberately weak in
/// every one of them: a set bit only ever means *scan here after all*.
///
/// * **A run needs two equal adjacent bytes.** `ZRUN` pays from two bytes up,
///   so any repeat at all sets its position.
/// * **A packed base needs five bytes of one class** -- four costs seven
///   characters where block mode costs six. Runs of four bytes that are in
///   *any* packed class are marked instead, which is weaker and so safe.
/// * **Passthrough needs ten carriable bytes**; nine ties, and a tie goes to
///   block mode by canonicity rule 2. Runs of eight are marked.
///
/// A run of `k` set bits is found by folding the mask into itself: `m & m>>1`
/// leaves the runs of two, and two more steps leave the runs of eight.
///
/// The top [`MARGIN`] bits are set unconditionally, because a run that begins
/// there may continue past what was loaded and this function can only see its
/// own window. The caller therefore advances by `LANES - MARGIN` per mask.
///
/// Returning a mask rather than a yes-or-no is what makes a window with one
/// interesting position cost one probe and one scan, instead of one probe and
/// thirty-two scans. On random bytes a quarter of windows hold something.
pub const MARGIN: usize = 10;

#[inline]
pub fn candidate_mask(data: &[u8], at: usize) -> u32 {
    debug_assert!(at + LANES + 1 <= data.len());
    let chunk = V::from_slice(&data[at..at + LANES]);
    let next = V::from_slice(&data[at + 1..at + 1 + LANES]);

    let repeat = chunk.simd_eq(next).to_bitmask() as u32;
    let carriable = runs_of_8(membership(chunk, &CARRIABLE32)) as u32;
    let packed = runs_of_4(membership(chunk, &PACKED_ANY32)) as u32;

    let margin = !0u32 << (LANES - MARGIN);
    repeat | carriable | packed | margin
}

/// One bit per lane: is this byte in the set the nibble table describes.
#[inline]
fn membership(chunk: V, lo32: &[u8; LANES]) -> u64 {
    let lo = V::from_array(*lo32);
    let hi = V::from_array(NIBBLE32);
    let lo_sel = lo.swizzle_dyn(chunk & V::splat(0x0F));
    let hi_sel = hi.swizzle_dyn((chunk >> V::splat(4)) & V::splat(0x0F));
    let miss = (lo_sel & hi_sel).simd_eq(V::splat(0)).to_bitmask();
    !miss & (u64::MAX >> (64 - LANES))
}

#[inline]
fn runs_of_4(m: u64) -> u64 {
    let m = m & (m >> 1);
    m & (m >> 2)
}

#[inline]
fn runs_of_8(m: u64) -> u64 {
    let m = m & (m >> 1);
    let m = m & (m >> 2);
    m & (m >> 4)
}

/// Nibble-pair tables over the bytes below 128. Everything at or above 128
/// indexes a zero in [`NIBBLE_BITS`] and is therefore never a member, which is
/// correct for both sets here: neither holds one.
const CARRIABLE32: [u8; LANES] = dup({
    let mut lo = [0u8; 16];
    let mut b = 0usize;
    while b < 128 {
        if crate::tables::PT_CARRIABLE[b] {
            lo[b & 15] |= 1 << (b >> 4);
        }
        b += 1;
    }
    lo
});

const PACKED_ANY32: [u8; LANES] = dup({
    let mut lo = [0u8; 16];
    let mut b = 0usize;
    while b < 128 {
        if crate::tables::PACKED_MEMBERSHIP[b] != 0 {
            lo[b & 15] |= 1 << (b >> 4);
        }
        b += 1;
    }
    lo
});

// ---------------------------------------------------------------------------
// The block coder
// ---------------------------------------------------------------------------

use std::simd::{u32x4, u8x16};

/// Eight thirteen-bit symbols out of thirteen bytes, in two vector steps.
///
/// **Measured, correct, and not used**, because it is 2.6x slower than the
/// `u128` shifts it replaces: 1 180 MB/s against 3 050. The symbols cannot
/// stay in the vector registers -- digit conversion is a lookup in a
/// 16 KiB table, which no shuffle reaches -- and moving eight lanes back out
/// costs more than the eight 128-bit shifts saved. It is kept because the next
/// person to have this idea should have the number rather than the idea.
///
/// It would pay only as part of a fully vectorised digit conversion, where
/// nothing leaves the registers until the sixteen characters are stored: the
/// division by 91 as a multiply-shift on `u32` lanes, and the alphabet as
/// arithmetic for values 0-61 with a shuffle for the twenty-nine punctuation
/// characters above them. That is the shape a base64 encoder uses, and it is
/// what this function would need around it.
///
/// The scalar path holds the group in a `u128` and shifts it eight times, and
/// a 128-bit shift is two instructions on a 64-bit machine. Here each symbol
/// gets a 32-bit lane holding the four bytes that cover it, big-endian, and
/// one variable shift settles all four lanes at once.
///
/// Symbol `k` occupies bits `[13k, 13k+13)` of the group counted from the top,
/// so it begins in byte `13k/8` at bit offset `13k%8`, and a four-byte
/// big-endian load at that byte puts it at `19 - offset` from the bottom:
///
/// | k | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
/// |---|---|---|---|---|---|---|---|---|
/// | byte | 0 | 1 | 3 | 4 | 6 | 8 | 9 | 11 |
/// | shift | 19 | 14 | 17 | 12 | 15 | 18 | 13 | 16 |
///
/// The highest byte read is 14, so sixteen readable bytes are enough for
/// thirteen of payload. `tests::simd_extract_matches_scalar` checks every lane
/// against the `u128` path on random groups, because a bit layout that is
/// nearly right here is a codec that is nearly right everywhere.
#[inline]
pub fn extract_group(bytes: &[u8]) -> [u32; 8] {
    debug_assert!(bytes.len() >= 16);
    let src = u8x16::from_slice(&bytes[..16]);

    // Two sixteen-byte shuffles rather than one of thirty-two: a byte shuffle
    // is a single-lane instruction on x86, and asking for thirty-two lanes
    // makes the compiler add the lane correction back.
    const LO: [u8; 16] = [3, 2, 1, 0, 4, 3, 2, 1, 6, 5, 4, 3, 7, 6, 5, 4];
    const HI: [u8; 16] = [9, 8, 7, 6, 11, 10, 9, 8, 12, 11, 10, 9, 14, 13, 12, 11];
    let lo = src.swizzle_dyn(u8x16::from_array(LO));
    let hi = src.swizzle_dyn(u8x16::from_array(HI));

    let lo: u32x4 = unsafe { std::mem::transmute(lo) };
    let hi: u32x4 = unsafe { std::mem::transmute(hi) };
    let mask = u32x4::splat(8191);
    let a = (lo >> u32x4::from_array([19, 14, 17, 12])) & mask;
    let b = (hi >> u32x4::from_array([15, 18, 13, 16])) & mask;

    let (a, b) = (a.to_array(), b.to_array());
    [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]]
}
