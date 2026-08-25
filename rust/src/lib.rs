// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The `simd` feature is nightly-only, and deliberately the only thing in this
// crate that is: see src/simd.rs and the README section it points at.
#![cfg_attr(feature = "simd", feature(portable_simd))]

//! # Base91z
//!
//! A prototype encoder and decoder for **specification v0.4.0** -- basE91 on
//! an alphabet JSON never has to escape, with typed segments.
//!
//! ```
//! let data = b"{\"user\":\"ada\",\"id\":42}";
//! let text = base91z::encode_plain(data);
//! assert_eq!(base91z::decode(&text).unwrap(), data);
//! ```
//!
//! **This is a prototype.** It implements every class of the specification --
//! passthrough, the packed bases, the runs and chained gaps, and the
//! compressed segment behind the default `zstd` feature -- with a decoder for
//! each. What it exists to answer is whether the format encodes at the density
//! the specification projects, and whether the parallel and vector paths the
//! format was shaped for actually pay.

pub mod error;
pub mod tables;

#[cfg(feature = "zstd")]
pub mod compress;
mod decode;
pub mod detect;
mod encode;
mod parallel;
pub(crate) mod symbols;

#[cfg(feature = "simd")]
pub mod simd;

/// Entry points the benchmarks use to time one layer at a time. Not part of
/// the format, and not a stable interface.
pub mod bench {
    pub use crate::encode::block_only;
    pub use crate::symbols::div91;
}

#[cfg(feature = "zstd")]
pub use compress::{encode_at, encode_auto, encode_zstd, zstd_chars, DEFAULT_LEVEL};
pub use decode::{decode, decode_bounded, explain};
pub use encode::encode_plain;

/// Encode `data`, compressing it where that is smaller.
///
/// This is the entry point. Compression is part of the format (specification
/// Section 10) rather than something bolted in front of it, so the default
/// encode uses it: the entropy sample of Section 11.5 decides per input, and
/// on a payload too short for a compressor to have a window -- a field in a
/// JSON document -- the classes of Sections 8 and 9 carry it and no frame is
/// emitted.
///
/// The level is [`DEFAULT_LEVEL`]. [`encode_at`] takes one explicitly:
/// negative levels encode faster and larger, higher ones slower and smaller,
/// and specification Section 17.21 has the table.
///
/// Infallible. A compressor error is not this caller's problem to handle --
/// there is always a valid uncompressed encoding, and that is what comes back.
/// Use [`encode_at`] where the error matters.
#[cfg(feature = "zstd")]
pub fn encode(data: &[u8]) -> String {
    encode_at(data, DEFAULT_LEVEL).unwrap_or_else(|_| encode_plain(data))
}

/// Encode `data`. Without the `zstd` feature there is nothing to compress
/// with, so this is [`encode_plain`]; see its documentation and the crate
/// README for what that costs.
#[cfg(not(feature = "zstd"))]
pub fn encode(data: &[u8]) -> String {
    encode_plain(data)
}
pub use error::{Code, Error, Result};
pub use parallel::{
    encode_parallel, encode_parallel_stats, encode_with_chunk, encode_with_chunk_stats,
    ParallelStats, MIN_PARALLEL_CHUNK,
};
