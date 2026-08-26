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
