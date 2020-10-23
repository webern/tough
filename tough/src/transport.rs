use serde::export::Formatter;
use std::fmt::Display;
use std::io::{ErrorKind, Read};
use url::Url;

pub type TransportResult = Result<Box<dyn Read>, TransportError>;

/// A trait to abstract over the method/protocol by which files are obtained.
pub trait Transport {
    // /// The type of `Read` object that the `fetch` function will return.
    // type Stream: Read;
    //
    // /// The type of error that the `fetch` function will return.
    // type Error: std::error::Error + Send + Sync + 'static;

    /// Opens a `Read` object for the file specified by `url`.
    fn fetch(&self, url: Url) -> TransportResult;
}

#[derive(Debug)]
pub enum TransportError {
    FileNotFound(Option<Box<dyn std::error::Error + Send + Sync + 'static>>),
    Failure(Option<Box<dyn std::error::Error + Send + Sync + 'static>>),
}

impl TransportError {
    pub fn name(&self) -> &'static str {
        match self {
            TransportError::FileNotFound(_) => "FileNotFound",
            TransportError::Failure(_) => "Failure",
        }
    }

    fn from_io_error(e: std::io::Error) -> TransportError {
        match e.kind() {
            ErrorKind::NotFound => TransportError::FileNotFound(Some(Box::new(e))),
            _ => TransportError::Failure(Some(Box::new(e))),
        }
    }
}

impl Display for TransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())?;
        if let Some(e) = match self {
            TransportError::FileNotFound(e) => e,
            TransportError::Failure(e) => e,
        } {
            write!(f, ": {}", e)?;
        }
        Ok(())
    }
}

unsafe impl Send for TransportError {}
unsafe impl Sync for TransportError {}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TransportError::FileNotFound(e) => e.and_then(|e| Some(e.as_ref())),
            TransportError::Failure(e) => e.and_then(|e| Some(e.as_ref())),
        }
    }
}

/// Provides a `Transport` for local files.
#[derive(Debug, Clone, Copy)]
pub struct FilesystemTransport;

impl Transport for FilesystemTransport {
    // type Stream = std::fs::File;
    // type Error = std::io::Error;

    fn fetch(&self, url: Url) -> TransportResult {
        // use std::io::{Error, ErrorKind};

        if url.scheme() != "file" {
            return Err(TransportError::from_io_error(std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("unexpected URL scheme: {}", url.scheme()),
            )));
        }

        let f = std::fs::File::open(url.path()).map_err(|e| TransportError::from_io_error(e))?;
        Ok(Box::new(f))
    }
}
