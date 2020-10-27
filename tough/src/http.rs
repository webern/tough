//! The `http` module provides `HttpTransport` which enables `Repository` objects to be
//! loaded over HTTP
use crate::error::Error::HttpRequestBuild;
use crate::transport::Kind;
use crate::{Transport, TransportError};
use log::{debug, error, trace};
use reqwest::blocking::{Client, ClientBuilder, Request, Response};
use reqwest::header::{self, HeaderValue, ACCEPT_RANGES};
use reqwest::{Error, Method, StatusCode};
use snafu::ResultExt;
use std::cmp::Ordering;
use std::io::Read;
use std::time::Duration;
use url::Url;

/// Settings for the HTTP client including retry strategy and timeouts.
#[derive(Clone, Copy, Debug)]
pub struct ClientSettings {
    /// Set a timeout for connect, read and write operations.
    pub timeout: Duration,
    /// Set a timeout for only the connect phase.
    pub connect_timeout: Duration,
    /// The total number of times we will try to get the response.
    pub tries: u32,
    /// The pause between the first and second try.
    pub initial_backoff: Duration,
    /// The maximum length of a pause between retries.
    pub max_backoff: Duration,
    /// The exponential backoff factor, the factor by which the pause time will increase after each
    /// try until reaching `max_backoff`.
    pub backoff_factor: f32,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            timeout: std::time::Duration::from_secs(30),
            connect_timeout: std::time::Duration::from_secs(10),
            /// try / 100ms / try / 150ms / try / 220ms / try
            tries: 4,
            initial_backoff: std::time::Duration::from_millis(100),
            max_backoff: std::time::Duration::from_secs(1),
            backoff_factor: 1.5,
        }
    }
}

/// An HTTP `Transport` with retry logic.
#[derive(Clone, Copy, Debug, Default)]
pub struct HttpTransport {
    settings: ClientSettings,
}

impl HttpTransport {
    /// Create a new `HttpRetryTransport` with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new `HttpRetryTransport` with specific settings.
    pub fn from_settings(settings: ClientSettings) -> Self {
        Self { settings }
    }
}

/// Implement the `tough` `Transport` trait for `HttpRetryTransport`
impl Transport for HttpTransport {
    /// Send a GET request to the URL. Request will be retried per the `ClientSettings`. The
    /// returned `RetryRead` will also retry as necessary per the `ClientSettings`.
    fn fetch(&self, url: Url) -> Result<Box<dyn Read>, TransportError> {
        let mut r = RetryState::new(self.settings.initial_backoff);
        Ok(Box::new(fetch_with_retries(&mut r, &self.settings, &url)?))
    }

    fn boxed_clone(&self) -> Box<dyn Transport> {
        Box::new(*self)
    }
}

/// This serves as a `Read`, but carries with it the necessary information to do retries.
#[derive(Debug)]
pub struct RetryRead {
    retry_state: RetryState,
    settings: ClientSettings,
    response: Response,
    url: Url,
}

impl Read for RetryRead {
    /// Read bytes into `buf`, retrying as necessary.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // retry loop
        loop {
            let retry_err = match self.response.read(buf) {
                Ok(sz) => {
                    self.retry_state.next_byte += sz;
                    return Ok(sz);
                }
                // store the error in `retry_err` to return later if there are no more retries
                Err(err) => err,
            };
            debug!("error during read of '{}': {:?}", self.url, retry_err);

            // increment the `retry_state` and fetch a new reader if retries are not exhausted
            if self.retry_state.current_try >= self.settings.tries - 1 {
                // we are out of retries, so return the last known error.
                return Err(retry_err);
            }
            self.retry_state.increment(&self.settings);
            self.err_if_no_range_support(retry_err)?;
            // wait, then retry the request (with a range header).
            std::thread::sleep(self.retry_state.wait);
            let new_retry_read =
                fetch_with_retries(&mut self.retry_state, &self.settings, &self.url)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
            // the new fetch succeeded so we need to replace our read object with the new one.
            self.response = new_retry_read.response;
        }
    }
}

impl RetryRead {
    /// Checks for the header `Accept-Ranges: bytes`
    fn supports_range(&self) -> bool {
        if let Some(ranges) = self.response.headers().get(ACCEPT_RANGES) {
            if let Ok(val) = ranges.to_str() {
                if val.contains("bytes") {
                    return true;
                }
            }
        }
        false
    }

    /// Returns an error when we have received an error during read, but our server does not support
    /// range headers. Our retry implementation considers this a fatal condition rather that trying
    /// to start over from the beginning and advancing the `Read` to the point where failure
    /// occurred.
    fn err_if_no_range_support(&self, e: std::io::Error) -> std::io::Result<()> {
        if !self.supports_range() {
            // we cannot send a byte range request to this server, so return the error
            error!(
                "an error occurred and we cannot retry because the server \
                    does not support range requests '{}': {:?}",
                self.url, e
            );
            return Err(e);
        }
        Ok(())
    }
}

/// A private struct that serves as the retry counter.
#[derive(Clone, Copy, Debug)]
struct RetryState {
    /// The current try we are on. First try is zero.
    current_try: u32,
    /// The amount that the we should sleep before the next retry.
    wait: Duration,
    /// The next byte that we should read. e.g. the last read byte + 1.
    next_byte: usize,
}

impl RetryState {
    fn new(initial_wait: Duration) -> Self {
        Self {
            current_try: 0,
            wait: initial_wait,
            next_byte: 0,
        }
    }
}

impl RetryState {
    /// Increments the count and the wait duration.
    fn increment(&mut self, settings: &ClientSettings) {
        if self.current_try > 0 {
            let new_wait = self.wait.mul_f32(settings.backoff_factor);
            match new_wait.cmp(&settings.max_backoff) {
                Ordering::Less => {
                    self.wait = new_wait;
                }
                Ordering::Greater => {
                    self.wait = settings.max_backoff;
                }
                Ordering::Equal => {}
            }
        }
        self.current_try += 1;
    }
}

/// Sends a `GET` request to the `url`. Retries the request as necessary per the `ClientSettings`.
fn fetch_with_retries(
    r: &mut RetryState,
    cs: &ClientSettings,
    url: &Url,
) -> Result<RetryRead, TransportError> {
    trace!("beginning fetch for '{}'", url);
    // create a reqwest client
    let client = ClientBuilder::new()
        .timeout(cs.timeout)
        .connect_timeout(cs.connect_timeout)
        .build()
        .map_err(|e| TransportError::new(Kind::Failure, &url, e))?;
    // TODO - variant for this error type? .context(error::HttpClientBuild { url: url.clone() })?;
    // retry loop
    loop {
        // build the request
        let request = build_request(&client, r.next_byte, &url)?;

        // send the request, inspect the result and convert to an HttpResult
        let http_result: HttpResult = client.execute(request).into();

        match http_result {
            HttpResult::Ok(response) => {
                trace!("{:?} - returning from successful fetch", r);
                return Ok(RetryRead {
                    retry_state: *r,
                    settings: *cs,
                    response,
                    url: url.clone(),
                });
            }
            HttpResult::Fatal(err) => {
                trace!("{:?} - returning fatal error from fetch: {}", r, err);
                return Err(TransportError::new(Kind::Failure, &url, err));
            }
            HttpResult::FileNotFound(err) => {
                trace!("{:?} - returning file not found from fetch: {}", r, err);
                return Err(TransportError::new(Kind::FileNotFound, &url, err));
            }
            HttpResult::Retryable(err) => {
                trace!("{:?} - retryable error: {}", r, err);
                if r.current_try >= cs.tries - 1 {
                    debug!("{:?} - returning failure, no more retries: {}", r, err);
                    return Err(TransportError::new(Kind::Failure, &url, err));
                    // TODO - variant for this error type? .context(error::HttpRetries { url: url.clone(), tries: cs.tries, });
                }
            }
        }

        r.increment(&cs);
        std::thread::sleep(r.wait);
    }
}

struct FetchResult(Result<reqwest::Response, reqwest::Error>);

impl Into<FetchResult> for Result<reqwest::Response, reqwest::Error> {
    fn into(self) -> FetchResult {
        FetchResult(self)
    }
}

/// Much of the complexity in the `fetch_with_retries` function is in deciphering the `Result` we
/// get from the reqwest client `execute` function. Using this enum we categorize the states of that
/// `Result` into the categories that we need to understand.
enum HttpResult {
    /// We got a response with an HTTP code that indicates success.
    Ok(reqwest::blocking::Response),
    /// We got an `Error` (other than file-not-found) which we will not retry.
    Fatal(reqwest::Error),
    /// The file could not be found (HTTP status 403 or 404).
    FileNotFound(reqwest::Error),
    /// We received an `Error`, or we received an HTTP response code that we can retry.
    Retryable(reqwest::Error),
}

/// Takes the `Result` type from the reqwest client `execute` function, and categorizes it into an
/// `HttpResult` variant.
impl Into<HttpResult> for Result<reqwest::blocking::Response, reqwest::Error> {
    fn into(self) -> HttpResult {
        match self {
            Ok(response) => {
                trace!("response received");
                // checks the status code of the response for errors
                parse_response(response)
            }
            Err(err) => {
                // an error occurred before the HTTP header could be read
                trace!("retryable error during fetch: {}", err);
                HttpResult::Retryable(err)
            }
        }
    }
}

/// Checks the HTTP response code and converts a non-successful response code to an error.
fn parse_response(response: reqwest::blocking::Response) -> HttpResult {
    match response.error_for_status() {
        Ok(ok) => {
            trace!("response is success");
            // http status is ok. return early from this function with happiness
            HttpResult::Ok(ok)
        }
        // http status is an error
        Err(err) => match err.status() {
            None => {
                // this shouldn't happen, we received this err from the err_for_status function,
                // so we the err should have a status. we cannot consider this a retryable error.
                trace!("error is fatal (no status): {}", err);
                HttpResult::Fatal(err)
            }
            Some(status) => parse_status_err(err, status),
        },
    }
}

/// Categorizes the the error type based on its HTTP code.
fn parse_status_err(err: reqwest::Error, status: reqwest::StatusCode) -> HttpResult {
    if status.is_server_error() {
        trace!("error is retryable: {}", err);
        HttpResult::Retryable(err)
    } else {
        match status.as_u16() {
            // some services (like S3) return a 403 when the file is not found
            403 | 404 => {
                trace!("error is file not found: {}", err);
                HttpResult::FileNotFound(err)
            }
            _ => {
                trace!("error is fatal (status): {}", err);
                HttpResult::Fatal(err)
            }
        }
    }
}

/// Builds a GET request. If `next_byte` is greater than zero, adds a byte range header to the request.
fn build_request(client: &Client, next_byte: usize, url: &Url) -> Result<Request, TransportError> {
    if next_byte == 0 {
        let request = client
            .request(Method::GET, url.as_str())
            .build()
            .context(http_error::RequestBuild)
            .map_err(|e| TransportError::new(Kind::Failure, &url, e))?; // TODO - remove
        Ok(request)
    } else {
        let header_value_string = format!("bytes={}-", next_byte);
        let header_value = HeaderValue::from_str(header_value_string.as_str())
            .context(http_error::InvalidHeader {
                header_value: &header_value_string,
            })
            .map_err(|e| TransportError::new(Kind::Failure, &url, e))?; // TODO - remove
        let request = client
            .request(Method::GET, url.as_str())
            .header(header::RANGE, header_value)
            .build()
            .context(http_error::RequestBuild)
            .map_err(|e| TransportError::new(Kind::Failure, &url, e))?; // TODO - remove
        Ok(request)
    }
}

mod http_error {
    #![allow(clippy::default_trait_access)]

    // use crate::schema;
    // use crate::schema::RoleType;
    // use chrono::{DateTime, Utc};
    use snafu::Snafu;
    // use std::io;
    // use std::path::PathBuf;
    // use url::Url;

    /// The error type for the HTTP transport module.
    #[derive(Debug, Snafu)]
    #[snafu(visibility = "pub(crate)")]
    #[non_exhaustive]
    #[allow(missing_docs)]
    pub enum HttpError {
        #[snafu(display("A non-retryable error occurred: {}", source))]
        FetchFatal { source: reqwest::Error },

        #[snafu(display("Invalid header value '{}': {}", header_value, source))]
        InvalidHeader {
            header_value: String,
            source: reqwest::header::InvalidHeaderValue,
        },

        #[snafu(display("Unable to create HTTP request: {}", source))]
        RequestBuild { source: reqwest::Error },
    }
}
