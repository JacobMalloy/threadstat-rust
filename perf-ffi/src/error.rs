use core::fmt;
use std;
use std::error::Error as Error_trait;
use crate::read_structs::{PerfGroupReadHeader,PerfGroupReadEntry};

#[derive(Debug)]
pub enum Error<'a> {
    IO(std::io::Error),
    Empty,
    HeaderCast(zerocopy::CastError<&'a[u8], PerfGroupReadHeader>),
    EntryCast(zerocopy::CastError<&'a[u8], [PerfGroupReadEntry]>),
}

impl <'a>Error_trait for Error<'a> {}

impl <'a>fmt::Display for Error<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IO(e) => e.fmt(f),
            Error::HeaderCast(e) => e.fmt(f),
            Error::EntryCast(e) => e.fmt(f),
            Error::Empty => write!(f, "Inputted iterator is empty"),
        }
    }
}

impl <'a>Error<'a> {
    pub fn empty() -> Self {
        Error::Empty
    }
}

impl <'a>From<std::io::Error> for Error<'a> {
    fn from(value: std::io::Error) -> Self {
        Error::IO(value)
    }
}

impl <'a>From<zerocopy::CastError<&'a[u8], PerfGroupReadHeader>> for Error<'a> {
    fn from(value:zerocopy::CastError<&'a[u8], PerfGroupReadHeader> ) -> Self {
        Error::HeaderCast(value)
    }
}

impl <'a>From<zerocopy::CastError<&'a[u8], [PerfGroupReadEntry]>> for Error<'a> {
    fn from(value:zerocopy::CastError<&'a[u8], [PerfGroupReadEntry]> ) -> Self {
        Error::EntryCast(value)
    }
}
