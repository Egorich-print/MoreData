//! Native graph + DSP kernel. No audio I/O, no plugins, no Pd.

mod compile;
mod dsp;
mod error;
pub mod event;
mod graph;
mod project;
mod report;

pub use compile::{CompileOptions, CompiledGraph, ParamSnapshot, ProcessState};
pub use error::GraphError;
pub use event::{Event, EventKind, EventQueue, EventWindow};
pub use graph::{Connection, Graph, Node, NodeId, NodeKind, PortDir, PortSpec};
pub use project::Project;
pub use report::{Diagnostics, StatusReport};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ENGINE: &str = "moredata-core";

#[cfg(test)]
mod tests;
