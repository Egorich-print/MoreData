use crate::error::GraphError;
use crate::graph::{Graph, NodeKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub sample_rate: u32,
    pub nodes: Vec<ProjectNode>,
    pub connections: Vec<ProjectConn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectNode {
    pub id: String,
    pub kind: NodeKind,
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConn {
    pub from: [String; 2],
    pub to: [String; 2],
}

impl Project {
    pub fn from_json(s: &str) -> Result<Self, GraphError> {
        serde_json::from_str(s).map_err(|e| GraphError::Project(e.to_string()))
    }

    pub fn to_graph(&self) -> Result<Graph, GraphError> {
        let mut g = Graph::new(self.sample_rate)?;
        let mut names = Vec::new();
        for n in &self.nodes {
            let id = g.add_node(&n.id, n.kind)?;
            for (k, v) in &n.params {
                g.set_param(id, k, *v)?;
            }
            names.push((n.id.clone(), id));
        }
        let lookup = |name: &str| -> Result<_, GraphError> {
            names
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, id)| *id)
                .ok_or_else(|| GraphError::UnknownNode(name.into()))
        };
        for c in &self.connections {
            let a = lookup(&c.from[0])?;
            let b = lookup(&c.to[0])?;
            g.connect(a, &c.from[1], b, &c.to[1])?;
        }
        g.validate()?;
        Ok(g)
    }
}

impl Graph {
    pub fn to_project(&self) -> Project {
        Project {
            sample_rate: self.sample_rate,
            nodes: self
                .nodes()
                .iter()
                .map(|n| ProjectNode {
                    id: n.name.clone(),
                    kind: n.kind,
                    params: n.params.clone(),
                })
                .collect(),
            connections: self
                .connections()
                .iter()
                .map(|c| {
                    let from_name = self
                        .node_by_id(c.from_node)
                        .map(|n| n.name.clone())
                        .unwrap_or_default();
                    let to_name = self
                        .node_by_id(c.to_node)
                        .map(|n| n.name.clone())
                        .unwrap_or_default();
                    ProjectConn {
                        from: [from_name, c.from_port.clone()],
                        to: [to_name, c.to_port.clone()],
                    }
                })
                .collect(),
        }
    }
}
