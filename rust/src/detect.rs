// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Deciding that a stretch of input is block mode before scanning it.
//!
//! The candidate scan is what encoding costs on data no class can carry: the
//! block coder alone runs at 531 MB/s and a scan at every position brings the
//! encoder to 31. The vector mask of [`crate::simd`] takes that to 125 by
//! asking per window instead of per byte, and this takes it the rest of the
//! way by not asking at all where the answer is already known.
//!
//! Two signals, in order of confidence:
//!
//! * **A magic number.** A stream that begins with a zstd frame header, a
//!   JPEG, a PNG, a zip, a gzip is compressed already. Nothing in it will
//!   compress, no alphabet will hold it, and its runs are accidents. This is
//!   not a guess.
//! * **Entropy.** Above [`ENTROPY_BITS`] bits per byte over a sample, nothing
//!   the format offers can carry the data: passthrough needs ten consecutive
//!   representable bytes and a packed base needs five of one alphabet, and at
//!   that entropy neither occurs often enough to pay for looking.
//!
//! **A wrong guess costs size, never correctness**, and it is bounded: block
//! mode is the ceiling of section 11.2, so the worst a false positive can do
//! is 1.2308 characters per byte on data that would have done better. That is
//! why the decision is taken per window rather than once per stream -- a `tar`
//! alternates text headers, compressed members and zero padding every few
//! hundred bytes, and one decision at the head of it would be wrong for most
//! of its length.

/// Bytes examined per decision.
pub const WINDOW: usize = 1 << 14;

/// Bytes sampled from a window to estimate its entropy.
///
/// A thousand samples over 256 buckets underestimates a uniform distribution
/// by about 0.18 bits, which the threshold has room for: real text sits near
/// 4.5 and a compressed stream near 7.9, so the two populations are three bits
/// apart and the sample only has to tell them apart.
pub const SAMPLE: usize = 1024;

/// Above this, the window is taken to be incompressible. Section 17.12
/// measures the corpus against it; the gap between the two populations is
/// wide enough that the exact value does not matter much.
pub const ENTROPY_BITS: f32 = 7.4;

/// Magic numbers of formats whose contents are already compressed.
const MAGIC: &[&[u8]] = &[
    &[0x28, 0xB5, 0x2F, 0xFD],       // zstd frame
    &[0x1F, 0x8B],                   // gzip
    &[0x78, 0x01],                   // zlib, no/low compression
    &[0x78, 0x9C],                   // zlib, default
    &[0x78, 0xDA],                   // zlib, best
    &[0xFF, 0xD8, 0xFF],             // JPEG
    &[0x89, b'P', b'N', b'G'],       // PNG
    &[b'P', b'K', 0x03, 0x04],       // zip
    &[0xFD, b'7', b'z', b'X', b'Z'], // xz
    b"BZh",                         // bzip2
    &[b'7', b'z', 0xBC, 0xAF],       // 7z
    &[0x04, 0x22, 0x4D, 0x18],       // LZ4 frame
    b"GIF8",                        // GIF
    b"OggS",                        // Ogg
    &[0x1A, 0x45, 0xDF, 0xA3],       // Matroska / WebM
    b"%PDF",                        // PDF
];

/// Whether the stream opens with a container whose contents are compressed.
pub fn magic(data: &[u8]) -> bool {
    MAGIC.iter().any(|m| data.len() >= m.len() && &data[..m.len()] == *m)
}

/// Shannon entropy of a sample, in bits per byte.
pub fn entropy(data: &[u8]) -> f32 {
    let sample = &data[..data.len().min(SAMPLE)];
    if sample.is_empty() {
        return 0.0;
    }
    let mut hist = [0u32; 256];
    for &b in sample {
        hist[b as usize] += 1;
    }
    let n = sample.len() as f32;
    let mut h = 0.0f32;
    for &c in hist.iter() {
        if c != 0 {
            let p = c as f32 / n;
            h -= p * p.log2();
        }
    }
    h
}

/// Whether this window should go straight through block mode.
#[inline]
pub fn is_block(window: &[u8], at_stream_start: bool) -> bool {
    if at_stream_start && magic(window) {
        return true;
    }
    window.len() >= SAMPLE && entropy(window) >= ENTROPY_BITS
}
