use crate::causality::{CausalityGraph, CausalityNodeId, CausalityRecord};
use crate::time::{SimCalendar, SimTick};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoricalEpoch {
    PrimordialAge,       // 創世の黎明
    AgeOfFoundations,     // 諸王国の建国期
    AgeOfStrife,          // 戦争・飢饉・動乱の時代
    GoldenEnlightenment,  // 繁栄・商業・技術の黄金期
    IndustrialDawn,       // 蒸気・機械化の曙光
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalPathNode {
    pub node_id: CausalityNodeId,
    pub year: u32,
    pub category: String,
    pub headline: String,
    pub severity: f32,
}

pub struct ChronicleEngine;

impl ChronicleEngine {
    /// 世界の状態から現在の時代区分 (Epoch) を自動判定
    pub fn determine_epoch(
        total_years: u32,
        active_wars: usize,
        global_unrest: f32,
        unlocked_tech_count: usize,
    ) -> HistoricalEpoch {
        if total_years < 10 {
            HistoricalEpoch::PrimordialAge
        } else if unlocked_tech_count >= 5 {
            HistoricalEpoch::IndustrialDawn
        } else if active_wars > 0 || global_unrest > 0.4 {
            HistoricalEpoch::AgeOfStrife
        } else if unlocked_tech_count >= 3 && global_unrest < 0.15 {
            HistoricalEpoch::GoldenEnlightenment
        } else {
            HistoricalEpoch::AgeOfFoundations
        }
    }

    /// 特定の重大事件から過去の根本原因（Root Causes）までの因果チェーンを抽出
    pub fn trace_causal_lineage(graph: &CausalityGraph, target_id: CausalityNodeId) -> Vec<CausalPathNode> {
        let mut path = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(target_id);

        while let Some(current_id) = queue.pop_front() {
            if !visited.insert(current_id) {
                continue;
            }

            if let Some(record) = graph.get_node(current_id) {
                let cal = SimCalendar::from_tick(record.tick);
                path.push(CausalPathNode {
                    node_id: record.id,
                    year: cal.year,
                    category: record.category.clone(),
                    headline: record.headline.clone(),
                    severity: record.severity,
                });

                for &parent_id in &record.parent_causes {
                    queue.push_back(parent_id);
                }
            }
        }

        path.sort_by_key(|n| n.year);
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::TICKS_PER_YEAR;

    #[test]
    fn early_world_is_primordial_regardless_of_other_state() {
        assert_eq!(
            ChronicleEngine::determine_epoch(0, 3, 0.9, 9),
            HistoricalEpoch::PrimordialAge
        );
        assert_eq!(
            ChronicleEngine::determine_epoch(9, 0, 0.0, 0),
            HistoricalEpoch::PrimordialAge
        );
    }

    #[test]
    fn plentiful_technology_marks_industrial_dawn() {
        assert_eq!(
            ChronicleEngine::determine_epoch(200, 4, 0.9, 5),
            HistoricalEpoch::IndustrialDawn
        );
    }

    #[test]
    fn wars_or_high_unrest_mark_age_of_strife() {
        assert_eq!(
            ChronicleEngine::determine_epoch(50, 1, 0.0, 0),
            HistoricalEpoch::AgeOfStrife
        );
        assert_eq!(
            ChronicleEngine::determine_epoch(50, 0, 0.5, 4),
            HistoricalEpoch::AgeOfStrife
        );
    }

    #[test]
    fn peaceful_advanced_world_is_golden_enlightenment() {
        assert_eq!(
            ChronicleEngine::determine_epoch(120, 0, 0.1, 3),
            HistoricalEpoch::GoldenEnlightenment
        );
    }

    #[test]
    fn quiet_primitive_world_is_age_of_foundations() {
        assert_eq!(
            ChronicleEngine::determine_epoch(30, 0, 0.2, 1),
            HistoricalEpoch::AgeOfFoundations
        );
        // Advanced but restless enough to miss the golden threshold.
        assert_eq!(
            ChronicleEngine::determine_epoch(30, 0, 0.3, 3),
            HistoricalEpoch::AgeOfFoundations
        );
    }

    #[test]
    fn causal_lineage_is_ordered_from_oldest_to_newest() {
        let mut graph = CausalityGraph::new();
        let quake = graph.record_event(SimTick(0), "Geology", "quake", Vec::new(), 0.9, "{}");
        let famine = graph.record_event(
            SimTick(TICKS_PER_YEAR * 3),
            "Society",
            "famine",
            vec![quake],
            0.7,
            "{}",
        );
        let revolt = graph.record_event(
            SimTick(TICKS_PER_YEAR * 8),
            "Society",
            "revolt",
            vec![famine],
            0.5,
            "{}",
        );

        let path = ChronicleEngine::trace_causal_lineage(&graph, revolt);
        let ids: Vec<CausalityNodeId> = path.iter().map(|n| n.node_id).collect();
        assert_eq!(ids, vec![quake, famine, revolt]);
        assert_eq!(path[0].year, 1);
        assert_eq!(path[1].year, 4);
        assert_eq!(path[2].year, 9);
        assert_eq!(path[2].category, "Society");
        assert_eq!(path[0].headline, "quake");
        assert_eq!(path[0].severity, 0.9);
    }

    #[test]
    fn causal_lineage_visits_each_shared_ancestor_once() {
        let mut graph = CausalityGraph::new();
        let root = graph.record_event(SimTick(0), "Geology", "quake", Vec::new(), 1.0, "{}");
        let left = graph.record_event(SimTick(10), "Hydrology", "flood", vec![root], 0.8, "{}");
        let right = graph.record_event(
            SimTick(20),
            "Society",
            "displacement",
            vec![root],
            0.6,
            "{}",
        );
        let joined = graph.record_event(
            SimTick(30),
            "Society",
            "famine",
            vec![left, right],
            0.9,
            "{}",
        );

        let path = ChronicleEngine::trace_causal_lineage(&graph, joined);
        assert_eq!(path.len(), 4);
        assert_eq!(path.iter().filter(|n| n.node_id == root).count(), 1);
    }

    #[test]
    fn causal_lineage_of_unknown_node_is_empty() {
        let graph = CausalityGraph::new();
        assert!(ChronicleEngine::trace_causal_lineage(&graph, CausalityNodeId(1)).is_empty());
    }
}
