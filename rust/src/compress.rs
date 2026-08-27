// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Classes 17 and 20, the compressed segments: specification sections 10.1
//! and 10.2.
//!
//! The payload is zstd output, packed at `w = 8` through the block coder. What
//! is interesting here is how little of a zstd frame survives the trip, and
//! why: the segment around it already says most of what a frame header says.
//! [`lean`] takes off the magic number, the content size, the checksum and the
//! dictionary id; [`strip`] takes off the frame header and the block header
//! when a payload compresses to a single block, which is every payload up to
//! 128 KiB.
//!
//! Three things here are decisions rather than transcription.
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
//!
//! **Why both compressed classes exist.** A block is at most 128 KiB, so the
//! stripped form of section 10.2 cannot carry a megabyte, and a megabyte of
//! payload cut into 128 KiB frames so that it could would cost 1.9 % at level
//! -5, 4.7 % at level 3 and 4.7 % at level 9 over the core corpus, because
//! each frame then starts its window empty. Five bytes are not worth four
//! percent. Class 17 keeps the frame for payloads that need more than one
//! block; class 20 strips it for the ones that do not, which is where five
//! bytes are 8 % of the encoding.

use crate::encode::encode_plain;
use crate::symbols::{block_bulk, length_chars, put_length};
use crate::tables::{CLASS_ZBLK, CLASS_ZSTD, SIGNAL_MIN};

/// The level [`crate::encode`] uses. Section 17.21 measures the alternatives:
/// level 1 is 2.7 times smaller than Base85N at 83 % of its throughput, and it
/// is the first level at which nothing is left in the frame for the scan to
/// have found (Section 10.1). Below it the output grows faster than the
/// encoder does.
pub const DEFAULT_LEVEL: i32 = 1;

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

    /// One decompression context per thread, kept between calls.
    ///
    /// The mirror of `COMPRESSOR`, and the same cost on the other side: a
    /// `ZSTD_DCtx` and the streaming buffers around it are built per call by
    /// every one-shot entry point zstd offers, and on a field-sized payload
    /// that setup *is* the measurement -- a sixty-nine-byte frame took 14.7 us
    /// to decompress with a context built for it and 129 ns with one kept.
    /// At a quarter of a megabyte it is still more than half of what the
    /// decompression costs.
    ///
    /// Not keyed by anything: a decompressor has no level, and the one
    /// parameter this crate sets is the same on every frame it reads.
    static DECOMPRESSOR: std::cell::RefCell<zstd::zstd_safe::DCtx<'static>> =
        std::cell::RefCell::new(zstd::zstd_safe::DCtx::create());
}

/// The frame settings, which are the interesting part of Section 10.1.
///
/// Everything switched off here is something the container already says, or
/// something the format explicitly does not claim:
///
/// * **The magic number.** Four bytes identifying the content as zstd, in a
///   segment whose signal identified it as zstd. `Magicless` is a documented
///   zstd frame format, not a modification of one -- a decoder reads it by
///   setting the same flag.
/// * **The content size.** One to eight bytes saying how large the payload
///   decompresses to. A decoder must bound that from the caller's ceiling
///   anyway (Section 16), so the field buys an allocation hint and costs
///   bytes on every frame.
/// * **The checksum.** The format makes no integrity claim (Section 2.3), and
///   a four-byte XXH64 tail on every frame is not the place to start making
///   one. Off by default in zstd; set here so that it stays off if that ever
///   changes.
/// * **The dictionary id.** Section 10.1 forbids dictionaries.
///
/// Worth 8 to 9 % of a field-sized payload's encoding and nothing at all on a
/// megabyte, which is what a fixed cost looks like.
fn lean(c: &mut zstd::bulk::Compressor<'static>) -> std::io::Result<()> {
    use zstd::zstd_safe::{CParameter, FrameFormat};
    for p in [
        CParameter::Format(FrameFormat::Magicless),
        CParameter::ContentSizeFlag(false),
        CParameter::ChecksumFlag(false),
        CParameter::DictIdFlag(false),
    ] {
        c.set_parameter(p)?;
    }
    Ok(())
}

/// The five bytes a lean frame still spends saying what the segment said.
///
/// A lean frame is `[frame header][block header][block]...`. With everything
/// [`lean`] switches off, the frame header is two bytes -- a descriptor whose
/// every field is now zero, and a window descriptor -- and each block header
/// is three: a last-block flag, a two-bit block type and a 21-bit block size.
/// When the whole payload came out as one compressed block, all five are
/// already known to a decoder that has the segment:
///
/// * the **descriptor** is `0x00` by construction, since the four flags it
///   carries are the four this encoder turned off;
/// * the **window descriptor** need not be the encoder's. A single block
///   decompresses to at most 128 KiB, so no match in it can reach further
///   back than that, and a decoder that declares a 128 KiB window decodes any
///   single-block frame whatever window the encoder chose. It is a constant;
/// * the **last-block flag** is set, because there is one block;
/// * the **block type** is *compressed*, because a frame whose only block came
///   out raw or run-length is a frame this encoder does not emit -- block mode
///   and class 19 carry that input better, and section 11.2 picks them;
/// * the **block size** is the segment's length field, less nothing.
///
/// So the five bytes are dropped and the segment is signalled as class 20.
/// `None` means the shape did not hold -- more than one block, or a block that
/// zstd did not compress -- and the caller keeps the frame under class 17.
///
/// This reads a frame that this process just produced, using only the frame
/// format of RFC 8878 section 3.1.1, which is stable. It does not use zstd's
/// block API, which does the same thing more directly and which upstream has
/// deprecated for removal.
fn strip(frame: &[u8]) -> Option<&[u8]> {
    if frame.len() < 5 || frame[0] != 0x00 {
        return None;
    }
    let header = u32::from(frame[2]) | u32::from(frame[3]) << 8 | u32::from(frame[4]) << 16;
    let last = header & 1 == 1;
    let compressed = (header >> 1) & 3 == 2;
    let size = (header >> 3) as usize;
    (last && compressed && size == frame.len() - 5).then(|| &frame[5..])
}

/// The window descriptor a decoder puts back: exponent 7, mantissa 0, which
/// is 128 KiB and therefore covers any single block. See [`strip`].
const ZBLK_WINDOW: u8 = (17 - 10) << 3;

/// Put back what [`strip`] took off, so that a stock zstd decoder can read it.
pub(crate) fn unstrip(block: &[u8], out: &mut Vec<u8>) {
    let header = 1 | 2 << 1 | (block.len() as u32) << 3;
    out.extend_from_slice(&[
        0x00,
        ZBLK_WINDOW,
        header as u8,
        (header >> 8) as u8,
        (header >> 16) as u8,
    ]);
    out.extend_from_slice(block);
}

/// Write one compressed segment: the signal, the length and the payload.
fn put_segment(class: u16, payload: &[u8], out: &mut Vec<u8>) {
    // The signal, with an empty flush field: block mode is at a group
    // boundary before the first segment and after every one.
    crate::symbols::put_pair(SIGNAL_MIN + 2 * class, out);
    put_length(payload.len(), out);
    // A payload pads its last symbol with zero bits rather than emitting a
    // short final group: specification section 9, and the decoder computes the
    // character count from the length field before reading any of them.
    // Closing with the final-group rule instead makes a stream whose frame is
    // one character short.
    let mut acc = crate::symbols::Acc::new();
    block_bulk(&mut acc, out, payload);
    acc.finish_padded(out);
}

/// Characters one compressed segment occupies, given its payload length.
fn segment_chars(payload: usize) -> usize {
    2 + length_chars(payload) + 2 * (8 * payload).div_ceil(13)
}

fn with_compressor<T>(
    level: i32,
    f: impl FnOnce(&mut zstd::bulk::Compressor<'static>) -> std::io::Result<T>,
) -> std::io::Result<T> {
    COMPRESSOR.with(|cell| {
        let mut slot = cell.borrow_mut();
        let reuse = matches!(&*slot, Some((l, _)) if *l == level);
        if !reuse {
            let mut c = zstd::bulk::Compressor::new(level)?;
            lean(&mut c)?;
            *slot = Some((level, c));
        }
        f(&mut slot.as_mut().unwrap().1)
    })
}

/// Run `f` against this thread's decompression context, reset for a new frame
/// and set to read the magicless frames [`lean`] writes.
///
/// `ZSTD_reset_session_only` leaves the parameters alone and abandons any
/// stream in progress, so a frame that failed half way through cannot leave
/// the context poisoned for the next one.
pub(crate) fn with_decompressor<T>(
    f: impl FnOnce(&mut zstd::zstd_safe::DCtx<'static>) -> std::io::Result<T>,
) -> std::io::Result<T> {
    use zstd::zstd_safe::{DParameter, FrameFormat, ResetDirective};
    DECOMPRESSOR.with(|cell| {
        let mut ctx = cell.borrow_mut();
        ctx.reset(ResetDirective::SessionOnly)
            .map_err(|_| std::io::Error::other("zstd context reset"))?;
        ctx.set_parameter(DParameter::Format(FrameFormat::Magicless))
            .map_err(|_| std::io::Error::other("magicless is not supported"))?;
        f(&mut ctx)
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

            match strip(&frame) {
                Some(block) => put_segment(CLASS_ZBLK, block, &mut out),
                None => put_segment(CLASS_ZSTD, &frame, &mut out),
            }
            Ok(())
        })?;
    }
    Ok(unsafe { String::from_utf8_unchecked(out) })
}

/// Build both candidates and return the shorter, which is what section 11.2
/// requires of a conforming encoder.
pub fn encode_auto(data: &[u8], level: i32) -> std::io::Result<String> {
    let plain = encode_plain(data);
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
pub fn encode_at(data: &[u8], level: i32) -> std::io::Result<String> {
    if data.len() < COMPARE_BELOW {
        // Small enough that the comparison of section 11.2 is free, and small
        // enough that the entropy sample would be guessing. Below the
        // crossover a frame header is a large fraction of the payload and
        // compression loses badly -- 1.4217 characters per byte over the short
        // corpus against 0.9252 -- so this is not an optimisation but a
        // correctness fix: an encoder that compresses everything produces
        // output worse than Base64 on a field-sized payload.
        return encode_auto(data, level);
    }
    if crate::detect::is_block(data, true) {
        // Already compressed, or close enough that nothing will come off it.
        // The plain path then costs nothing either: section 11.5 skips the
        // scan for exactly this input.
        return Ok(encode_plain(data));
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
            with_compressor(level, |c| c.compress_to_buffer(chunk, &mut *frame))?;
            Ok(strip(&frame).map_or(frame.len(), <[u8]>::len))
        })?;
        chars += segment_chars(n);
    }
    Ok(chars)
}
