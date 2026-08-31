// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! An emitter for streams no encoder would write.
//!
//! The round-trip tests can only reach decoder states the *encoder* can
//! produce, which is a small and well-behaved corner of what the decoder must
//! survive: an encoder never writes a length of zero, never claims a run of
//! four billion, never puts a class number the version does not define on the
//! wire. A decoder reads streams it did not write (specification Section
//! 15.4), so those are exactly the states worth reaching, and this builds
//! them field by field.
//!
//! Shared by `tests/adversarial.rs`, which fixes named cases, and by the
//! `decode_structured` fuzz target, which lets a fuzzer choose the field
//! values. One emitter, so a case found by one is expressible in the other.

#![allow(dead_code)] // Each consumer uses a different part of it.

use base91z::tables::{ALPHABET, ESCAPE_PAIR, SIGNAL_MIN};

/// The largest value a pair of characters can hold, and the escape.
pub const PAIR_MAX: u16 = ESCAPE_PAIR;
/// The largest value one character can hold.
pub const CHAR_MAX: u16 = 90;
/// The radix of a tier-three length digit. A digit equal to it would be the
/// escape pair, which the decoder rejects.
pub const TIER3_RADIX: usize = 8280;
/// Where tier three starts: one past what tier two can hold.
pub const TIER3_BASE: usize = 8370;

#[derive(Default, Clone)]
pub struct Stream {
    text: String,
}

impl Stream {
    pub fn new() -> Self {
        Self::default()
    }

    /// One character, by alphabet value. Values above 90 wrap, so a fuzzer
    /// cannot make this panic on a byte it chose.
    pub fn ch(&mut self, v: u16) -> &mut Self {
        self.text.push(ALPHABET[usize::from(v % 91)] as char);
        self
    }

    /// One pair, by value. `lo + 91 * hi` is what the decoder reads, so a
    /// value of 8280 is the escape and anything above it cannot be written.
    pub fn pair(&mut self, v: u16) -> &mut Self {
        let v = v.min(PAIR_MAX);
        self.ch(v % 91);
        self.ch(v / 91);
        self
    }

    /// A segment signal for `class`, with the flush's high bit `hi`.
    ///
    /// At the start of a stream no bits are owed, so `hi = 0` means no flush
    /// field follows and the class's own fields come next. `hi = 1` demands
    /// an eight-bit flush field there instead, which is its own thing to get
    /// wrong.
    pub fn signal(&mut self, class: u16, hi: u16) -> &mut Self {
        self.pair(SIGNAL_MIN + 2 * class + (hi & 1))
    }

    /// The escape pair, which this version of the format defines no meaning
    /// for and the decoder must refuse rather than skip.
    pub fn escape(&mut self) -> &mut Self {
        self.pair(ESCAPE_PAIR)
    }

    /// A length field in the shortest tier that holds `l` -- what an encoder
    /// would have written.
    pub fn length(&mut self, l: usize) -> &mut Self {
        if l < 90 {
            self.ch(l as u16)
        } else if l < TIER3_BASE {
            self.ch(CHAR_MAX);
            self.pair((l - 90) as u16)
        } else {
            let rest = l - TIER3_BASE;
            self.ch(CHAR_MAX);
            self.escape();
            self.pair((rest % TIER3_RADIX) as u16);
            self.pair((rest / TIER3_RADIX) as u16)
        }
    }

    /// A tier-three length field with both digits given directly, so a digit
    /// above the radix -- which no encoder can produce and the decoder has to
    /// refuse -- is expressible.
    pub fn length_tier3_raw(&mut self, p0: u16, p1: u16) -> &mut Self {
        self.ch(CHAR_MAX);
        self.escape();
        self.pair(p0);
        self.pair(p1)
    }

    /// Characters that are not a signal, to stand in for payload or to give
    /// the stream a tail.
    pub fn filler(&mut self, count: usize) -> &mut Self {
        for i in 0..count {
            self.ch((i % 91) as u16);
        }
        self
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The stream, and every prefix of it. Truncation is its own attack: a
    /// field that is half there must end the decode with an error and not
    /// with whatever was in the buffer.
    pub fn prefixes(&self) -> impl Iterator<Item = &str> {
        (0..=self.text.len()).map(move |i| &self.text[..i])
    }
}
