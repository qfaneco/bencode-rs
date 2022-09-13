use std::{fmt, io, str};
use serde::{de, ser};

pub type Result<T> = std::result::Result<T, Error>;

/// This type represents all possible errors that can occur when serializing or
/// deserializing Bencode data.
pub struct Error {
    /// This `Box` allows us to keep the size of `Error` as small as possible. A
    /// larger `Error` type was substantially slower due to all the functions
    /// that pass around `Result<T, Error>`.
    err: Box<ErrorContent>,
}

impl Error {
    pub fn index(&self) -> Option<usize> {
        self.err.index
    }
    
    #[cold]
    pub(in crate) fn syntax(kind: ErrorKind, index: usize) -> Self {
        Error {
            err: Box::new(ErrorContent { kind, index: Some(index) })
        }
    }

    #[cold]
    pub(in crate) fn eof(index: usize) -> Self {
        Error { err: Box::new(ErrorContent {
            kind: ErrorKind::Eof,
            index: Some(index),
        })}
    }

    #[cold]
    pub(in crate) fn io(err: io::Error) -> Self {
        Error { err: Box::new(ErrorContent {
            kind: ErrorKind::Io(err),
            index: None,
        })}
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&*self.err, f)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Error({:?}, index: {})",
            self.err.kind.to_string(),
            self.err.index.unwrap_or(0),
        )
    }
}

impl de::StdError for Error {
    fn source(&self) -> Option<&(dyn de::StdError + 'static)> {
        match self.err.kind {
            ErrorKind::Io(ref err) => Some(err),
            _ => None,
        }
    }
}

impl ser::Error for Error {
    #[cold]
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self {
            err: Box::new(ErrorContent {
                kind: ErrorKind::Message(msg.to_string().into_boxed_str()),
                index: None,
            })
        }
    }
}

impl de::Error for Error {
    #[cold]
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self {
            err: Box::new(ErrorContent {
                kind: ErrorKind::Message(msg.to_string().into_boxed_str()),
                index: None,
            })
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::io(err)
    }
}

struct ErrorContent {
    kind: ErrorKind,
    index: Option<usize>,
}

impl fmt::Display for ErrorContent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.index.is_none() {
            fmt::Display::fmt(&self.kind, f)
        } else {
            write!(f, "{} at index {}", self.kind, self.index.unwrap())
        }
    }
}

pub enum ErrorKind {
    Message(Box<str>),
    Io(io::Error),
    Eof,
    ExpectedBoolean,
    ExpectedInteger,
    ExpectedString,
    ExpectedChar,
    ExpectedList,
    ExpectedDict,
    ExpectedStringDelim,
    ExpectedEnum,
    ExpectedEnd,
    ExpectedSomeValue,
    MinusZero,
    LeadingZero,
    IntegerOutOfRange,
    StringNotUtf8,
    KeyMustBeAString,
    TrailingCharacters,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ErrorKind::*;
        match *self {
            Message(ref msg)    => write!(f, "{}", msg),
            Io(ref err)     => fmt::Display::fmt(err, f),
            Eof                 => write!(f, "EOF while parsing"),
            ExpectedBoolean     => write!(f, "expected boolean"),
            ExpectedInteger     => write!(f, "expected integer"),
            ExpectedString      => write!(f, "expected string"),
            ExpectedChar        => write!(f, "expected character"),
            ExpectedList        => write!(f, "expected list"),
            ExpectedDict        => write!(f, "expected dictionary"),
            ExpectedEnd         => write!(f, "expected `e`"),
            ExpectedStringDelim => write!(f, "expected `:`"),
            ExpectedEnum        => write!(f, "expected enum"),
            ExpectedSomeValue   => write!(f, "expected value"),
            MinusZero           => write!(f, "`i-0e` is invalid"),
            LeadingZero         => write!(f, "leading zeros are invalid"),
            IntegerOutOfRange   => write!(f, "integer out of range"),
            StringNotUtf8       => write!(f, "strings must be a utf-8"),
            KeyMustBeAString    => write!(f, "key must be a string"),
            TrailingCharacters  => write!(f, "trailing characters"),
        }
    }
}
