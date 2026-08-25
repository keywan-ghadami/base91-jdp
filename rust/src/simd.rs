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
//! [`SkipSet`] is the same idea applied to the passthrough prefix scan, and it
//! is **not wired in, because it was measured and it loses.** Four
//! arrangements were tried on the core corpus against the scalar scan at
//! 30-114 MB/s:
//!
//! | arrangement | result |
//! |---|---|
//! | static set, shuffle emulated with `to_array` | 0-20 MB/s |
//! | static set, real `swizzle_dyn`, repeats scanned separately | 5-56 MB/s |
//! | static set, repeats folded into the same vector step | 6-58 MB/s |
//! | ... and a guard against retrying the probe per byte | 18-87 MB/s |
//! | set rebuilt as the segment's state changes | 2-83 MB/s |
//!
//! The reason is the format, not the code. A vector probe pays when it settles
//! many bytes per call, and the bytes that stop this one -- the R-Set members
//! and the donors -- are precisely the frequent characters of the text the
//! scan runs on. Making the set dynamic lengthens the skips and costs a
//! 128-entry rebuild every time a literal lowers a donor rank, which on prose
//! is often. The scalar loop's six L1 lookups per byte are simply hard to beat
//! here.
//!
//! What would be worth trying next is vectorising the *donor bookkeeping*
//! rather than skipping it: the four per-profile minima are four bytes, one
//! `u32` lane each, and `min` across them is one instruction. That is Base85N's
//! section 11.2, and it accelerates the work instead of trying to avoid it.
//!
//! The membership test is the nibble-pair lookup simdjson uses: byte `b` is in
//! the set iff bit `b >> 4` of `lo[b & 15]` is set. Sixteen bytes describe any
//! subset of ASCII, and bytes at or above 127 index a zero in [`NIBBLE_BITS`],
//! so they always stop the skip -- which they must, since none of them is
//! representable in passthrough at all.

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
