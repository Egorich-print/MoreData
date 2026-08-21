use crate::VERSION;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct StatusReport {
    pub engine: &'static str,
    pub version: &'static str,
    pub phase: &'static str,
    pub realtime_plane: &'static str,
    pub control_plane: &'static str,
    pub pd_coupled: bool,
}

impl StatusReport {
    pub fn current() -> Self {
        Self {
            engine: "moredata-core",
            version: VERSION,
            phase: "prototype",
            realtime_plane: "compiled-graph",
            control_plane: "cli-json",
            pd_coupled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostics {
    pub blocks: u64,
    pub frames: u64,
    pub xruns: u64,
    pub last_block_ns: u64,
    pub sample_rate: u32,
    pub max_block: usize,
    pub nodes: usize,
    pub backend: String,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            blocks: 0,
            frames: 0,
            xruns: 0,
            last_block_ns: 0,
            sample_rate: 0,
            max_block: 0,
            nodes: 0,
            backend: "none".into(),
        }
    }
}
