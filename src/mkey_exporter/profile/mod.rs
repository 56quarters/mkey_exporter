use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;
use warp::http::header::CONTENT_TYPE;
use warp::http::{HeaderValue, Response, StatusCode};
use warp::{Filter, Rejection, Reply};

#[cfg(not(feature = "profile"))]
pub use nop::Profiler;
#[cfg(feature = "profile")]
pub use pull::Profiler;

#[cfg(not(feature = "profile"))]
mod nop;
#[cfg(feature = "profile")]
mod pull;

const OCTET_STREAM: &str = "application/octet-stream";

/// Construct and return a new `Profiler` CPU profiler. This may be a pprof CPU
/// profiler or a no-op CPU profiler depending on build-time settings.
pub fn build() -> Result<Profiler, ProfilerError> {
    Profiler::new()
}

#[derive(Debug)]
pub struct ProfilerError {
    msg: String,
    cause: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl ProfilerError {
    pub fn msg<S>(msg: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            msg: msg.into(),
            cause: None,
        }
    }

    pub fn msg_cause<S, E>(msg: S, cause: E) -> Self
    where
        S: Into<String>,
        E: Error + Send + Sync + 'static,
    {
        Self {
            msg: msg.into(),
            cause: Some(Box::new(cause)),
        }
    }
}

impl Display for ProfilerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(e) = &self.cause {
            write!(f, "{}: {}", self.msg, e)
        } else {
            write!(f, "{}", self.msg)
        }
    }
}

impl Error for ProfilerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(e) = &self.cause {
            Some(e.as_ref())
        } else {
            None
        }
    }
}

pub fn http_pprof(profiler: Arc<Profiler>) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("debug")
        .and(warp::path("pprof"))
        .and(warp::path("profile"))
        .and(warp::filters::method::get())
        .map(move || {
            let profiler = profiler.clone();
            match profiler.proto() {
                Ok(bytes) => {
                    tracing::debug!(message = "encoded profiling data to protobuf", bytes = bytes.len());
                    let mut res = Response::new(bytes);
                    res.headers_mut()
                        .insert(CONTENT_TYPE, HeaderValue::from_static(OCTET_STREAM));
                    res.into_response()
                }
                Err(e) => {
                    tracing::error!(message = "error building or encoding profiling report", error = %e);
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        })
}
