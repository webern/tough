use serde::export::Formatter;
use std::fmt::{Debug, Display};
use std::io::{ErrorKind, Read};
use url::Url;

/// A trait to abstract over the method/protocol by which files are obtained.
///
/// The trait hides the underlying types involved by returning the `Read` object as a
/// `Box<dyn Read>` and by requiring concrete type [`TransportError`] as the error type.
///
pub trait Transport: Debug {
    /// Opens a `Read` object for the file specified by `url`.
    fn fetch(&self, url: Url) -> Result<Box<dyn Read>, TransportError>;

    /// Returns a clone of `self` as a `Box<dyn Transport>`.
    ///
    /// # Why
    ///
    /// The [`Repository`] object holds a `Box<dyn Transport>`, and because we want the `Repository`
    /// object to implement `Clone`, we need a way of cloning the boxed `Transport` without knowing
    /// its underlying type. We cannot require require the `Clone` trait bound on `Transport`
    /// because, if we do, the trait can no longer serve as a 'trait object'.
    ///
    /// # How
    ///
    /// If your `Transport` object implements clone, then:
    ///
    /// ```rust,ignore
    /// fn boxed_clone(&self) -> Box<dyn Transport> {
    ///     Box::new(self.clone())
    /// }
    /// ```
    ///
    fn boxed_clone(&self) -> Box<dyn Transport>;
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// The kind of error that the transport object experienced during `fetch`.
///
/// # Why
///
/// Some TUF operations need to know if a [`Transport`] failure is a result of a file not being
/// found. In particular:
/// > 5.1.2. Try downloading version N+1 of the root metadata file `[...]` If this file is not
/// > available `[...]` then go to step 5.1.9.
///
/// To distinguish this case from other [`Transport`] failures, we use
/// `TransportErrorKind::FileNotFound`.
///
#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub enum TransportErrorKind {
    /// The trait does not handle the URL scheme named in `String`. e.g. `file://` or `http://`.
    UnsupportedUrlScheme,
    /// The file cannot be found.
    FileNotFound,
    /// The transport failed for any other reason, e.g. IO error, HTTP broken pipe, etc.
    Failure,
}

/// The error type that [`Transport`] `fetch` returns.
#[derive(Debug)]
pub struct TransportError {
    /// The kind of error that occurred.
    pub kind: TransportErrorKind,
    /// The URL that the transport was trying to fetch.
    pub url: String,
    /// The underlying error that occurred.
    pub source: Box<dyn std::error::Error + Send + Sync>,
}

impl TransportError {
    /// Creates a new [`TransportError`].
    pub fn new<S, E>(kind: TransportErrorKind, url: S, source_error: E) -> Self
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

    /// Creates a [`TransportError`] for reporting an unhandled URL type.
    pub fn unsupported_url<S: AsRef<str>>(url: S) -> Self {
        TransportError::new(
            TransportErrorKind::UnsupportedUrlScheme,
            url,
            "Transport cannot handle the given URL scheme.".to_string(),
        )
    }
}

/// [`TransportError`] implements the standard error interface.
impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        let x: &(dyn std::error::Error + 'static) = self.source.as_ref();
        Some(x)
    }
}

/// [`TransportError`] implements display, part of the standard error interface.
impl Display for TransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Transport error '{:?}' for '{}', source: {}",
            self.kind, self.url, self.source
        )
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// Provides a [`Transport`] for local files.
#[derive(Debug, Clone, Copy)]
pub struct FilesystemTransport;

impl Transport for FilesystemTransport {
    fn fetch(&self, url: Url) -> Result<Box<dyn Read>, TransportError> {
        if url.scheme() != "file" {
            return Err(TransportError::unsupported_url(url));
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
        Box::new(*self)
    }
}
