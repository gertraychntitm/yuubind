use std::{fmt, str};
use nom;
use nom::Needed;

#[derive(Fail, Debug, Clone)]
pub enum ParseError {
    DidNotConsumeEverything(usize),
    ParseError(nom::Err),
    IncompleteString(Needed),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ParseError::*;
        match self {
            &DidNotConsumeEverything(rem) =>
                write!(f, "Input contains {} trailing characters", rem),
            &ParseError(ref err) =>
                write!(f, "Parse error: {}", err),
            &IncompleteString(Needed::Unknown) =>
                write!(f, "Input appears to be incomplete"),
            &IncompleteString(Needed::Size(sz)) =>
                write!(f, "Input appears to be missing {} characters", sz),
        }
    }
}

pub fn bytes_to_dbg(b: &[u8]) -> String {
    if let Ok(s) = str::from_utf8(b) {
        format!("b\"{}\"", s.chars().flat_map(|x| x.escape_default()).collect::<String>())
    } else {
        format!("{:?}", b)
    }
}
