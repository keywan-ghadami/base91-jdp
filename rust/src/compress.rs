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

use crate::encode::{encode, Encoder};
use crate::symbols::{block_bulk, length_chars, put_length};
use crate::tables::{CLASS_ZSTD, SIGNAL_MIN};

/// Payload per frame. Section 17.9 is why it is not the whole input.
pub const FRAME_PAYLOAD: usize = 1 << 20;

/// A conservative ceiling on what one frame may expand to on decode, used
/// where the frame declares no content size. Specification section 16: the
/// expansion is attacker-controlled and the ceiling belongs on the total.
pub const DEFAULT_EXPANSION_LIMIT: usize = 1 << 30;

/// Encode as compressed segments, without weighing them against anything.
pub fn encode_zstd(data: &[u8], level: i32) -> std::io::Result<String> {
    let mut enc = Encoder::new();
    enc.out.reserve(2 * (8 * data.len() + 12) / 13 + 64);
    let mut compressor = zstd::bulk::Compressor::new(level)?;
    for chunk in data.chunks(FRAME_PAYLOAD) {
        let frame = compressor.compress(chunk)?;
        // The signal, with an empty flush field: block mode is at a group
        // boundary before the first segment and after every one.
        let mut out = std::mem::take(&mut enc.out);
        crate::symbols::put_pair(SIGNAL_MIN + 2 * CLASS_ZSTD, &mut out);
        put_length(frame.len(), &mut out);
        // A payload pads its last symbol with zero bits rather than emitting
        // a short final group: specification section 9, and the decoder
        // computes the character count from the length field before reading
        // any of them. Closing with the final-group rule instead makes a
        // stream whose frame is one character short.
        let mut acc = crate::symbols::Acc::new();
        block_bulk(&mut acc, &mut out, &frame);
        acc.finish_padded(&mut out);
        enc.out = out;
    }
    Ok(unsafe { String::from_utf8_unchecked(std::mem::take(&mut enc.out)) })
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
    let mut compressor = zstd::bulk::Compressor::new(level)?;
    let mut chars = 0usize;
    for chunk in data.chunks(FRAME_PAYLOAD) {
        let n = compressor.compress(chunk)?.len();
        chars += 2 + length_chars(n) + 2 * ((8 * n + 12) / 13);
    }
    Ok(chars)
}
