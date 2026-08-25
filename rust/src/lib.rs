// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The `simd` feature is nightly-only, and deliberately the only thing in this
// crate that is: see src/simd.rs and the README section it points at.
#![cfg_attr(feature = "simd", feature(portable_simd))]

//! # base91-jdp
//!
//! A prototype encoder and decoder for **specification v0.4.0** -- basE91 on
//! an alphabet JSON never has to escape, with typed segments.
//!
//! ```
//! let data = b"{\"user\":\"ada\",\"id\":42}";
//! let text = base91_jdp::encode(data);
//! assert_eq!(base91_jdp::decode(&text).unwrap(), data);
//! ```
//!
//! **This is a prototype.** It implements the passthrough, packed-base and run
//! classes of the specification and the block coder under them; it does not
//! implement class 20, the zstd segment, and rejects it on decode with
//! `Code::Unsupported`. What it exists to answer is whether the format encodes
//! at the density the specification projects, and whether the parallel and
//! vector paths the format was shaped for actually pay.

pub mod error;
pub mod tables;

mod decode;
mod encode;
mod parallel;
mod symbols;

#[cfg(feature = "simd")]
pub mod simd;

/// Entry points the benchmarks use to time one layer at a time. Not part of
/// the format, and not a stable interface.
pub mod bench {
    pub use crate::encode::block_only;
}

pub use decode::{decode, decode_bounded};
pub use encode::encode;
pub use error::{Code, Error, Result};
pub use parallel::{
    encode_parallel, encode_parallel_stats, encode_with_chunk, encode_with_chunk_stats,
    ParallelStats, MIN_PARALLEL_CHUNK,
};
