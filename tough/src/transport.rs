use serde::export::Formatter;
use std::fmt::{Debug, Display};
use std::io::{ErrorKind, Read};
use url::Url;

/// A trait to abstract over the method/protocol by which files are obtained.
pub trait Transport: Debug {
    // /// The type of `Read` object that the `fetch` function will return.
    // type Stream: Read;
    //
    // /// The type of error that the `fetch` function will return.
    // type Error: std::error::Error + Send + Sync + 'static;

    /// Opens a `Read` object for the file specified by `url`.
    fn fetch(&self, url: Url) -> Result<Box<dyn Read>, TransportError>;

    /// Returns a clone of `self` as a `Box<dyn Transport>`. Because the `Repository` object holds
    /// a `Box<dyn Transport>`, and because we want the `Repository` object to implement `Clone`,
    /// we need a way of cloning the held `Transport` without knowing its underlying type. We cannot
    /// require `Clone` on `Transport` because if we do, it can no longer serve as a 'trait object'.
    fn boxed_clone(&self) -> Box<dyn Transport>;
}

#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub enum TransportErrorKind {
    FileNotFound,
    // /// The scheme, e.g. `file://` or `ftp://`, is not supported by this transport. The offending
    // /// scheme is given
    // WrongScheme(String),
    Failure,
}

#[derive(Debug)]
pub struct TransportError {
    pub kind: TransportErrorKind,
    pub url: String,
    pub source: Box<dyn std::error::Error + Send + Sync>,
}

impl TransportError {
    pub fn new<S, E>(kind: TransportErrorKind, url: S, source_error: E) -> TransportError
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
        S: AsRef<str>,
    {
        Self {
            kind,
            url: url.as_ref().to_owned(),
            source: source_error.into(),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        let x: &(dyn std::error::Error + 'static) = self.source.as_ref();
        Some(x)
    }
}

impl Display for TransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Transport error '{:?}' for '{}', source: {}",
            self.kind, self.url, self.source
        )
    }
}

// /// The error type to use when implementing the `Transport` trait.
// #[derive(Debug, Snafu)]
// #[snafu(visibility = "pub")]
// #[non_exhaustive]
// pub enum TransportError {
//     #[snafu(display("File not found '{}': {}", string, source))]
//     FileNotFound { url: String, source: std::io::Error },
// }

/// Provides a `Transport` for local files.
#[derive(Debug, Clone, Copy)]
pub struct FilesystemTransport;

impl Transport for FilesystemTransport {
    // type Stream = std::fs::File;
    // type Error = std::io::Error;

    fn fetch(&self, url: Url) -> Result<Box<dyn Read>, TransportError> {
        // use std::io::{Error, ErrorKind};

        if url.scheme() != "file" {
            return Err(TransportError::new(
                TransportErrorKind::Failure,
                &url,
                format!("Wrong URL scheme: {}", url.scheme()),
            ));
            // return Err(TransportError::from_io_error(std::io::Error::new(
            //     ErrorKind::InvalidInput,
            //     format!("unexpected URL scheme: {}", url.scheme()),
            // )));
        }

        let f = std::fs::File::open(url.path()).map_err(|e| {
            let kind = match e.kind() {
                ErrorKind::NotFound => TransportErrorKind::FileNotFound,
                _ => TransportErrorKind::Failure,
            };
            TransportError::new(kind, url, e)
        })?;
        Ok(Box::new(f))
    }

    fn boxed_clone(&self) -> Box<dyn Transport> {
        Box::new(Clone::clone(self))
    }
}
