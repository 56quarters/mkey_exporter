use crate::profile::ProfilerError;
use pprof::protos::Message;
use pprof::{ProfilerGuard, ProfilerGuardBuilder};
use std::ffi::c_int;

const PROFILE_FREQUENCY_HZ: c_int = 1000;
const PROFILE_BLOCK_LIST: &[&str] = &["libc", "libgcc", "pthread", "vdso"];

pub struct Profiler {
    guard: ProfilerGuard<'static>,
}

impl Profiler {
    pub fn new() -> Result<Self, ProfilerError> {
        let guard = ProfilerGuardBuilder::default()
            .frequency(PROFILE_FREQUENCY_HZ)
            .blocklist(PROFILE_BLOCK_LIST)
            .build()
            .map_err(|e| ProfilerError::msg_cause("cannot build profiler", e))?;

        Ok(Self { guard })
    }

    pub fn proto(&self) -> Result<Vec<u8>, ProfilerError> {
        self.guard
            .report()
            .build()
            .and_then(|report| report.pprof())
            .map_err(|e| ProfilerError::msg_cause("cannot build profile report", e))
            .and_then(|profile| {
                profile
                    .write_to_bytes()
                    .map_err(|e| ProfilerError::msg_cause("cannot encode as protobuf", e))
            })
    }
}
