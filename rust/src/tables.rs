// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The frozen tables of specification v0.4.0: the alphabet, the R-Set, the
//! donor profiles, and the segment classes.

/// The 91 characters, in value order. Specification section 4.
pub const ALPHABET: &[u8; 91] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!#$%&()*+,./:;<=>?@[]^_`{|}~-";

/// Byte value -> alphabet value, or 0xFF for a byte the alphabet does not hold.
pub static VALUE_OF: [u8; 256] = {
    let mut t = [0xFFu8; 256];
    let mut i = 0;
    while i < 91 {
        t[ALPHABET[i] as usize] = i as u8;
        i += 1;
    }
    t
};

pub const SYMBOL_BITS: u32 = 13;
pub const SYMBOL_MAX: u16 = 8192; // first value that is not a symbol
pub const ESCAPE_PAIR: u16 = 8280; // "--"
pub const SIGNAL_MIN: u16 = 8192;
pub const SIGNAL_MAX: u16 = 8279;

/// Bytes per whole symbol group: 13 bytes are 104 bits are 8 symbols.
/// A block-mode split here needs no seam -- specification section 14.5.
pub const PARALLEL_ALIGN: usize = 13;

pub const MIN_BINARY_RUN: usize = 0;

/// The shortest run of one repeated byte that ends a passthrough or packed
/// prefix, so that a run class can carry it instead.
///
/// Without this the prefix scan is greedy and swallows runs whole: passthrough
/// carries a zero byte at one character each -- NUL is an R-Set member -- where
/// `ZRUN` carries eighty-nine of them in three characters. Breaking out costs
/// the run's own segment and the one that resumes afterwards, so the threshold
/// is where that stops being worth it. Measured by `examples/sweep.rs`.
pub const MIN_RUN_IN_SEGMENT: usize = 8;

/// The same for a run of a byte other than zero, which needs `RUN` and its
/// extra pair rather than `ZRUN`, and therefore has to be longer to pay.
pub const MIN_NONZERO_RUN_IN_SEGMENT: usize = 8;

/// The three thresholds above, as the sweep of `examples/sweep.rs` sets them.
/// A prototype exists to find out what they should be; the constants are what
/// it found.
pub mod tuning {
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    pub static BINARY_RUN: AtomicUsize = AtomicUsize::new(super::MIN_BINARY_RUN);
    pub static ZERO_RUN: AtomicUsize = AtomicUsize::new(super::MIN_RUN_IN_SEGMENT);
    pub static NONZERO_RUN: AtomicUsize = AtomicUsize::new(super::MIN_NONZERO_RUN_IN_SEGMENT);

    #[inline]
    pub fn binary_run() -> usize {
        BINARY_RUN.load(Relaxed)
    }
    #[inline]
    pub fn run_break(zero: bool) -> usize {
        if zero { ZERO_RUN.load(Relaxed) } else { NONZERO_RUN.load(Relaxed) }
    }
    /// Which candidate families the scan considers. For the benchmark only:
    /// turning one off changes the encoding, so this is not a knob a caller
    /// has, only a way to find out where the time goes.
    pub static FAMILIES: AtomicUsize = AtomicUsize::new(0b111);
    pub const F_RUN: usize = 1;
    pub const F_PACKED: usize = 2;
    pub const F_PT: usize = 4;

    #[inline]
    pub fn families() -> usize {
        FAMILIES.load(Relaxed)
    }

    /// Put every threshold back where the specification has it.
    pub fn reset() {
        BINARY_RUN.store(super::MIN_BINARY_RUN, Relaxed);
        ZERO_RUN.store(super::MIN_RUN_IN_SEGMENT, Relaxed);
        NONZERO_RUN.store(super::MIN_NONZERO_RUN_IN_SEGMENT, Relaxed);
        FAMILIES.store(0b111, Relaxed);
    }
}
pub const MAX_SEGMENT_BYTES: usize = 65_536;
pub const R_LEN: usize = 8;

/// The R-Set, in the index order that fixes the bits of `mask`.
/// Specification section 8.1: seven text characters, then NUL.
pub const R_CHARS: [u8; R_LEN] = [0x20, 0x22, 0x0A, 0x5C, 0x0D, 0x27, 0x09, 0x00];

/// Donor profiles, specification section 8.2. Eight ranks each.
pub const PROFILES: [[u8; 8]; 4] = [
    *b"$~^%#@><",
    *b"@&!~%<$^",
    *b"%@#<~>$^",
    *b"*$?&^|~%",
];
pub const NUM_PROFILES: usize = PROFILES.len();

// ---------------------------------------------------------------------------
// Classes
// ---------------------------------------------------------------------------

pub const CLASS_PT: u16 = 0;
pub const CLASS_PT0: u16 = 1;
pub const CLASS_PACKED_FIRST: u16 = 7;
pub const CLASS_PACKED_LAST: u16 = 19;
pub const CLASS_ZSTD: u16 = 20;
pub const CLASS_ZRUN: u16 = 21;
pub const CLASS_RUN: u16 = 22;
pub const CLASS_ZMIX_FIRST: u16 = 23;
pub const CLASS_ZMIX_LAST: u16 = 30;
pub const CLASS_MAX_DEFINED: u16 = 30;

/// The passthrough shorthands of classes 1..=6: the mask each one implies,
/// all with profile 0. Index is `class - 1`.
pub const SHORTHAND_MASK: [u8; 6] = [
    0b0000_0000, // 1 PT0    {}
    0b0000_0001, // 2 PT_S   {SP}
    0b0000_0101, // 3 PT_SL  {SP, LF}
    0b0000_0011, // 4 PT_SQ  {SP, "}
    0b0000_0111, // 5 PT_SQL {SP, ", LF}
    0b1000_0000, // 6 PT_Z   {NUL}
];

/// One packed base: its alphabet and the width of one index.
pub struct Packed {
    pub name: &'static str,
    pub w: u32,
    pub chars: &'static [u8],
}

/// Classes 7..=19, in class order. Specification section 7.4.
pub static PACKED: [Packed; 13] = [
    Packed { name: "DEC", w: 4, chars: b"0123456789" },
    Packed { name: "HEXL", w: 4, chars: b"0123456789abcdef" },
    Packed { name: "HEXU", w: 4, chars: b"0123456789ABCDEF" },
    Packed { name: "HEXL_D", w: 5, chars: b"0123456789abcdef-" },
    Packed { name: "HEXU_D", w: 5, chars: b"0123456789ABCDEF-" },
    Packed { name: "ALPHA_L", w: 5, chars: b"abcdefghijklmnopqrstuvwxyz" },
    Packed { name: "ALPHA_U", w: 5, chars: b"ABCDEFGHIJKLMNOPQRSTUVWXYZ" },
    Packed { name: "B32", w: 5, chars: b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567" },
    Packed { name: "B32H", w: 5, chars: b"0123456789ABCDEFGHIJKLMNOPQRSTUV" },
    Packed { name: "CROCK", w: 5, chars: b"0123456789ABCDEFGHJKMNPQRSTVWXYZ" },
    Packed { name: "B64", w: 6, chars: b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/" },
    Packed { name: "B64U", w: 6, chars: b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_" },
    Packed { name: "ALNUM", w: 6, chars: b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz" },
];

/// Per packed class, byte -> index, or 0xFF for a byte outside its alphabet.
pub static PACKED_INDEX: [[u8; 256]; 13] = {
    let mut all = [[0xFFu8; 256]; 13];
    let mut c = 0;
    while c < 13 {
        let chars = PACKED[c].chars;
        let mut i = 0;
        while i < chars.len() {
            all[c][chars[i] as usize] = i as u8;
            i += 1;
        }
        c += 1;
    }
    all
};

/// One bit per packed class: bit `c` set means byte `b` is in class `c`'s
/// alphabet. One load answers "which classes could open here" for a byte,
/// which is what the scan of specification section 11.1 asks first.
pub static PACKED_MEMBERSHIP: [u16; 256] = {
    let mut t = [0u16; 256];
    let mut b = 0;
    while b < 256 {
        let mut c = 0;
        while c < 13 {
            if PACKED_INDEX[c][b] != 0xFF {
                t[b] |= 1 << c;
            }
            c += 1;
        }
        b += 1;
    }
    t
};

/// True where the byte is an alphabet character or an R-Set member, which is
/// the precondition for passthrough carrying it at all.
pub static PT_CARRIABLE: [bool; 256] = {
    let mut t = [false; 256];
    let mut b = 0;
    while b < 256 {
        t[b] = VALUE_OF[b] != 0xFF;
        b += 1;
    }
    let mut j = 0;
    while j < R_LEN {
        t[R_CHARS[j] as usize] = true;
        j += 1;
    }
    t
};

/// R-Set index of a byte, or 0xFF. Byte values and R-Set members are disjoint
/// from the alphabet, so this and `VALUE_OF` never both answer.
pub static R_INDEX: [u8; 256] = {
    let mut t = [0xFFu8; 256];
    let mut j = 0;
    while j < R_LEN {
        t[R_CHARS[j] as usize] = j as u8;
        j += 1;
    }
    t
};

/// Per profile, the rank a byte holds as a donor, or 8 where it is not one.
/// The scan keeps the lowest rank any literal has held, per profile, and a
/// profile stays viable exactly while that is at least `k`.
pub static DONOR_RANK: [[u8; 256]; NUM_PROFILES] = {
    let mut all = [[8u8; 256]; NUM_PROFILES];
    let mut p = 0;
    while p < NUM_PROFILES {
        let mut r = 0;
        while r < 8 {
            all[p][PROFILES[p][r] as usize] = r as u8;
            r += 1;
        }
        p += 1;
    }
    all
};
