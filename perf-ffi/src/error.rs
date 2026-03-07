use core::fmt;
use std;
use std::error::Error as Error_trait;

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Empty,
}

impl Error_trait for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IO(e) => e.fmt(f),
            Error::Empty => write!(f, "Inputted iterator is empty"),
        }
    }
}

impl Error {
    pub fn empty() -> Self {
        Error::Empty
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::IO(value)
    }
}
