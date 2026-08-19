//! What can go wrong, and where.

use std::fmt;

/// The result of anything in this crate that can fail.
pub type Result<T> = std::result::Result<T, Error>;

/// Why a document could not be read, written or configured.
///
/// Every variant carries a message written for whoever has to fix the document,
/// not for whoever wrote the parser, and [`Error::Parse`] carries the byte
/// offset the reader gave up at so a caller can slice the text with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The text is not JSON. [`crate::locate`] says exactly where and why.
    Parse {
        /// What is wrong, in plain words.
        message: String,
        /// The byte offset it was found at, when the reader knows one.
        position: Option<usize>,
    },
    /// The document parses, but it is not a json-compress envelope this build
    /// can restore: an unknown version, a reference pointing outside its
    /// catalogue, a table whose rows do not line up with its columns.
    Decode(String),
    /// The options given to an encoder do not make sense.
    Options(String),
    /// A value could not be carried into or out of the model: an object key that
    /// is not a string, a number that will not fit the field it is being read
    /// into, or whatever a `Serialize` implementation reported itself. Only the
    /// `serde` feature produces this.
    Conversion(String),
    /// The document is nested deeper than this build will walk. The limit is
    /// there so that a hostile document exhausts a counter rather than the
    /// stack; the value carried is the limit that was passed.
    TooDeep(usize),
}

impl Error {
    pub(crate) fn parse(message: impl Into<String>, position: usize) -> Error {
        Error::Parse {
            message: message.into(),
            position: Some(position),
        }
    }

    pub(crate) fn truncated() -> Error {
        Error::Parse {
            message: "the document ends before it is complete".to_owned(),
            position: None,
        }
    }

    pub(crate) fn decode(message: impl Into<String>) -> Error {
        Error::Decode(message.into())
    }

    pub(crate) fn options(message: impl Into<String>) -> Error {
        Error::Options(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse {
                message,
                position: Some(at),
            } => write!(out, "json-compress: {message}, at byte {at}"),
            Error::Parse {
                message,
                position: None,
            } => write!(out, "json-compress: {message}"),
            Error::Decode(message) | Error::Options(message) | Error::Conversion(message) => {
                write!(out, "json-compress: {message}")
            }
            Error::TooDeep(limit) => write!(
                out,
                "json-compress: the document is nested more than {limit} levels deep, which is deeper than this build will read"
            ),
        }
    }
}

impl std::error::Error for Error {}
