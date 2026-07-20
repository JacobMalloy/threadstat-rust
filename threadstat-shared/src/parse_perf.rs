
use perf_ffi::PfmError;
use std::error::Error;
use string_intern::{InteriorNulError,InternC};
use std::fmt::Display;
use non_empty::{NonEmpty,MaybeNonEmpty};

#[derive(Clone, Debug)]
pub enum ParseError {
    Pfm(PfmError),
    InternalNULL(InteriorNulError),
}

impl Error for ParseError {}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Pfm(e) => write!(f, "libpfm error:{e}"),
            ParseError::InternalNULL(e) => e.fmt(f),
        }
    }
}

impl From<PfmError> for ParseError {
    fn from(e: PfmError) -> ParseError {
        ParseError::Pfm(e)
    }
}

impl From<InteriorNulError> for ParseError {
    fn from(e: InteriorNulError) -> ParseError {
        ParseError::InternalNULL(e)
    }
}


/// Parse "event1,[event2,event3],event4" into `Vec<Vec<PerfConfig<Intern>>>`.
/// Top-level commas separate groups; [..] brackets group multiple events together.
pub fn parse_event_groups(s: &str) -> Result<Box<[NonEmpty<perf_ffi::PerfConfig<InternC>>]>, ParseError> {
    let mut groups = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;

    loop {
        while i < b.len() && (b[i] == b',' || b[i].is_ascii_whitespace()) {
            i += 1;
        }

        if i >= b.len() {
            break;
        }

        if b[i] == b'[' {
            i += 1; // consume '['
            let start = i;
            while i < b.len() && b[i] != b']' {
                i += 1;
            }
            let inner = &s[start..i];
            if i < b.len() {
                i += 1; // consume ']'
            }
            let group: Result<MaybeNonEmpty<_>, ParseError> = inner
                .split(',')
                .map(str::trim)
                .filter(|e| !e.is_empty())
                .map(|e| -> Result<_, ParseError> {
                    let interned = InternC::try_new(e)?;
                    Ok(perf_ffi::PerfConfig::from_pfm_string(interned, interned)?)
                })
                .collect();
            if let Some(v) = group?.into_option() {
                groups.push(v);
            }
        } else {
            let start = i;
            while i < b.len() && b[i] != b',' && b[i] != b'[' {
                i += 1;
            }
            let name = s[start..i].trim();
            if !name.is_empty() {
                let interned = InternC::new(name);
                groups.push(NonEmpty::new_single(perf_ffi::PerfConfig::from_pfm_string(
                    interned, interned,
                )?));
            }
        }
    }

    Ok(groups.into_boxed_slice())
}


