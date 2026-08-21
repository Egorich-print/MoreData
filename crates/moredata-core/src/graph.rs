use crate::error::GraphError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortDir {
    In,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Oscillator,
    Gain,
    Mixer,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortSpec {
    pub name: &'static str,
    pub dir: PortDir,
    pub channels: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub kind: NodeKind,
    pub params: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub from_node: NodeId,
    pub from_port: String,
    pub to_node: NodeId,
    pub to_port: String,
}

#[derive(Debug, Clone)]
pub struct Graph {
    pub sample_rate: u32,
    next_id: u32,
    nodes: Vec<Node>,
    connections: Vec<Connection>,
}

impl NodeKind {
    pub fn ports(self) -> &'static [PortSpec] {
        match self {
            NodeKind::Oscillator => &[PortSpec {
                name: "out",
                dir: PortDir::Out,
                channels: 1,
            }],
            NodeKind::Gain => &[
                PortSpec {
                    name: "in",
                    dir: PortDir::In,
                    channels: 1,
                },
                PortSpec {
                    name: "out",
                    dir: PortDir::Out,
                    channels: 1,
                },
            ],
            NodeKind::Mixer => &[
                PortSpec {
                    name: "in",
                    dir: PortDir::In,
                    channels: 1,
                },
                PortSpec {
                    name: "out",
                    dir: PortDir::Out,
                    channels: 1,
                },
            ],
            NodeKind::Output => &[PortSpec {
                name: "in",
                dir: PortDir::In,
                channels: 1,
            }],
        }
    }

    pub fn default_params(self) -> BTreeMap<String, f32> {
        let mut m = BTreeMap::new();
        match self {
            NodeKind::Oscillator => {
                m.insert("freq".into(), 440.0);
                m.insert("amp".into(), 0.2);
            }
            NodeKind::Gain => {
                m.insert("gain".into(), 1.0);
            }
            NodeKind::Mixer | NodeKind::Output => {}
        }
        m
    }

    pub fn param_ok(self, name: &str, value: f32) -> bool {
        match (self, name) {
            (NodeKind::Oscillator, "freq") => {
                value.is_finite() && (0.0..=20_000.0).contains(&value)
            }
            (NodeKind::Oscillator, "amp") => value.is_finite() && (0.0..=1.0).contains(&value),
            (NodeKind::Gain, "gain") => value.is_finite() && (0.0..=8.0).contains(&value),
            _ => false,
        }
    }
}

impl Graph {
    pub fn new(sample_rate: u32) -> Result<Self, GraphError> {
        if sample_rate == 0 {
            return Err(GraphError::BadSampleRate);
        }
        Ok(Self {
            sample_rate,
            next_id: 1,
            nodes: Vec::new(),
            connections: Vec::new(),
        })
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    pub fn add_node(
        &mut self,
        name: impl Into<String>,
        kind: NodeKind,
    ) -> Result<NodeId, GraphError> {
        let name = name.into();
        if name.is_empty() {
            return Err(GraphError::EmptyId);
        }
        if self.nodes.iter().any(|n| n.name == name) {
            return Err(GraphError::DuplicateNode(name));
        }
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.nodes.push(Node {
            id,
            name,
            kind,
            params: kind.default_params(),
        });
        Ok(id)
    }

    pub fn node_by_name(&self, name: &str) -> Result<&Node, GraphError> {
        self.nodes
            .iter()
            .find(|n| n.name == name)
            .ok_or_else(|| GraphError::UnknownNode(name.into()))
    }

    pub fn node_by_id(&self, id: NodeId) -> Result<&Node, GraphError> {
        self.nodes
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| GraphError::UnknownNode(format!("{}", id.0)))
    }

    pub fn set_param(&mut self, id: NodeId, param: &str, value: f32) -> Result<(), GraphError> {
        let node = self
            .nodes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or_else(|| GraphError::UnknownNode(format!("{}", id.0)))?;
        if !node.kind.param_ok(param, value) {
            if !node.kind.default_params().contains_key(param) {
                return Err(GraphError::UnknownParam {
                    node: node.name.clone(),
                    param: param.into(),
                });
            }
            return Err(GraphError::ParamRange {
                node: node.name.clone(),
                param: param.into(),
            });
        }
        node.params.insert(param.into(), value);
        Ok(())
    }

    pub fn connect(
        &mut self,
        from: NodeId,
        from_port: &str,
        to: NodeId,
        to_port: &str,
    ) -> Result<(), GraphError> {
        let src = self.node_by_id(from)?.clone_meta();
        let dst = self.node_by_id(to)?.clone_meta();
        let sp = src
            .kind
            .ports()
            .iter()
            .find(|p| p.name == from_port)
            .ok_or_else(|| GraphError::UnknownPort {
                node: src.name.clone(),
                port: from_port.into(),
            })?;
        let dp = dst
            .kind
            .ports()
            .iter()
            .find(|p| p.name == to_port)
            .ok_or_else(|| GraphError::UnknownPort {
                node: dst.name.clone(),
                port: to_port.into(),
            })?;
        if sp.dir != PortDir::Out || dp.dir != PortDir::In {
            return Err(GraphError::DirectionMismatch {
                from: format!("{}.{}", src.name, from_port),
                from_dir: sp.dir,
                to: format!("{}.{}", dst.name, to_port),
                to_dir: dp.dir,
            });
        }
        if sp.channels != dp.channels {
            return Err(GraphError::ChannelMismatch {
                from: format!("{}.{}", src.name, from_port),
                to: format!("{}.{}", dst.name, to_port),
                from_ch: sp.channels,
                to_ch: dp.channels,
            });
        }
        self.connections.push(Connection {
            from_node: from,
            from_port: from_port.into(),
            to_node: to,
            to_port: to_port.into(),
        });
        Ok(())
    }

    pub fn validate(&self) -> Result<(), GraphError> {
        if !self.nodes.iter().any(|n| n.kind == NodeKind::Output) {
            return Err(GraphError::NoOutput);
        }
        self.topo()?;
        Ok(())
    }

    pub(crate) fn topo(&self) -> Result<Vec<NodeId>, GraphError> {
        let n = self.nodes.len();
        let mut idx: BTreeMap<u32, usize> = BTreeMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            idx.insert(node.id.0, i);
        }
        let mut indeg = vec![0u32; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for c in &self.connections {
            let a = *idx
                .get(&c.from_node.0)
                .ok_or_else(|| GraphError::UnknownNode(format!("{}", c.from_node.0)))?;
            let b = *idx
                .get(&c.to_node.0)
                .ok_or_else(|| GraphError::UnknownNode(format!("{}", c.to_node.0)))?;
            adj[a].push(b);
            indeg[b] += 1;
        }
        let mut q: Vec<usize> = indeg
            .iter()
            .enumerate()
            .filter(|(_, d)| **d == 0)
            .map(|(i, _)| i)
            .collect();
        let mut order = Vec::with_capacity(n);
        while let Some(i) = q.pop() {
            order.push(self.nodes[i].id);
            for &j in &adj[i] {
                indeg[j] -= 1;
                if indeg[j] == 0 {
                    q.push(j);
                }
            }
        }
        if order.len() != n {
            let involved = self
                .nodes
                .iter()
                .find(|node| !order.contains(&node.id))
                .map(|n| n.name.clone())
                .unwrap_or_else(|| "?".into());
            return Err(GraphError::Cycle(involved));
        }
        Ok(order)
    }
}

struct NodeMeta {
    name: String,
    kind: NodeKind,
}

impl Node {
    fn clone_meta(&self) -> NodeMeta {
        NodeMeta {
            name: self.name.clone(),
            kind: self.kind,
        }
    }
}
