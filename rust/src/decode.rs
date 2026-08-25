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

/// The most a decoder reserves up front for a frame's plaintext. Past this the
/// buffer grows as bytes actually arrive.
#[cfg(feature = "zstd")]
const RESERVE_CAP: usize = 1 << 26;

/// The name of a class, for [`explain`].
fn class_name(class: u16) -> &'static str {
    match class {
        CLASS_PT => "PT",
        1 => "PT0",
        2 => "PT_S",
        3 => "PT_SL",
        4 => "PT_SQ",
        5 => "PT_SQL",
        6 => "PT_Z",
        CLASS_ZSTD => "ZSTD",
        CLASS_ZBLK => "ZBLK",
        CLASS_ZRUN => "ZRUN",
        CLASS_RUN => "RUN",
        CLASS_PACKED_FIRST..=CLASS_PACKED_LAST => {
            PACKED[(class - CLASS_PACKED_FIRST) as usize].name
        }
        _ => "?",
    }
}

/// What to reserve for a frame's plaintext. A frame declares its content size
/// only when an encoder pays for the field, and this one does not (see
/// `compress::lean`), so the guess comes from the frame itself: four to one is
/// a little under what the corpus averages, which is what a reserve wants. A
/// hint for the allocation, never a bound -- the bound is the caller's.
#[cfg(feature = "zstd")]
fn frame_reserve(frame: &[u8]) -> usize {
    frame.len().saturating_mul(4)
}

pub struct Decoder<'a> {
    src: std::borrow::Cow<'a, [u8]>, // significant characters only
    /// Character -> the byte it stands for, or `INVALID`. Identity for every
    /// alphabet character; a passthrough segment overwrites at most the eight
    /// donor entries and puts them back afterwards, so the table is built once
    /// for the stream rather than once per segment, and the per-byte work is
    /// one lookup instead of a walk over the R-Set.
    xlat: [u16; 256],
    raw: &'a [u8],
    at: usize,
    bits: u32,
    n: u32,
    /// A ceiling on everything emitted, since runs and packed classes expand.
    budget: usize,
    /// Set by [`explain`]: which class carried how many bytes.
    trace: Option<Vec<(&'static str, usize)>>,
}

/// Whitespace is removed before decoding: none of the four is in the alphabet,
/// so wrapped output decodes unchanged. Removal is one pass, not a repeated
/// scan, which is what keeps a padded stream linear (section 16).
/// The stream without its insignificant whitespace.
///
/// Borrowed where there is none to remove, which is every stream this
/// implementation produces and most streams anyone else will hand it. The
/// copy is not free: filtering a megabyte one byte at a time, with a branch
/// per byte and a growing vector, cost more than decoding it did -- a JPEG
/// went from 384 MB/s to 810 the moment this stopped allocating, and a WASM
/// binary from 328 to 596. The scan
/// that decides is a single pass with no branch the compiler cannot
/// vectorise.
fn significant(text: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    let dirty = text
        .iter()
        .fold(0u8, |a, &b| a | u8::from(matches!(b, b' ' | b'\t' | b'\n' | b'\r')));
    if dirty == 0 {
        return std::borrow::Cow::Borrowed(text);
    }
    std::borrow::Cow::Owned(
        text.iter()
            .copied()
            .filter(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
            .collect(),
    )
}

/// A character stands for a byte the alphabet does not hold.
const INVALID: u16 = 0xFFFF;

/// Identity for every alphabet character, `INVALID` for everything else.
fn identity_xlat() -> [u16; 256] {
    let mut t = [INVALID; 256];
    let mut c = 0usize;
    while c < 256 {
        if VALUE_OF[c] != 0xFF {
            t[c] = c as u16;
        }
        c += 1;
    }
    t
}

/// Which classes a stream used, and how many input bytes each carried.
///
/// A benchmark that reports only a ratio cannot say whether a class fired at
/// all, and the packed bases of section 9 are the ones most likely to be
/// silently never chosen. This decodes and reports rather than guessing from
/// the size.
pub fn explain(text: &str) -> Result<Vec<(&'static str, usize)>> {
    let raw = text.as_bytes();
    let mut d = Decoder {
        src: significant(raw),
        raw,
        at: 0,
        bits: 0,
        n: 0,
        budget: usize::MAX / 4,
        xlat: identity_xlat(),
        trace: Some(Vec::new()),
    };
    d.run()?;
    Ok(d.trace.take().unwrap_or_default())
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
        xlat: identity_xlat(),
        trace: None,
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
            if self.n == 0 && left >= 18 {
                // Whole groups, the mirror of `symbols::block_bulk`: sixteen
                // characters are eight pairs are thirteen bytes, exactly, with
                // no bits owed at either end. The scalar path below reaches the
                // same answer one byte at a time through `emit`, which checks
                // the ceiling per byte and pushes per byte; here the ceiling is
                // checked once per group and the thirteen bytes are one write.
                // Eighteen rather than sixteen keeps the final group, which is
                // one or two characters, out of the fast path entirely.
                if self.bulk(&mut out)? {
                    continue;
                }
            }
            let v = self.take_pair()?;
            if v >= SIGNAL_MIN {
                self.signal(v, &mut out)?;
            } else {
                self.push_bits(v as u32, SYMBOL_BITS, &mut out)?;
            }
        }
    }

    /// Decode as many whole groups as run without a signal. `Ok(false)` means
    /// the first group was not one this path may take, and the caller should
    /// fall back; anything it does take, it has consumed.
    ///
    /// The bounds are established once for the whole group, so the inner reads
    /// carry no check of their own -- `left >= 18` at entry, and the loop
    /// re-establishes it before every iteration.
    fn bulk(&mut self, out: &mut Vec<u8>) -> Result<bool> {
        let mut took = false;
        while self.left() >= 18 && out.len() + 13 <= self.budget {
            let w = &self.src[self.at..self.at + 16];
            let mut g: u128 = 0;
            // No branch inside the group. `VALUE_OF` is 0..=90 for a character
            // in the alphabet and 0xFF for one that is not, so bit 7 of the OR
            // of all sixteen lookups is set exactly when one of them failed;
            // and the OR of the eight pair values reaches `SIGNAL_MIN` exactly
            // when one of them is a signal. Two conditions, one branch, once
            // per thirteen bytes -- where the first version of this asked
            // sixteen times and decoded a JPEG at 405 MB/s.
            let mut bad = 0u8;
            let mut sig = 0u16;
            for k in 0..8 {
                let lo = VALUE_OF[w[2 * k] as usize];
                let hi = VALUE_OF[w[2 * k + 1] as usize];
                bad |= lo | hi;
                let v = (lo as u16 & 0x7F) + 91 * (hi as u16 & 0x7F);
                sig |= v;
                g |= (v as u128) << (115 - 13 * k);
            }
            if bad & 0x80 != 0 || sig >= SIGNAL_MIN {
                // A character outside the alphabet, or a signal: both are the
                // scalar path's business, which reports where and why.
                return Ok(took);
            }
            out.extend_from_slice(&g.to_be_bytes()[..13]);
            self.at += 16;
            took = true;
            // No trace entry: block mode is what `explain` reports by
            // subtraction, exactly as the scalar path leaves it.
        }
        Ok(took)
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

        let before = out.len();
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
            CLASS_ZSTD | CLASS_ZBLK => {
                #[cfg(not(feature = "zstd"))]
                {
                    return Err(self.err(Code::UnknownClass, "built without classes 17 and 20"));
                }
                #[cfg(feature = "zstd")]
                {
                    let l = self.bounded_length(crate::tables::MAX_FRAME_BYTES)?;
                    // The payload is read as bytes through the block coder and
                    // then handed to zstd. Nothing about it is this format's
                    // business -- not its length, which the field above gave,
                    // and not its checksum, which it does not have.
                    let mut frame = Vec::with_capacity(l + 5);
                    if class == CLASS_ZBLK {
                        // Class 20 carries a bare block. The five bytes of
                        // header that would precede it are all implied, so the
                        // decoder writes them rather than reading them; see
                        // `compress::strip` for each one.
                        if l > crate::tables::MAX_BLOCK_BYTES {
                            return Err(self.err(Code::InvalidLength, "above one block"));
                        }
                        let mut block = Vec::with_capacity(l);
                        self.read_packed_bytes(l, &mut block)?;
                        crate::compress::unstrip(&block, &mut frame);
                    } else {
                        self.read_packed_bytes(l, &mut frame)?;
                    }
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
                    // The frame carries no magic number: the segment signal
                    // already said what it is. See `compress::lean`.
                    reader
                        .set_parameter(zstd::zstd_safe::DParameter::Format(
                            zstd::zstd_safe::FrameFormat::Magicless,
                        ))
                        .map_err(|_| self.err(Code::MalformedFrame, "magicless not supported"))?;
                    let mut plain =
                        Vec::with_capacity(frame_reserve(&frame).min(limit).min(RESERVE_CAP));
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
                if self.left() < l {
                    return Err(self.err(Code::UnexpectedEos, "passthrough payload"));
                }
                if out.len() + l > self.budget {
                    return Err(self.err(Code::InvalidLength, "output ceiling exceeded"));
                }
                // Lend the table the segment's donors, and take them back
                // after: only the set bits move, so this is at most eight
                // writes however long the payload is.
                let mut saved = [(0u8, 0u16); R_LEN];
                let mut n_saved = 0;
                for j in 0..R_LEN {
                    if mask & (1 << j) != 0 {
                        let d = donors[j];
                        saved[n_saved] = (d, self.xlat[d as usize]);
                        n_saved += 1;
                        self.xlat[d as usize] = R_CHARS[j] as u16;
                    }
                }
                let mut bad = None;
                out.reserve(l);
                for k in 0..l {
                    let ch = self.src[self.at + k];
                    let b = self.xlat[ch as usize];
                    if b == INVALID {
                        bad = Some(k);
                        break;
                    }
                    out.push(b as u8);
                }
                for &(d, v) in &saved[..n_saved] {
                    self.xlat[d as usize] = v;
                }
                if let Some(k) = bad {
                    self.at += k;
                    return Err(self.err(Code::InvalidCharacter, "in a passthrough payload"));
                }
                self.at += l;
            }
        }
        if let Some(t) = self.trace.as_mut() {
            t.push((class_name(class), out.len() - before));
        }
        Ok(())
    }

    /// `count` bytes packed at eight bits each, into a caller's buffer rather
    /// than the output: a compressed frame is not output until zstd has seen
    /// it.
    #[cfg(feature = "zstd")]
    fn read_packed_bytes(&mut self, count: usize, into: &mut Vec<u8>) -> Result<()> {
        let chars = 2 * (count * 8).div_ceil(13);
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

    /// The same, mapped back through a packed class's alphabet.
    fn read_indices(&mut self, ci: usize, count: usize, out: &mut Vec<u8>) -> Result<()> {
        let w = PACKED[ci].w;
        let alphabet = PACKED[ci].chars;
        let chars = 2 * (count * w as usize).div_ceil(13);
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
