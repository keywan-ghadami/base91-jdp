// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! One error type, with the codes of specification section 13. A caller
//! catches one type and matches on the code.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    InvalidCharacter,
    UnexpectedEos,
    UnknownClass,
    ExtendedClass,
    InvalidFlush,
    InvalidParams,
    InvalidLength,
    InvalidFinalBlock,
    InvalidIndex,
    InvalidRunValue,
    InvalidChain,
    MalformedPadding,
    MalformedFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: Code,
    pub at: usize,
    pub what: &'static str,
}

impl Error {
    pub fn new(code: Code, at: usize, what: &'static str) -> Self {
        Self { code, at, what }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at character {}: {}", self.code, self.at, self.what)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
