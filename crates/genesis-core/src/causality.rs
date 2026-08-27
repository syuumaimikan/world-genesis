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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_event_assigns_sequential_ids_and_counts_events() {
        let mut graph = CausalityGraph::new();
        let a = graph.record_event(SimTick(1), "Geology", "quake", Vec::new(), 0.5, "{}");
        let b = graph.record_event(SimTick(2), "Climate", "drought", Vec::new(), 0.5, "{}");
        assert_eq!(a, CausalityNodeId(1));
        assert_eq!(b, CausalityNodeId(2));
        assert_eq!(graph.total_events(), 2);
    }

    #[test]
    fn record_event_clamps_severity_and_stores_payload() {
        let mut graph = CausalityGraph::new();
        let hot = graph.record_event(SimTick(1), "War", "siege", Vec::new(), 5.0, "{\"a\":1}");
        let cold = graph.record_event(SimTick(1), "War", "truce", Vec::new(), -3.0, "{}");
        assert_eq!(graph.get_node(hot).unwrap().severity, 1.0);
        assert_eq!(graph.get_node(cold).unwrap().severity, 0.0);
        assert_eq!(graph.get_node(hot).unwrap().payload_json, "{\"a\":1}");
        assert_eq!(graph.get_node(hot).unwrap().category, "War");
        assert_eq!(graph.get_node(hot).unwrap().headline, "siege");
    }

    #[test]
    fn record_event_links_parents_to_children_bidirectionally() {
        let mut graph = CausalityGraph::new();
        let root = graph.record_event(SimTick(0), "Geology", "quake", Vec::new(), 0.9, "{}");
        let child = graph.record_event(SimTick(1), "Hydrology", "flood", vec![root], 0.8, "{}");

        assert_eq!(graph.get_node(root).unwrap().child_effects, vec![child]);
        assert_eq!(graph.get_node(child).unwrap().parent_causes, vec![root]);
    }

    #[test]
    fn get_node_returns_none_for_unknown_id() {
        let graph = CausalityGraph::new();
        assert!(graph.get_node(CausalityNodeId(42)).is_none());
    }

    #[test]
    fn trace_root_causes_walks_back_to_parentless_events() {
        let mut graph = CausalityGraph::new();
        let quake = graph.record_event(SimTick(0), "Geology", "quake", Vec::new(), 1.0, "{}");
        let drought = graph.record_event(SimTick(0), "Climate", "drought", Vec::new(), 1.0, "{}");
        let famine = graph.record_event(
            SimTick(5),
            "Society",
            "famine",
            vec![quake, drought],
            1.0,
            "{}",
        );
        let revolt = graph.record_event(SimTick(9), "Society", "revolt", vec![famine], 1.0, "{}");

        let mut roots = graph.trace_root_causes(revolt);
        roots.sort();
        assert_eq!(roots, vec![quake, drought]);
    }

    #[test]
    fn trace_root_causes_of_a_root_is_itself() {
        let mut graph = CausalityGraph::new();
        let root = graph.record_event(SimTick(0), "Genesis", "world born", Vec::new(), 1.0, "{}");
        assert_eq!(graph.trace_root_causes(root), vec![root]);
    }

    #[test]
    fn trace_root_causes_terminates_on_cyclic_parents() {
        let mut graph = CausalityGraph::new();
        let a = graph.record_event(SimTick(0), "A", "a", Vec::new(), 1.0, "{}");
        let b = graph.record_event(SimTick(1), "B", "b", vec![a], 1.0, "{}");
        // Force a cycle a -> b -> a to prove traversal is cycle-safe.
        graph.nodes.get_mut(&a).unwrap().parent_causes.push(b);

        assert!(graph.trace_root_causes(b).is_empty());
    }

    #[test]
    fn trace_root_causes_of_unknown_node_is_empty() {
        let graph = CausalityGraph::new();
        assert!(graph.trace_root_causes(CausalityNodeId(7)).is_empty());
    }

    #[test]
    fn graph_survives_serde_roundtrip() {
        let mut graph = CausalityGraph::new();
        let root = graph.record_event(SimTick(3), "Geology", "quake", Vec::new(), 0.4, "{}");
        let child = graph.record_event(SimTick(4), "Hydrology", "flood", vec![root], 0.6, "{}");

        let json = serde_json::to_string(&graph).unwrap();
        let restored: CausalityGraph = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.total_events(), 2);
        assert_eq!(restored.get_node(child).unwrap().parent_causes, vec![root]);
        assert_eq!(restored.trace_root_causes(child), vec![root]);
    }
}
