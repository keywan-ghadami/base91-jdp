// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The bit accumulator and the pair digits: specification sections 5 and 6.
//!
//! Bits enter at the bottom and symbols leave off the top, most significant
//! first, and a pair is `d0 + d1 * 91` with the low digit first. Every field
//! in the format uses this order, the flush field included -- there is no
//! little-endian path anywhere, which is the one thing a second implementation
//! is most likely to get wrong.

pub use crate::tables::{ALPHABET, PAIR_CHARS, SYMBOL_BITS};

/// The encoder's pending bits: at most twelve, since a thirteenth would have
/// become a symbol.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Acc {
    bits: u32,
    pub n: u32,
}

impl Acc {
    #[inline]
    pub fn new() -> Self {
        Self { bits: 0, n: 0 }
    }

    /// Push `width` bits and emit whatever whole symbols that completes.
    #[inline]
    pub fn push(&mut self, value: u32, width: u32, out: &mut Vec<u8>) {
        self.bits = (self.bits << width) | value;
        self.n += width;
        while self.n >= SYMBOL_BITS {
            self.n -= SYMBOL_BITS;
            let v = ((self.bits >> self.n) & 8191) as u16;
            put_pair(v, out);
        }
        self.bits &= (1u32 << self.n) - 1;
    }

    /// The pending bits themselves, for the flush field of section 7.2.
    #[inline]
    pub fn pending(&self) -> u32 {
        self.bits
    }

    /// Restore an accumulator saved by a bulk pass.
    #[inline]
    pub fn set(&mut self, bits: u32, n: u32) {
        self.bits = bits;
        self.n = n;
    }

    #[inline]
    pub fn reset(&mut self) {
        self.bits = 0;
        self.n = 0;
    }

    /// The final group of section 6.3: nothing, one character or two.
    pub fn finish(&mut self, out: &mut Vec<u8>) {
        match self.n {
            0 => {}
            1..=6 => out.push(ALPHABET[self.bits as usize]),
            _ => put_pair(self.bits as u16, out),
        }
        self.reset();
    }
}

/// Two characters of a pair value, low digit first.
#[inline]
pub fn put_pair(v: u16, out: &mut Vec<u8>) {
    out.push(ALPHABET[(v % 91) as usize]);
    out.push(ALPHABET[(v / 91) as usize]);
}

/// Characters the flush field occupies for `n_enc` pending bits.
#[inline]
pub fn flush_chars(n_enc: u32) -> usize {
    match n_enc {
        0 => 0,
        1..=6 => 1,
        _ => 2,
    }
}

/// Characters a length field occupies. Specification section 7.3.
#[inline]
pub fn length_chars(len: usize) -> usize {
    if len < 90 {
        1
    } else if len < 8370 {
        3
    } else {
        7
    }
}

/// Write a length field in the shortest tier that carries it.
pub fn put_length(len: usize, out: &mut Vec<u8>) {
    if len < 90 {
        out.push(ALPHABET[len]);
    } else if len < 8370 {
        out.push(ALPHABET[90]);
        put_pair((len - 90) as u16, out);
    } else {
        out.push(ALPHABET[90]);
        put_pair(8280, out);
        let rest = len - 8370;
        put_pair((rest % 8280) as u16, out);
        put_pair((rest / 8280) as u16, out);
    }
}


/// Characters a payload of `len` bytes occupies at `w` bits each, padded to
/// whole symbols. Specification section 9.
#[inline]
pub fn packed_chars(len: usize, w: u32) -> usize {
    2 * (len * w as usize).div_ceil(13)
}

// ---------------------------------------------------------------------------
// Bulk block mode
// ---------------------------------------------------------------------------

/// `v / 91` for `v <= 8280`, as a multiply and a shift.
///
/// The block coder divides by 91 twice per pair, which is twice per thirteen
/// bits, and a hardware divide is twenty-odd cycles. `11523 = ceil(2^20 / 91)`
/// makes it a multiply and a shift; `tests` checks all 8 281 values against the
/// real division rather than trusting the derivation.
#[inline(always)]
pub fn div91(v: u32) -> u32 {
    (v * 11523) >> 20
}

/// Push a stretch of bytes through block mode with nothing else in the way.
///
/// This is the same arithmetic as [`Acc::push`], written so that the
/// accumulator and the output cursor are locals rather than fields reached
/// through a borrow. That alone is most of the difference between 93 MB/s and
/// 300: the per-byte path in the encoder cannot keep either in a register
/// while a `&mut Encoder` is live across the loop.
///
/// Thirteen bytes are exactly eight symbols, so a whole group is taken at once
/// where one is available and the accumulator is empty -- one `u128` load, no
/// carry, no branch per byte.
pub fn block_bulk(acc: &mut Acc, out: &mut Vec<u8>, data: &[u8]) {
    let mut bits = acc.pending() as u64;
    let mut n = acc.n;
    let mut i = 0usize;

    // Get to a group boundary first: whatever the accumulator already owes.
    while i < data.len() && n != 0 {
        bits = (bits << 8) | data[i] as u64;
        n += 8;
        while n >= SYMBOL_BITS {
            n -= SYMBOL_BITS;
            put_pair(((bits >> n) & 8191) as u16, out);
        }
        bits &= (1u64 << n) - 1;
        i += 1;
    }

    // Then whole groups. Sixteen bytes are loaded to reach thirteen, so the
    // group loop stops three bytes early and the tail below finishes.
    if i + 16 <= data.len() {
        let groups = (data.len() - i - 3) / 13;
        out.reserve(16 * groups);
        let mut dst = out.len();
        // The buffer has room for every character this loop writes, so the
        // writes go through a pointer rather than sixteen capacity checks per
        // group. `dst` is advanced only by what was written, and `set_len` is
        // called once, after the loop.
        let base = out.as_mut_ptr();
        for _ in 0..groups {
            // One big-endian load. The thirteen bytes of the group are the top
            // 104 bits of it, so symbol k is a shift and a mask -- where an
            // earlier version of this built the same value with thirteen
            // shift-ors, which cost more than the eight extractions did.
            // One big-endian load. The thirteen bytes of the group are the
            // top 104 bits of it, so each symbol is a shift and a mask.
            //
            // Two things that look like improvements are not, and both were
            // measured: extracting the eight symbols with a vector shuffle
            // (crate::simd::extract_group) costs 2.6x, because the symbols
            // have to leave the vector registers again for the table lookup;
            // and assembling the sixteen characters into one u128 to store
            // once costs 9 %, because the assembly is more work than the seven
            // stores it saves.
            let g = u128::from_be_bytes(data[i..i + 16].try_into().unwrap());
            for k in 0..8u32 {
                let v = ((g >> (115 - 13 * k)) & 8191) as usize;
                // SAFETY: `dst` starts at `out.len()` and rises by two per
                // write, sixteen per group, over the `16 * groups` bytes
                // reserved above; `base` is that reservation and nothing
                // reallocates inside the loop. The write is unaligned by
                // declaration, and `u16` has no invalid bit patterns.
                unsafe {
                    base.add(dst).cast::<u16>().write_unaligned(PAIR_CHARS[v]);
                }
                dst += 2;
            }
            i += 13;
        }
        // SAFETY: every byte from the old length up to `dst` was written by
        // the loop, and `dst` is within the reserved capacity.
        unsafe { out.set_len(dst) };
    }

    // And the tail.
    while i < data.len() {
        bits = (bits << 8) | data[i] as u64;
        n += 8;
        while n >= SYMBOL_BITS {
            n -= SYMBOL_BITS;
            put_pair(((bits >> n) & 8191) as u16, out);
        }
        bits &= (1u64 << n) - 1;
        i += 1;
    }
    acc.set(bits as u32, n);
}
