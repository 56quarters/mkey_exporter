use crate::profile::ProfilerError;

#[derive(Debug)]
pub struct Profiler {}

impl Profiler {
    pub fn new() -> Result<Self, ProfilerError> {
        Ok(Self {})
    }

    pub fn proto(&self) -> Result<Vec<u8>, ProfilerError> {
        Err(ProfilerError::msg("not implemented"))
    }
}
