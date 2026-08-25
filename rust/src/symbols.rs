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

use crate::tables::{ALPHABET, SYMBOL_BITS};

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

/// Characters `len` bytes cost in block mode, given `n` pending bits, and the
/// bits left over afterwards. Used by the candidate scan to compare like with
/// like: a segment is only worth emitting against what block mode would have
/// charged for the same bytes from the same state.
#[inline]
pub fn block_cost(len: usize, n: u32) -> (usize, u32) {
    let bits = 8 * len as u64 + n as u64;
    let symbols = bits / SYMBOL_BITS as u64;
    (2 * symbols as usize, (bits % SYMBOL_BITS as u64) as u32)
}

/// Characters a payload of `len` bytes occupies at `w` bits each, padded to
/// whole symbols. Specification section 9.
#[inline]
pub fn packed_chars(len: usize, w: u32) -> usize {
    2 * ((len * w as usize + 12) / 13)
}
