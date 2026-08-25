// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The decoder: specification section 12.
//!
//! Every field says how long it is before it starts, so this holds no
//! lookahead and no state across a segment boundary. What it does hold is the
//! rule that a decoder must not read outside its input and must not allocate
//! on an attacker's word alone -- every length is checked against its class's
//! bound before anything is reserved.

use crate::error::{Code, Error, Result};
use crate::tables::*;

#[cfg(feature = "zstd")]
use std::io::Read;

/// The most a decoder reserves up front on a frame's own word about how big it
/// will be. Past this the buffer grows as bytes actually arrive.
#[cfg(feature = "zstd")]
const RESERVE_CAP: usize = 1 << 26;

/// The content size a frame declares, where it declares one. A hint for the
/// allocation, never a bound: the bound is the caller's.
#[cfg(feature = "zstd")]
fn frame_content_size(frame: &[u8]) -> usize {
    zstd::zstd_safe::get_frame_content_size(frame)
        .ok()
        .flatten()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0)
}

pub struct Decoder<'a> {
    src: Vec<u8>, // significant characters only
    raw: &'a [u8],
    at: usize,
    bits: u32,
    n: u32,
    /// A ceiling on everything emitted, since runs and packed classes expand.
    budget: usize,
}

/// Whitespace is removed before decoding: none of the four is in the alphabet,
/// so wrapped output decodes unchanged. Removal is one pass, not a repeated
/// scan, which is what keeps a padded stream linear (section 16).
fn significant(text: &[u8]) -> Vec<u8> {
    text.iter()
        .copied()
        .filter(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
        .collect()
}

pub fn decode(text: &str) -> Result<Vec<u8>> {
    decode_bounded(text, usize::MAX / 4)
}

pub fn decode_bounded(text: &str, budget: usize) -> Result<Vec<u8>> {
    let raw = text.as_bytes();
    let mut d = Decoder {
        src: significant(raw),
        raw,
        at: 0,
        bits: 0,
        n: 0,
        budget,
    };
    d.run()
}

impl<'a> Decoder<'a> {
    fn err(&self, code: Code, what: &'static str) -> Error {
        Error::new(code, self.at, what)
    }

    #[inline]
    fn left(&self) -> usize {
        self.src.len() - self.at
    }

    #[inline]
    fn digit(&self, i: usize) -> Result<u16> {
        let v = VALUE_OF[self.src[i] as usize];
        if v == 0xFF {
            return Err(Error::new(Code::InvalidCharacter, i, "not in the alphabet"));
        }
        Ok(v as u16)
    }

    #[inline]
    fn take_char(&mut self) -> Result<u16> {
        if self.left() == 0 {
            return Err(self.err(Code::UnexpectedEos, "a character was required"));
        }
        let v = self.digit(self.at)?;
        self.at += 1;
        Ok(v)
    }

    #[inline]
    fn take_pair(&mut self) -> Result<u16> {
        if self.left() < 2 {
            return Err(self.err(Code::UnexpectedEos, "a pair was required"));
        }
        let v = self.digit(self.at)? + 91 * self.digit(self.at + 1)?;
        self.at += 2;
        Ok(v)
    }

    #[inline]
    fn emit(&mut self, out: &mut Vec<u8>, byte: u8) -> Result<()> {
        if out.len() >= self.budget {
            return Err(self.err(Code::InvalidLength, "output ceiling exceeded"));
        }
        out.push(byte);
        Ok(())
    }

    fn push_bits(&mut self, value: u32, width: u32, out: &mut Vec<u8>) -> Result<()> {
        self.bits = (self.bits << width) | value;
        self.n += width;
        while self.n >= 8 {
            self.n -= 8;
            let b = ((self.bits >> self.n) & 0xFF) as u8;
            self.emit(out, b)?;
        }
        self.bits &= (1u32 << self.n) - 1;
        Ok(())
    }

    fn run(&mut self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(self.raw.len());
        loop {
            let left = self.left();
            if left == 0 {
                if self.n != 0 {
                    return Err(self.err(Code::InvalidFinalBlock, "bits owed at end of stream"));
                }
                return Ok(out);
            }
            if left == 1 || (left == 2 && self.n != 3) {
                // Section 12.2: a two-character tail that is a signal is a
                // stream that ends before its fields arrive, not a final group.
                if left == 2 {
                    let v = self.digit(self.at)? + 91 * self.digit(self.at + 1)?;
                    if v >= SIGNAL_MIN {
                        return Err(self.err(Code::UnexpectedEos, "a signal ends the stream"));
                    }
                }
                self.final_group(&mut out)?;
                return Ok(out);
            }
            let v = self.take_pair()?;
            if v >= SIGNAL_MIN {
                self.signal(v, &mut out)?;
            } else {
                self.push_bits(v as u32, SYMBOL_BITS, &mut out)?;
            }
        }
    }

    fn final_group(&mut self, out: &mut Vec<u8>) -> Result<()> {
        let owed = (8 - self.n) % 8;
        let n_end = if self.left() == 1 {
            if !(1..=6).contains(&owed) {
                return Err(self.err(Code::InvalidFinalBlock, "one character cannot owe that"));
            }
            owed
        } else {
            let w = if owed >= 7 { owed } else { owed + 8 };
            if !(7..=12).contains(&w) {
                return Err(self.err(Code::InvalidFinalBlock, "two characters cannot owe that"));
            }
            w
        };
        let w = if n_end <= 6 {
            self.take_char()? as u32
        } else {
            self.take_pair()? as u32
        };
        if w >= (1u32 << n_end) {
            return Err(self.err(Code::InvalidFinalBlock, "more bits than are owed"));
        }
        self.push_bits(w, n_end, out)?;
        if self.n != 0 {
            return Err(self.err(Code::InvalidFinalBlock, "bits left after the final group"));
        }
        Ok(())
    }

    fn length(&mut self) -> Result<usize> {
        let first = self.take_char()?;
        if first < 90 {
            return Ok(first as usize);
        }
        let p = self.take_pair()?;
        if p != ESCAPE_PAIR {
            let v = 90 + p as usize;
            if v < 90 {
                return Err(self.err(Code::InvalidLength, "tier longer than necessary"));
            }
            return Ok(v);
        }
        let p0 = self.take_pair()? as usize;
        let p1 = self.take_pair()? as usize;
        if p0 > SIGNAL_MAX as usize || p1 > SIGNAL_MAX as usize {
            return Err(self.err(Code::InvalidLength, "a length digit above the radix"));
        }
        let v = 8370 + p0 + 8280 * p1;
        Ok(v)
    }

    fn bounded_length(&mut self, cap: usize) -> Result<usize> {
        let l = self.length()?;
        if l == 0 {
            return Err(self.err(Code::InvalidLength, "length zero"));
        }
        if l > cap {
            return Err(self.err(Code::InvalidLength, "above the class bound"));
        }
        Ok(l)
    }

    fn signal(&mut self, v: u16, out: &mut Vec<u8>) -> Result<()> {
        if v == ESCAPE_PAIR {
            return Err(self.err(Code::ExtendedClass, "this version implements no escape"));
        }
        let s = v - SIGNAL_MIN;
        let hi = (s & 1) as u32;
        let class = s >> 1;
        if class > CLASS_MAX_DEFINED {
            return Err(self.err(Code::UnknownClass, "a class this version does not define"));
        }


        // The flush field, section 7.2.
        let n_enc = ((8 - self.n) % 8) + 8 * hi;
        if n_enc > 12 {
            return Err(self.err(Code::InvalidFlush, "n_enc above twelve"));
        }
        if n_enc > 0 {
            let f = if n_enc <= 6 {
                self.take_char()? as u32
            } else {
                self.take_pair()? as u32
            };
            if f >= (1u32 << n_enc) {
                return Err(self.err(Code::InvalidFlush, "more bits than the field holds"));
            }
            self.push_bits(f, n_enc, out)?;
        }
        self.bits = 0;
        self.n = 0;

        match class {
            CLASS_ZRUN => {
                let l = self.bounded_length(MAX_SEGMENT_BYTES)?;
                for _ in 0..l {
                    self.emit(out, 0)?;
                }
            }
            CLASS_RUN => {
                let l = self.bounded_length(MAX_SEGMENT_BYTES)?;
                let b = self.take_pair()?;
                if b == 0 || b > 255 {
                    return Err(self.err(Code::InvalidRunValue, "zero, or above 255"));
                }
                for _ in 0..l {
                    self.emit(out, b as u8)?;
                }
            }
            CLASS_ZMIX_FIRST..=CLASS_ZMIX_LAST => {
                let g = (class - CLASS_ZMIX_FIRST + 1) as usize;
                let gaps = self.bounded_length(89)?;
                let mut total = 0usize;
                for i in 0..=gaps {
                    let zeros = self.bounded_length(MAX_SEGMENT_BYTES)?;
                    total += zeros;
                    if total > MAX_SEGMENT_BYTES {
                        return Err(self.err(Code::InvalidChain, "chain past the segment bound"));
                    }
                    for _ in 0..zeros {
                        self.emit(out, 0)?;
                    }
                    if i < gaps {
                        self.read_packed(g, 8, out)?;
                        total += g;
                    }
                }
            }
            CLASS_ZSTD => {
                #[cfg(not(feature = "zstd"))]
                {
                    return Err(self.err(Code::UnknownClass, "built without class 20"));
                }
                #[cfg(feature = "zstd")]
                {
                    let l = self.bounded_length(crate::tables::MAX_FRAME_BYTES)?;
                    // The frame is read as bytes through the block coder, then
                    // handed to zstd whole. Nothing about it is this format's
                    // business -- not its length, which the field above gave,
                    // and not its checksum.
                    let mut frame = Vec::with_capacity(l);
                    self.read_packed_bytes(l, &mut frame)?;
                    // Section 16: the expansion is attacker-controlled, so the
                    // ceiling has to bound what is *allocated* and not only
                    // what is kept. Handing the remaining budget to a one-shot
                    // decompressor asks it to reserve that much up front --
                    // with the default budget that is an exabyte, and the
                    // first version of this line aborted the process on a
                    // one-megabyte input. Reading through a capped reader
                    // grows the buffer as the frame actually produces bytes.
                    let limit = self.budget.saturating_sub(out.len());
                    let mut reader = zstd::stream::read::Decoder::new(&frame[..])
                        .map_err(|_| self.err(Code::MalformedFrame, "not a zstd frame"))?;
                    let mut plain = Vec::new();
                    plain.reserve(frame_content_size(&frame).min(limit).min(RESERVE_CAP));
                    // One byte past the ceiling, so a frame that would exceed
                    // it is caught rather than truncated silently.
                    Read::take(&mut reader, limit as u64 + 1)
                        .read_to_end(&mut plain)
                        .map_err(|_| {
                            self.err(Code::MalformedFrame, "the decompressor refused the frame")
                        })?;
                    if plain.len() > limit {
                        return Err(self.err(Code::InvalidLength, "output ceiling exceeded"));
                    }
                    out.extend_from_slice(&plain);
                }
            }
            CLASS_PACKED_FIRST..=CLASS_PACKED_LAST => {
                let ci = (class - CLASS_PACKED_FIRST) as usize;
                let l = self.bounded_length(MAX_SEGMENT_BYTES)?;
                self.read_indices(ci, l, out)?;
            }
            _ => {
                let (mask, profile) = if class == CLASS_PT {
                    let p = self.take_pair()?;
                    if p > 1023 {
                        return Err(self.err(Code::InvalidParams, "above 1023"));
                    }
                    ((p & 255) as u8, (p >> 8) as usize)
                } else {
                    (SHORTHAND_MASK[(class - CLASS_PT0) as usize], 0)
                };
                let l = self.bounded_length(MAX_SEGMENT_BYTES)?;
                let donors = crate::encode::donor_table(mask, profile);
                for _ in 0..l {
                    if self.left() == 0 {
                        return Err(self.err(Code::UnexpectedEos, "passthrough payload"));
                    }
                    let ch = self.src[self.at];
                    self.at += 1;
                    let mut byte = ch;
                    for j in 0..R_LEN {
                        if mask & (1 << j) != 0 && donors[j] == ch {
                            byte = R_CHARS[j];
                            break;
                        }
                    }
                    if VALUE_OF[ch as usize] == 0xFF {
                        return Err(self.err(Code::InvalidCharacter, "in a passthrough payload"));
                    }
                    self.emit(out, byte)?;
                }
            }
        }
        Ok(())
    }

    /// `count` bytes packed at eight bits each, into a caller's buffer rather
    /// than the output: a compressed frame is not output until zstd has seen
    /// it.
    #[cfg(feature = "zstd")]
    fn read_packed_bytes(&mut self, count: usize, into: &mut Vec<u8>) -> Result<()> {
        let chars = 2 * ((count * 8 + 12) / 13);
        if self.left() < chars {
            return Err(self.err(Code::UnexpectedEos, "a compressed payload"));
        }
        let (mut bits, mut nb) = (0u64, 0u32);
        while into.len() < count {
            let v = self.take_pair()?;
            if v >= SIGNAL_MIN {
                return Err(self.err(Code::InvalidCharacter, "a signal inside a frame"));
            }
            bits = (bits << SYMBOL_BITS) | v as u64;
            nb += SYMBOL_BITS;
            while nb >= 8 && into.len() < count {
                nb -= 8;
                into.push(((bits >> nb) & 0xFF) as u8);
            }
        }
        Ok(())
    }

    /// `count` bytes packed at `w` bits each, as their own symbol run.
    fn read_packed(&mut self, count: usize, w: u32, out: &mut Vec<u8>) -> Result<()> {
        let chars = 2 * ((count * w as usize + 12) / 13);
        if self.left() < chars {
            return Err(self.err(Code::UnexpectedEos, "a packed payload"));
        }
        let (mut bits, mut nb) = (0u64, 0u32);
        let mut produced = 0usize;
        for _ in 0..chars / 2 {
            let v = self.take_pair()?;
            if v >= SIGNAL_MIN {
                return Err(self.err(Code::InvalidCharacter, "a signal inside a payload"));
            }
            bits = (bits << SYMBOL_BITS) | v as u64;
            nb += SYMBOL_BITS;
            while nb >= w && produced < count {
                nb -= w;
                let idx = ((bits >> nb) & ((1u64 << w) - 1)) as u8;
                self.emit(out, idx)?;
                produced += 1;
            }
        }
        Ok(())
    }

    /// The same, mapped back through a packed class's alphabet.
    fn read_indices(&mut self, ci: usize, count: usize, out: &mut Vec<u8>) -> Result<()> {
        let w = PACKED[ci].w;
        let alphabet = PACKED[ci].chars;
        let chars = 2 * ((count * w as usize + 12) / 13);
        if self.left() < chars {
            return Err(self.err(Code::UnexpectedEos, "a packed payload"));
        }
        let (mut bits, mut nb) = (0u64, 0u32);
        let mut produced = 0usize;
        for _ in 0..chars / 2 {
            let v = self.take_pair()?;
            if v >= SIGNAL_MIN {
                return Err(self.err(Code::InvalidCharacter, "a signal inside a payload"));
            }
            bits = (bits << SYMBOL_BITS) | v as u64;
            nb += SYMBOL_BITS;
            while nb >= w && produced < count {
                nb -= w;
                let idx = ((bits >> nb) & ((1u64 << w) - 1)) as usize;
                if idx >= alphabet.len() {
                    return Err(self.err(Code::InvalidIndex, "index above the class alphabet"));
                }
                self.emit(out, alphabet[idx])?;
                produced += 1;
            }
        }
        Ok(())
    }
}
