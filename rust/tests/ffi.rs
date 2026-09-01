// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The C ABI, called as C calls it, and the header checked against it.
//!
//! `examples/c/demo.c` links the real library and is what proves the header
//! compiles and the symbols resolve. This is the other half: it runs in
//! `cargo test`, on every push, without a C compiler -- and it reads
//! `include/base91z.h` as text to check that every status code in it has the
//! same number as the Rust enum. A header is a promise about an ABI, and the
//! way that promise breaks is silently: renumber the enum, and every C caller
//! built against the old header misreads every error until someone recompiles.

use std::ffi::{c_char, c_void, CStr};
use std::ptr;

use base91z::ffi::*;

extern "C" {
    fn free(p: *mut c_void);
}

fn encode_ffi(data: &[u8]) -> Result<String, base91z_status> {
    let mut out: *mut c_char = ptr::null_mut();
    let mut len = 0usize;
    let st = unsafe { base91z_encode(data.as_ptr(), data.len(), &mut out, &mut len) };
    if st != base91z_status::BASE91Z_OK {
        return Err(st);
    }
    let bytes = unsafe { std::slice::from_raw_parts(out as *const u8, len) };
    let s = String::from_utf8(bytes.to_vec()).expect("the encoder emits ASCII");
    // The terminator is there, and is not counted in the length.
    assert_eq!(unsafe { *out.add(len) }, 0, "output is not NUL-terminated");
    unsafe { free(out as *mut c_void) };
    Ok(s)
}

fn decode_ffi(text: &str, budget: Option<usize>) -> Result<Vec<u8>, (base91z_status, usize)> {
    let mut out: *mut u8 = ptr::null_mut();
    let mut len = 0usize;
    let mut at = usize::MAX;
    let st = unsafe {
        match budget {
            Some(max) => base91z_decode_bounded(
                text.as_ptr() as *const c_char,
                text.len(),
                max,
                &mut out,
                &mut len,
                &mut at,
            ),
            None => base91z_decode(
                text.as_ptr() as *const c_char,
                text.len(),
                &mut out,
                &mut len,
                &mut at,
            ),
        }
    };
    if st != base91z_status::BASE91Z_OK {
        assert!(out.is_null(), "an error left an allocation behind");
        return Err((st, at));
    }
    assert!(!out.is_null(), "a successful decode returned no pointer");
    let bytes = unsafe { std::slice::from_raw_parts(out, len) }.to_vec();
    unsafe { free(out as *mut c_void) };
    Ok(bytes)
}

#[test]
fn round_trip_through_the_c_boundary() {
    for case in [
        &b""[..],
        b"a",
        b"{\"user\":\"ada\",\"id\":42}",
        &[0u8; 300][..],
        &(0..=255u8).collect::<Vec<_>>()[..],
    ] {
        let text = encode_ffi(case).expect("encode");
        assert_eq!(decode_ffi(&text, None).expect("decode"), case);
        assert_eq!(
            decode_ffi(&text, Some(1 << 20)).expect("bounded decode"),
            case
        );
    }
}

#[test]
fn the_empty_input_is_a_real_pointer() {
    // Not null, and freeable: a caller that branches on null for "no output"
    // would otherwise treat the empty payload as an error.
    let mut out: *mut c_char = ptr::null_mut();
    let mut len = 1usize;
    let st = unsafe { base91z_encode(ptr::null(), 0, &mut out, &mut len) };
    assert_eq!(st, base91z_status::BASE91Z_OK);
    assert!(!out.is_null());
    assert_eq!(len, 0);
    unsafe { free(out as *mut c_void) };
}

#[test]
fn bad_arguments_are_refused_rather_than_dereferenced() {
    let mut len = 0usize;
    let mut out: *mut c_char = ptr::null_mut();
    // A null out-parameter.
    assert_eq!(
        unsafe { base91z_encode(b"x".as_ptr(), 1, ptr::null_mut(), &mut len) },
        base91z_status::BASE91Z_ERR_INVALID_ARGUMENT
    );
    // A null input with a length that says otherwise.
    assert_eq!(
        unsafe { base91z_encode(ptr::null(), 7, &mut out, &mut len) },
        base91z_status::BASE91Z_ERR_INVALID_ARGUMENT
    );
    let mut bytes: *mut u8 = ptr::null_mut();
    assert_eq!(
        unsafe { base91z_decode(ptr::null(), 7, &mut bytes, &mut len, ptr::null_mut()) },
        base91z_status::BASE91Z_ERR_INVALID_ARGUMENT
    );
}

#[test]
fn a_decode_failure_reports_its_code_and_offset() {
    // A quotation mark is not in the alphabet, and it is at character zero.
    let (st, at) = decode_ffi("\"nope", None).expect_err("must be refused");
    assert_eq!(st, base91z_status::BASE91Z_ERR_INVALID_CHARACTER);
    assert_eq!(at, 0);
}

#[test]
fn the_budget_is_reachable_from_c() {
    // Nine characters that declare 65 536 bytes of zero run: the case the
    // ceiling exists for, and the reason `decode_bounded` is exported at all.
    const BOMB: &str = "m----Y]GA";
    let (st, _) = decode_ffi(BOMB, Some(1000)).expect_err("must not pass a 1 000-byte budget");
    assert_eq!(st, base91z_status::BASE91Z_ERR_INVALID_LENGTH);
    assert_eq!(decode_ffi(BOMB, Some(1 << 20)).expect("fits").len(), 65_536);
}

#[test]
fn a_null_free_is_a_no_op() {
    unsafe { base91z_free(ptr::null_mut()) };
}

#[test]
fn every_status_has_a_description() {
    use base91z_status::*;
    for st in [
        BASE91Z_OK,
        BASE91Z_ERR_INVALID_CHARACTER,
        BASE91Z_ERR_UNEXPECTED_EOS,
        BASE91Z_ERR_UNKNOWN_CLASS,
        BASE91Z_ERR_EXTENDED_CLASS,
        BASE91Z_ERR_INVALID_FLUSH,
        BASE91Z_ERR_INVALID_PARAMS,
        BASE91Z_ERR_INVALID_LENGTH,
        BASE91Z_ERR_INVALID_FINAL_BLOCK,
        BASE91Z_ERR_INVALID_INDEX,
        BASE91Z_ERR_INVALID_RUN_VALUE,
        BASE91Z_ERR_MALFORMED_PADDING,
        BASE91Z_ERR_MALFORMED_FRAME,
        BASE91Z_ERR_ALLOC,
        BASE91Z_ERR_INVALID_ARGUMENT,
        BASE91Z_ERR_NO_COMPRESSOR,
    ] {
        let s = unsafe { CStr::from_ptr(base91z_strerror(st)) }
            .to_str()
            .expect("descriptions are ASCII");
        assert!(!s.is_empty(), "{st:?} has no description");
    }
}

#[test]
fn the_versions_are_reported() {
    let spec = unsafe { CStr::from_ptr(base91z_spec_version()) }.to_str().unwrap();
    assert_eq!(spec, "0.4.0");
    let version = unsafe { CStr::from_ptr(base91z_version()) }.to_str().unwrap();
    assert_eq!(version, env!("CARGO_PKG_VERSION"));
    assert_eq!(base91z_has_compressor(), cfg!(feature = "zstd"));
}

/// The header and the enum agree, name for name and number for number.
///
/// This is the one thing a C caller cannot check for itself: it compiles
/// against the header and links against the library, and if the two disagree
/// about which number means which failure, everything still builds.
#[test]
fn the_header_numbers_match_the_enum() {
    let header = include_str!("../include/base91z.h");
    let expected: &[(&str, i32)] = &[
        ("BASE91Z_OK", 0),
        ("BASE91Z_ERR_INVALID_CHARACTER", 1),
        ("BASE91Z_ERR_UNEXPECTED_EOS", 2),
        ("BASE91Z_ERR_UNKNOWN_CLASS", 3),
        ("BASE91Z_ERR_EXTENDED_CLASS", 4),
        ("BASE91Z_ERR_INVALID_FLUSH", 5),
        ("BASE91Z_ERR_INVALID_PARAMS", 6),
        ("BASE91Z_ERR_INVALID_LENGTH", 7),
        ("BASE91Z_ERR_INVALID_FINAL_BLOCK", 8),
        ("BASE91Z_ERR_INVALID_INDEX", 9),
        ("BASE91Z_ERR_INVALID_RUN_VALUE", 10),
        ("BASE91Z_ERR_MALFORMED_PADDING", 11),
        ("BASE91Z_ERR_MALFORMED_FRAME", 12),
        ("BASE91Z_ERR_ALLOC", 13),
        ("BASE91Z_ERR_INVALID_ARGUMENT", 14),
        ("BASE91Z_ERR_NO_COMPRESSOR", 15),
    ];
    // What the Rust enum says, by construction: the discriminants are what a
    // C caller receives.
    let rust: Vec<i32> = vec![
        base91z_status::BASE91Z_OK as i32,
        base91z_status::BASE91Z_ERR_INVALID_CHARACTER as i32,
        base91z_status::BASE91Z_ERR_UNEXPECTED_EOS as i32,
        base91z_status::BASE91Z_ERR_UNKNOWN_CLASS as i32,
        base91z_status::BASE91Z_ERR_EXTENDED_CLASS as i32,
        base91z_status::BASE91Z_ERR_INVALID_FLUSH as i32,
        base91z_status::BASE91Z_ERR_INVALID_PARAMS as i32,
        base91z_status::BASE91Z_ERR_INVALID_LENGTH as i32,
        base91z_status::BASE91Z_ERR_INVALID_FINAL_BLOCK as i32,
        base91z_status::BASE91Z_ERR_INVALID_INDEX as i32,
        base91z_status::BASE91Z_ERR_INVALID_RUN_VALUE as i32,
        base91z_status::BASE91Z_ERR_MALFORMED_PADDING as i32,
        base91z_status::BASE91Z_ERR_MALFORMED_FRAME as i32,
        base91z_status::BASE91Z_ERR_ALLOC as i32,
        base91z_status::BASE91Z_ERR_INVALID_ARGUMENT as i32,
        base91z_status::BASE91Z_ERR_NO_COMPRESSOR as i32,
    ];

    for (i, (name, value)) in expected.iter().enumerate() {
        assert_eq!(rust[i], *value, "the Rust enum moved {name}");
        let line = format!("{name} = {value}");
        assert!(
            header.contains(&line),
            "include/base91z.h does not say `{line}`"
        );
    }

    // And every symbol the module exports is declared in the header, so a new
    // entry point cannot ship without one.
    for symbol in [
        "base91z_encode",
        "base91z_encode_at",
        "base91z_encode_plain",
        "base91z_decode",
        "base91z_decode_bounded",
        "base91z_free",
        "base91z_strerror",
        "base91z_spec_version",
        "base91z_version",
        "base91z_has_compressor",
    ] {
        assert!(
            header.contains(symbol),
            "include/base91z.h does not declare {symbol}"
        );
    }

    // The default level is a number in two places; they are the same number.
    assert!(
        header.contains(&format!("BASE91Z_DEFAULT_LEVEL {}", default_level())),
        "the header's BASE91Z_DEFAULT_LEVEL is not the crate's"
    );
}

#[cfg(feature = "zstd")]
fn default_level() -> i32 {
    base91z::DEFAULT_LEVEL
}

#[cfg(not(feature = "zstd"))]
fn default_level() -> i32 {
    1
}
