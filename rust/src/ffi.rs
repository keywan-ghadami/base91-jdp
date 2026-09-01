// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! C ABI for this crate: the encoder and decoder behind the C calling
//! convention, declared in `include/base91z.h`.
//!
//! The point is that a C, C++, Python, Ruby, Go, Zig, Java or any other
//! FFI-capable caller gets *this* implementation rather than a second one
//! written for that language. Everything below the exported functions is safe
//! Rust: the parsing of attacker-controlled input, which is the part worth
//! protecting, is bounds-checked by the compiler, and the ceiling of
//! specification Section 16 is reachable from C through
//! [`base91z_decode_bounded`].
//!
//! # Contract
//!
//! No Rust type crosses the boundary, no pointer the caller passes in is
//! retained after a call returns, and the library holds no global state.
//! Output buffers are allocated with the C `malloc()` rather than with Rust's
//! allocator, so a caller releases them with [`base91z_free`] -- or with
//! `free()`, which is the same thing everywhere the two link the same C
//! runtime. Prefer [`base91z_free`]: on Windows a DLL and its caller can hold
//! separate heaps, and then only the library can free what the library
//! allocated.
//!
//! # Failure behaviour
//!
//! Every entry point is `extern "C"`, so a panic inside one aborts the process
//! rather than unwinding into foreign frames (Rust guarantees this since 1.71).
//! No panic is expected: encoding is total, and decoding returns its error
//! conditions as values. A failure of Rust's *internal* allocator also aborts,
//! which is Rust's global policy and not something this layer can turn into
//! `BASE91Z_ERR_ALLOC`; only the allocation of the caller-owned output buffer
//! is reported that way.

#![allow(non_camel_case_types)]

use core::ffi::{c_char, c_int, c_void, CStr};
use core::slice;

use crate::error::Code;

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
}

/// Status codes. `BASE91Z_OK` is zero and every failure is positive, so
/// `if (base91z_decode(...))` reads as "if it failed".
///
/// The twelve decode conditions are the codes of specification Section 13, in
/// its order; the last two are this boundary's own.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum base91z_status {
    BASE91Z_OK = 0,
    BASE91Z_ERR_INVALID_CHARACTER = 1,
    BASE91Z_ERR_UNEXPECTED_EOS = 2,
    BASE91Z_ERR_UNKNOWN_CLASS = 3,
    BASE91Z_ERR_EXTENDED_CLASS = 4,
    BASE91Z_ERR_INVALID_FLUSH = 5,
    BASE91Z_ERR_INVALID_PARAMS = 6,
    BASE91Z_ERR_INVALID_LENGTH = 7,
    BASE91Z_ERR_INVALID_FINAL_BLOCK = 8,
    BASE91Z_ERR_INVALID_INDEX = 9,
    BASE91Z_ERR_INVALID_RUN_VALUE = 10,
    BASE91Z_ERR_MALFORMED_PADDING = 11,
    BASE91Z_ERR_MALFORMED_FRAME = 12,
    /// The caller-owned output buffer could not be allocated.
    BASE91Z_ERR_ALLOC = 13,
    /// A null pointer where one is not allowed, or a length that contradicts
    /// it. Nothing was read and nothing was written.
    BASE91Z_ERR_INVALID_ARGUMENT = 14,
    /// The crate was built without the `zstd` feature, so classes 17 and 20
    /// are not available and a level cannot be honoured.
    BASE91Z_ERR_NO_COMPRESSOR = 15,
}

use base91z_status::*;

impl From<Code> for base91z_status {
    fn from(c: Code) -> Self {
        match c {
            Code::InvalidCharacter => BASE91Z_ERR_INVALID_CHARACTER,
            Code::UnexpectedEos => BASE91Z_ERR_UNEXPECTED_EOS,
            Code::UnknownClass => BASE91Z_ERR_UNKNOWN_CLASS,
            Code::ExtendedClass => BASE91Z_ERR_EXTENDED_CLASS,
            Code::InvalidFlush => BASE91Z_ERR_INVALID_FLUSH,
            Code::InvalidParams => BASE91Z_ERR_INVALID_PARAMS,
            Code::InvalidLength => BASE91Z_ERR_INVALID_LENGTH,
            Code::InvalidFinalBlock => BASE91Z_ERR_INVALID_FINAL_BLOCK,
            Code::InvalidIndex => BASE91Z_ERR_INVALID_INDEX,
            Code::InvalidRunValue => BASE91Z_ERR_INVALID_RUN_VALUE,
            Code::MalformedPadding => BASE91Z_ERR_MALFORMED_PADDING,
            Code::MalformedFrame => BASE91Z_ERR_MALFORMED_FRAME,
        }
    }
}

/// Copy `bytes` into a `malloc`'d buffer, appending a NUL that is not counted
/// in the reported length.
///
/// Returns null on allocation failure. `nul` is what makes the encode path
/// hand back a C string while the decode path hands back exactly the bytes it
/// decoded. The extra byte is also what lets a zero-length result be a real
/// pointer the caller can free.
fn malloc_copy(bytes: &[u8], nul: bool) -> *mut u8 {
    let size = match bytes.len().checked_add(1) {
        Some(n) => n,
        None => return core::ptr::null_mut(),
    };
    // SAFETY: `size` is non-zero, and `malloc` returns either a buffer of at
    // least `size` writable bytes or null, which is checked below.
    let p = unsafe { malloc(size) } as *mut u8;
    if p.is_null() {
        return p;
    }
    // SAFETY: `p` points to `bytes.len() + 1` writable bytes, and a fresh
    // allocation cannot overlap the live slice `bytes`.
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        if nul {
            p.add(bytes.len()).write(0);
        }
    }
    p
}

/// The input slice for a (pointer, length) pair, or `None` if the pair is not
/// one this boundary accepts.
///
/// # Safety
///
/// `ptr` must point to `len` readable bytes, or be null when `len` is 0.
unsafe fn input<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if ptr.is_null() {
        return if len == 0 { Some(&[]) } else { None };
    }
    // SAFETY: the caller guarantees `len` readable bytes at `ptr`.
    Some(unsafe { slice::from_raw_parts(ptr, len) })
}

/// Hand an encoded string to the caller.
fn give_string(text: &str, out_str: *mut *mut c_char, out_len: *mut usize) -> base91z_status {
    let buf = malloc_copy(text.as_bytes(), true);
    if buf.is_null() {
        return BASE91Z_ERR_ALLOC;
    }
    // SAFETY: the out-pointers were checked non-null by the caller of this
    // helper, which the caller of *that* guarantees are writable.
    unsafe {
        *out_str = buf as *mut c_char;
        *out_len = text.len();
    }
    BASE91Z_OK
}

// --------------------------------------------------------------- encoding --

/// Encode `data_len` bytes, compressing where that is smaller.
///
/// This is the entry point: compression is part of the format rather than a
/// stage in front of it, so the default encode uses it and the same
/// [`base91z_decode`] reads whatever it chose. The level is
/// `BASE91Z_DEFAULT_LEVEL`.
///
/// Total: every byte sequence has an encoding, so this fails only on a bad
/// argument or on the output allocation.
///
/// On `BASE91Z_OK`, `*out_str` receives a `malloc`'d NUL-terminated buffer the
/// caller owns and releases with [`base91z_free`], and `*out_len` its length
/// excluding the terminator. On any error both are left untouched.
///
/// # Safety
///
/// `data` must point to `data_len` readable bytes, or be null when `data_len`
/// is 0. `out_str` and `out_len` must be non-null and writable.
#[no_mangle]
pub unsafe extern "C" fn base91z_encode(
    data: *const u8,
    data_len: usize,
    out_str: *mut *mut c_char,
    out_len: *mut usize,
) -> base91z_status {
    if out_str.is_null() || out_len.is_null() {
        return BASE91Z_ERR_INVALID_ARGUMENT;
    }
    // SAFETY: the caller's guarantee about `data` and `data_len`.
    let Some(input) = (unsafe { input(data, data_len) }) else {
        return BASE91Z_ERR_INVALID_ARGUMENT;
    };
    give_string(&crate::encode(input), out_str, out_len)
}

/// Encode at an explicit compression level.
///
/// Negative levels encode faster and larger, higher ones slower and smaller.
/// **The level is part of the encoding, not of the payload**: the same bytes at
/// two levels give two different strings, both valid and both decoding back to
/// those bytes. Anything that treats the text as an identity -- a signature, a
/// cache key -- wants [`base91z_encode_plain`] at most, and really wants to be
/// computed over the payload instead.
///
/// Returns `BASE91Z_ERR_NO_COMPRESSOR` if the library was built without the
/// `zstd` feature.
///
/// # Safety
///
/// As [`base91z_encode`].
#[no_mangle]
pub unsafe extern "C" fn base91z_encode_at(
    data: *const u8,
    data_len: usize,
    level: c_int,
    out_str: *mut *mut c_char,
    out_len: *mut usize,
) -> base91z_status {
    if out_str.is_null() || out_len.is_null() {
        return BASE91Z_ERR_INVALID_ARGUMENT;
    }
    // SAFETY: the caller's guarantee about `data` and `data_len`.
    let Some(input) = (unsafe { input(data, data_len) }) else {
        return BASE91Z_ERR_INVALID_ARGUMENT;
    };
    #[cfg(feature = "zstd")]
    {
        match crate::encode_at(input, level) {
            // A compressor error is not the caller's problem to handle: there
            // is always a valid uncompressed encoding, which is what `encode`
            // falls back to, and this does the same.
            Ok(text) => give_string(&text, out_str, out_len),
            Err(_) => give_string(&crate::encode_plain(input), out_str, out_len),
        }
    }
    #[cfg(not(feature = "zstd"))]
    {
        let _ = (input, level);
        BASE91Z_ERR_NO_COMPRESSOR
    }
}

/// Encode without compressing: the container alone, every typed class except
/// the compressed ones.
///
/// Slower to shrink and faster to produce, and -- unlike the two above -- it
/// has no level, so within one version of this library the same bytes always
/// give the same string.
///
/// # Safety
///
/// As [`base91z_encode`].
#[no_mangle]
pub unsafe extern "C" fn base91z_encode_plain(
    data: *const u8,
    data_len: usize,
    out_str: *mut *mut c_char,
    out_len: *mut usize,
) -> base91z_status {
    if out_str.is_null() || out_len.is_null() {
        return BASE91Z_ERR_INVALID_ARGUMENT;
    }
    // SAFETY: the caller's guarantee about `data` and `data_len`.
    let Some(input) = (unsafe { input(data, data_len) }) else {
        return BASE91Z_ERR_INVALID_ARGUMENT;
    };
    give_string(&crate::encode_plain(input), out_str, out_len)
}

// --------------------------------------------------------------- decoding --

/// Decode `s_len` characters back into bytes, with no ceiling of the caller's
/// own.
///
/// Equivalent to [`base91z_decode_bounded`] with the library's default budget.
/// **On input from anywhere you do not control, use that one instead**: a
/// length field is four characters and can ask for far more memory than the
/// stream is long, and a budget is how a caller says how much of that it is
/// willing to spend (specification Section 16).
///
/// # Safety
///
/// `s` must point to `s_len` readable bytes, or be null when `s_len` is 0.
/// `out_data` and `out_len` must be non-null and writable. `out_error_at` may
/// be null; when it is not, it is written only on a decode failure.
#[no_mangle]
pub unsafe extern "C" fn base91z_decode(
    s: *const c_char,
    s_len: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
    out_error_at: *mut usize,
) -> base91z_status {
    // SAFETY: the caller's guarantees, forwarded unchanged.
    unsafe { decode_impl(s, s_len, None, out_data, out_len, out_error_at) }
}

/// Decode, refusing to produce more than `max_bytes`.
///
/// The budget is a ceiling on the *output*, checked before anything is
/// reserved for it: a run class, a packed class and a compressed segment each
/// declare a length, and a declaration is not evidence. A stream that would
/// cross the ceiling is refused with `BASE91Z_ERR_INVALID_LENGTH` rather than
/// truncated, so a short read is never mistaken for a short payload.
///
/// # Safety
///
/// As [`base91z_decode`].
#[no_mangle]
pub unsafe extern "C" fn base91z_decode_bounded(
    s: *const c_char,
    s_len: usize,
    max_bytes: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
    out_error_at: *mut usize,
) -> base91z_status {
    // SAFETY: the caller's guarantees, forwarded unchanged.
    unsafe { decode_impl(s, s_len, Some(max_bytes), out_data, out_len, out_error_at) }
}

/// # Safety
///
/// As [`base91z_decode`].
unsafe fn decode_impl(
    s: *const c_char,
    s_len: usize,
    budget: Option<usize>,
    out_data: *mut *mut u8,
    out_len: *mut usize,
    out_error_at: *mut usize,
) -> base91z_status {
    if out_data.is_null() || out_len.is_null() {
        return BASE91Z_ERR_INVALID_ARGUMENT;
    }
    // SAFETY: the caller's guarantee about `s` and `s_len`; `c_char` and `u8`
    // have the same layout, and the text is read as bytes rather than as a
    // NUL-terminated string.
    let Some(bytes) = (unsafe { input(s as *const u8, s_len) }) else {
        return BASE91Z_ERR_INVALID_ARGUMENT;
    };

    // Bytes, not UTF-8. Every alphabet character is ASCII, so a byte from 0x80
    // up is one significant character outside the alphabet -- one character per
    // byte, which is the count every offset in an error message is against.
    // Reading the input as UTF-8 instead would reject a stray byte as an
    // invalid character before the structural checks that must come first, and
    // would count one character where a well-formed multi-byte sequence is two
    // or three to every other reader of the same buffer.
    let owned;
    let text = match core::str::from_utf8(bytes) {
        Ok(t) if bytes.is_ascii() => t,
        _ => {
            owned = bytes.iter().map(|&b| b as char).collect::<String>();
            owned.as_str()
        }
    };

    let result = match budget {
        Some(max) => crate::decode_bounded(text, max),
        None => crate::decode(text),
    };
    let decoded = match result {
        Ok(d) => d,
        Err(e) => {
            if !out_error_at.is_null() {
                // SAFETY: checked non-null, and the caller guarantees it is
                // writable.
                unsafe { *out_error_at = e.at };
            }
            return e.code.into();
        }
    };

    let buf = malloc_copy(&decoded, false);
    if buf.is_null() {
        return BASE91Z_ERR_ALLOC;
    }
    // SAFETY: both out-pointers were checked non-null above, and the caller
    // guarantees they are writable.
    unsafe {
        *out_data = buf;
        *out_len = decoded.len();
    }
    BASE91Z_OK
}

// ------------------------------------------------------------ housekeeping --

/// Release a buffer handed back by any of the functions above.
///
/// `free()` does the same thing wherever the caller and this library share a C
/// runtime. This exists for where they do not -- a DLL on Windows can hold its
/// own heap -- and costs nothing to prefer. A null pointer is accepted and
/// ignored.
///
/// # Safety
///
/// `p` must be null, or a pointer this library returned and that has not been
/// freed already.
#[no_mangle]
pub unsafe extern "C" fn base91z_free(p: *mut c_void) {
    if !p.is_null() {
        // SAFETY: the caller guarantees `p` came from this library's `malloc`
        // and has not been freed.
        unsafe { free(p) };
    }
}

/// A short, static, human-readable description of a status code.
///
/// The pointer is to static storage: it is never freed and stays valid for the
/// lifetime of the process.
#[no_mangle]
pub extern "C" fn base91z_strerror(status: base91z_status) -> *const c_char {
    let s: &'static CStr = match status {
        BASE91Z_OK => c"ok",
        BASE91Z_ERR_INVALID_CHARACTER => c"character not in the alphabet",
        BASE91Z_ERR_UNEXPECTED_EOS => c"stream ended inside a field",
        BASE91Z_ERR_UNKNOWN_CLASS => c"a class this version does not define",
        BASE91Z_ERR_EXTENDED_CLASS => c"the escape, which this version does not implement",
        BASE91Z_ERR_INVALID_FLUSH => c"invalid flush field",
        BASE91Z_ERR_INVALID_PARAMS => c"invalid segment parameters",
        BASE91Z_ERR_INVALID_LENGTH => c"invalid length, or the output ceiling exceeded",
        BASE91Z_ERR_INVALID_FINAL_BLOCK => c"bits owed at end of stream",
        BASE91Z_ERR_INVALID_INDEX => c"index outside the packed alphabet",
        BASE91Z_ERR_INVALID_RUN_VALUE => c"run value zero, or above 255",
        BASE91Z_ERR_MALFORMED_PADDING => c"malformed padding",
        BASE91Z_ERR_MALFORMED_FRAME => c"the decompressor refused the frame",
        BASE91Z_ERR_ALLOC => c"memory allocation failure",
        BASE91Z_ERR_INVALID_ARGUMENT => c"invalid argument",
        BASE91Z_ERR_NO_COMPRESSOR => c"built without the compressed classes",
    };
    s.as_ptr()
}

/// The specification version this library implements, as a NUL-terminated
/// static string.
#[no_mangle]
pub extern "C" fn base91z_spec_version() -> *const c_char {
    c"0.4.0".as_ptr()
}

/// This library's own version, as a NUL-terminated static string.
#[no_mangle]
pub extern "C" fn base91z_version() -> *const c_char {
    // `c"..."` needs a literal, and `env!` is one by the time it is expanded.
    const V: &CStr = match CStr::from_bytes_with_nul(
        concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes(),
    ) {
        Ok(v) => v,
        Err(_) => panic!("CARGO_PKG_VERSION contains a NUL"),
    };
    V.as_ptr()
}

/// Whether the library was built with the compressed classes (17 and 20).
///
/// Without them `base91z_encode` still encodes everything, using the container
/// alone, and `base91z_decode` refuses a stream that contains one.
#[no_mangle]
pub extern "C" fn base91z_has_compressor() -> bool {
    cfg!(feature = "zstd")
}
