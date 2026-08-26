use crate::time::SimTick;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CausalityNodeId(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalityRecord {
    pub id: CausalityNodeId,
    pub tick: SimTick,
    pub category: String,
    pub headline: String,
    pub parent_causes: Vec<CausalityNodeId>,
    pub child_effects: Vec<CausalityNodeId>,
    pub severity: f32,
    pub payload_json: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CausalityGraph {
    nodes: HashMap<CausalityNodeId, CausalityRecord>,
    next_id: u64,
}

impl CausalityGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn record_event(
        &mut self,
        tick: SimTick,
        category: impl Into<String>,
        headline: impl Into<String>,
        parents: Vec<CausalityNodeId>,
        severity: f32,
        payload_json: impl Into<String>,
    ) -> CausalityNodeId {
        let node_id = CausalityNodeId(self.next_id);
        self.next_id += 1;

        for &parent_id in &parents {
            if let Some(parent) = self.nodes.get_mut(&parent_id) {
                parent.child_effects.push(node_id);
            }
        }

        let record = CausalityRecord {
            id: node_id,
            tick,
            category: category.into(),
            headline: headline.into(),
            parent_causes: parents,
            child_effects: Vec::new(),
            severity: severity.clamp(0.0, 1.0),
            payload_json: payload_json.into(),
        };

        self.nodes.insert(node_id, record);
        node_id
    }

    pub fn get_node(&self, id: CausalityNodeId) -> Option<&CausalityRecord> {
        self.nodes.get(&id)
    }

    pub fn trace_root_causes(&self, id: CausalityNodeId) -> Vec<CausalityNodeId> {
        let mut roots = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(id);

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current) {
                continue;
            }
            if let Some(node) = self.nodes.get(&current) {
                if node.parent_causes.is_empty() {
                    roots.push(current);
                } else {
                    for &parent in &node.parent_causes {
                        queue.push_back(parent);
                    }
                }
            }
        }
        roots
    }

    pub fn total_events(&self) -> usize {
        self.nodes.len()
    }
}
