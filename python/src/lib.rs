// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Python bindings for Base91z, built with PyO3 and packaged by maturin.
//!
//! There is no Python implementation of the format: this is a thin layer over
//! the `base91z` crate, so what Python runs is byte for byte the same encoder
//! and decoder the Rust and C callers get. The layer does four things and
//! nothing else -- convert argument types, release the GIL around the call,
//! turn a decode error into a Python exception carrying the specification's
//! error code and offset, and keep the ceiling of Section 16 reachable.

use pyo3::create_exception;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes, PyString};

use base91z::{Code, Error};

create_exception!(
    base91z,
    Base91zDecodeError,
    PyValueError,
    "Raised by decode() on malformed input.\n\n\
     `code` is one of the specification's section 13 conditions, as a\n\
     lower-case string; `position` is the character offset at which the\n\
     condition was detected."
);

/// The specification's name for a condition, which is what a caller should
/// branch on -- the numbers belong to the C ABI, and the wording of the
/// message belongs to whoever reads it.
fn code_name(code: Code) -> &'static str {
    match code {
        Code::InvalidCharacter => "invalid_character",
        Code::UnexpectedEos => "unexpected_end_of_stream",
        Code::UnknownClass => "unknown_class",
        Code::ExtendedClass => "extended_class",
        Code::InvalidFlush => "invalid_flush",
        Code::InvalidParams => "invalid_params",
        Code::InvalidLength => "invalid_length",
        Code::InvalidFinalBlock => "invalid_final_block",
        Code::InvalidIndex => "invalid_index",
        Code::InvalidRunValue => "invalid_run_value",
        Code::MalformedPadding => "malformed_padding",
        Code::MalformedFrame => "malformed_frame",
    }
}

fn decode_error(py: Python<'_>, err: &Error) -> PyErr {
    let py_err = Base91zDecodeError::new_err(err.to_string());
    let value = py_err.value(py);
    // Attribute assignment on a freshly created exception cannot fail; if it
    // somehow did, the error itself is still what matters, so the result is
    // deliberately dropped rather than replacing the real error.
    let _ = value.setattr("code", code_name(err.code));
    let _ = value.setattr("position", err.at);
    py_err
}

/// The bytes of a `bytes` or `bytearray` argument.
///
/// Matched by type rather than by extraction, so that a sequence which merely
/// happens to hold small integers -- a list, a tuple -- is a `TypeError` and
/// not silently an input.
fn byte_argument(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<Vec<u8>> {
    if let Ok(b) = obj.downcast::<PyBytes>() {
        return Ok(b.as_bytes().to_vec());
    }
    if let Ok(b) = obj.downcast::<PyByteArray>() {
        return Ok(b.to_vec());
    }
    Err(PyTypeError::new_err(what.to_string()))
}

/// Encode bytes as a Base91z string, compressing where that is smaller.
///
/// Accepts `bytes` or `bytearray`, and always succeeds: every byte sequence
/// has an encoding, including the empty one.
///
/// `level` is the compression level -- negative encodes faster and larger,
/// higher slower and smaller, and `None` means the format's default. **It is
/// part of the encoding, not of the payload**: the same bytes at two levels
/// give two different strings, both valid and both decoding back to those
/// bytes. Do not use the text as a signature input, a cache key or a fixture;
/// compute those over the payload.
#[pyfunction]
#[pyo3(signature = (data, /, level = None))]
#[pyo3(text_signature = "(data, /, level=None)")]
fn encode<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    level: Option<i32>,
) -> PyResult<Bound<'py, PyString>> {
    let data = byte_argument(data, "encode() expects bytes or bytearray")?;
    // The encoder touches no Python object, so other threads may run while it
    // works. That matters here: this is the call made on a whole file.
    let encoded = py.allow_threads(|| match level {
        // A compressor error is not the caller's problem: there is always a
        // valid uncompressed encoding, which is what the crate falls back to.
        Some(l) => base91z::encode_at(&data, l).unwrap_or_else(|_| base91z::encode_plain(&data)),
        None => base91z::encode(&data),
    });
    Ok(PyString::new(py, &encoded))
}

/// Encode without compressing: the container alone, every typed class except
/// the compressed ones.
///
/// It has no level, so within one version of this library the same bytes
/// always give the same string -- which is what `encode` cannot promise.
///
/// `threads` is a performance knob and nothing else: any value produces the
/// same string, because a worker's output is spliced only where the encoder
/// proves it would have produced the same characters serially. 1 encodes on
/// the calling thread; 0 asks for one worker per available core. Inputs below
/// half a megabyte ignore it -- splitting them costs more than it saves.
#[pyfunction]
#[pyo3(signature = (data, /, threads = 1))]
#[pyo3(text_signature = "(data, /, threads=1)")]
fn encode_plain<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    threads: usize,
) -> PyResult<Bound<'py, PyString>> {
    let data = byte_argument(data, "encode_plain() expects bytes or bytearray")?;
    let threads = if threads == 0 {
        std::thread::available_parallelism().map_or(1, |n| n.get())
    } else {
        threads
    };
    let encoded = py.allow_threads(|| base91z::encode_parallel(&data, threads));
    Ok(PyString::new(py, &encoded))
}

/// Decode a Base91z string back into bytes.
///
/// Accepts `str`, `bytes` or `bytearray`; ASCII is the only encoding a valid
/// stream can be in. Raises `Base91zDecodeError` on malformed input.
///
/// `max_bytes` is a ceiling on the output. **Set it for anything you did not
/// encode yourself**: a length field is a few characters and can declare far
/// more than the stream is long, so a decoder without a ceiling will do what
/// the stream asks. The ceiling is checked before memory is reserved, and a
/// stream that would cross it raises rather than being truncated. `None` uses
/// the library's own default, which is large.
#[pyfunction]
#[pyo3(signature = (s, /, max_bytes = None))]
#[pyo3(text_signature = "(s, /, max_bytes=None)")]
fn decode<'py>(
    py: Python<'py>,
    s: &Bound<'py, PyAny>,
    max_bytes: Option<usize>,
) -> PyResult<Bound<'py, PyBytes>> {
    let text: String = match s.extract::<String>() {
        Ok(text) => text,
        Err(_) => {
            let raw = byte_argument(s, "decode() expects str, bytes or bytearray")?;
            // Bytes are bytes, and the format is defined over them: every
            // alphabet character is ASCII, so a byte from 0x80 up is one
            // significant character outside the alphabet. Mapping each byte to
            // the character of the same value is the identity on ASCII and
            // keeps every reported offset counting what the caller sent.
            raw.iter().map(|&b| b as char).collect::<String>()
        }
    };

    let result = py.allow_threads(|| match max_bytes {
        Some(max) => base91z::decode_bounded(&text, max),
        None => base91z::decode(&text),
    });
    match result {
        Ok(bytes) => Ok(PyBytes::new(py, &bytes)),
        Err(e) => Err(decode_error(py, &e)),
    }
}

#[pymodule]
#[pyo3(name = "base91z")]
fn base91z_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    use base91z::tables as t;

    m.add_function(wrap_pyfunction!(encode, m)?)?;
    m.add_function(wrap_pyfunction!(encode_plain, m)?)?;
    m.add_function(wrap_pyfunction!(decode, m)?)?;
    m.add("Base91zDecodeError", m.py().get_type::<Base91zDecodeError>())?;

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("SPEC_VERSION", "0.4.0")?;
    m.add("HAS_COMPRESSOR", cfg!(feature = "zstd"))?;
    m.add("DEFAULT_LEVEL", default_level())?;

    // Section 4: the alphabet, so tooling has one source for it rather than a
    // transcribed copy.
    m.add("ALPHABET", std::str::from_utf8(t::ALPHABET).unwrap())?;

    // Section 11.4: the bounds a decoder enforces per class. A caller sizing
    // its own `max_bytes` wants these.
    m.add("MAX_SEGMENT_BYTES", t::MAX_SEGMENT_BYTES)?;
    m.add("MAX_FRAME_BYTES", t::MAX_FRAME_BYTES)?;
    m.add("MAX_BLOCK_BYTES", t::MAX_BLOCK_BYTES)?;
    m.add("MAX_FRAME_PLAIN_BYTES", t::MAX_FRAME_PLAIN_BYTES)?;
    m.add("PARALLEL_ALIGN", t::PARALLEL_ALIGN)?;

    m.add(
        "__all__",
        vec![
            "__version__",
            "encode",
            "encode_plain",
            "decode",
            "Base91zDecodeError",
            "ALPHABET",
            "SPEC_VERSION",
            "HAS_COMPRESSOR",
            "DEFAULT_LEVEL",
            "MAX_SEGMENT_BYTES",
            "MAX_FRAME_BYTES",
            "MAX_BLOCK_BYTES",
            "MAX_FRAME_PLAIN_BYTES",
            "PARALLEL_ALIGN",
        ],
    )?;
    Ok(())
}

#[cfg(feature = "zstd")]
fn default_level() -> i32 {
    base91z::DEFAULT_LEVEL
}

#[cfg(not(feature = "zstd"))]
fn default_level() -> i32 {
    1
}
