use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GraphError {
    #[error("unknown node `{0}`")]
    UnknownNode(String),
    #[error("duplicate node id `{0}`")]
    DuplicateNode(String),
    #[error("unknown port `{port}` on node `{node}`")]
    UnknownPort { node: String, port: String },
    #[error("cannot connect {from_dir:?} port `{from}` to {to_dir:?} port `{to}`")]
    DirectionMismatch {
        from: String,
        from_dir: super::graph::PortDir,
        to: String,
        to_dir: super::graph::PortDir,
    },
    #[error("channel mismatch {from_ch} → {to_ch} ({from} → {to})")]
    ChannelMismatch {
        from: String,
        to: String,
        from_ch: u16,
        to_ch: u16,
    },
    #[error("unknown parameter `{param}` on `{node}`")]
    UnknownParam { node: String, param: String },
    #[error("parameter `{param}` out of range on `{node}`")]
    ParamRange { node: String, param: String },
    #[error("cycle detected involving `{0}`")]
    Cycle(String),
    #[error("graph has no output node")]
    NoOutput,
    #[error("empty node id")]
    EmptyId,
    #[error("invalid project: {0}")]
    Project(String),
    #[error("sample rate must be > 0")]
    BadSampleRate,
}
