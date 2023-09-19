use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

#[cfg(not(feature = "profile"))]
pub use nop::Profiler;
#[cfg(feature = "profile")]
pub use pull::Profiler;

#[cfg(not(feature = "profile"))]
mod nop;
#[cfg(feature = "profile")]
mod pull;

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
