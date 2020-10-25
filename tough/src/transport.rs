use serde::export::Formatter;
use std::borrow::Borrow;
use std::fmt::{Debug, Display};
use std::io::{ErrorKind, Read};
use url::Url;

pub type TransportResult = Result<Box<dyn Read>, TransportError>;

/// A trait to abstract over the method/protocol by which files are obtained.
pub trait Transport: Debug {
    // /// The type of `Read` object that the `fetch` function will return.
    // type Stream: Read;
    //
    // /// The type of error that the `fetch` function will return.
    // type Error: std::error::Error + Send + Sync + 'static;

    /// Opens a `Read` object for the file specified by `url`.
    fn fetch(&self, url: Url) -> TransportResult;

    /// Returns a clone of `self` as a `Box<dyn Transport>`. Because the `Repository` object holds
    /// a `Box<dyn Transport>`, and because we want the `Repository` object to implement `Clone`,
    /// we need a way of cloning the held `Transport` without knowing its underlying type. We cannot
    /// require `Clone` on `Transport` because if we do, it can no longer serve as a 'trait object'.
    fn boxed_clone(&self) -> Box<dyn Transport>;
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

    pub fn into_inner(self) -> Option<Box<dyn std::error::Error + Send + Sync + 'static>> {
        match self {
            TransportError::FileNotFound(a) => a,
            TransportError::Failure(b) => b,
        }
    }

    pub fn inner(&self) -> Option<&(dyn std::error::Error + Send + Sync + 'static)> {
        let source = match self {
            TransportError::FileNotFound(a) => a,
            TransportError::Failure(b) => b,
        };
        // source.as_ref().and_then(|e| Some(e.borrow()))
        // source.as_ref().and_then(|e| Some(e.borrow()))
        source.as_ref().map(|e| e.borrow())
    }
}

impl Display for TransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())?;
        if let Some(e) = match self {
            TransportError::FileNotFound(e) => e,
            TransportError::Failure(f) => f,
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
        // self.inner()
        //     .and_then(|xxx| Some(coerce_error(xxx.borrow())))
        self.inner().map(|xxx| coerce_error(xxx.borrow()))
        // match self {
        //     TransportError::FileNotFound(e) => match e {
        //         None => None,
        //         Some(e) => Some(coerce_error(e.borrow())),
        //     },
        //
        //     //e.and_then(|e| Some(coerce_error(e.as_ref()))),
        //     TransportError::Failure(e) => match e {
        //         None => None,
        //         Some(e) => Some(coerce_error(e.borrow())),
        //     },
        //     //e.and_then(|e| Some(coerce_error(e.as_ref()))),
        // }
    }
}

fn coerce_error<'a>(
    e: &'a (dyn std::error::Error + Send + Sync + 'static),
) -> &'a (dyn std::error::Error + 'static) {
    e
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

        let f = std::fs::File::open(url.path()).map_err(TransportError::from_io_error)?;
        Ok(Box::new(f))
    }

    fn boxed_clone(&self) -> Box<dyn Transport> {
        Box::new(Clone::clone(self))
    }
}
