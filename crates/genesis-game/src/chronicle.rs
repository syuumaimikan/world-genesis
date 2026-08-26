//! 世界史の記録。
//!
//! 起きた出来事をただ並べるのではなく、原因への参照を持たせる。
//! 「革命が起きた」だけでは歴史ではない。「地震 → 不作 → 食糧不足 →
//! 物価高騰 → 暴動 → 革命」と辿れて初めて歴史になる。

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronicleEvent {
    pub id: u64,
    pub tick: u64,
    pub title: String,
    pub detail: String,
    /// 0.0〜1.0。世界史に残る大事件ほど高い。
    pub importance: f32,
    /// この出来事を引き起こした出来事の ID。
    pub causes: Vec<u64>,
}

impl ChronicleEvent {
    /// 何日目の出来事か。
    pub fn day(&self) -> u64 {
        self.tick / 86_400
    }

    pub fn year(&self) -> u64 {
        self.day() / 360 + 1
    }

    pub fn formatted_date(&self) -> String {
        let day_of_year = self.day() % 360;
        format!(
            "{}年 {}月{}日",
            self.year(),
            day_of_year / 30 + 1,
            day_of_year % 30 + 1
        )
    }
}

/// このセッションで起きた出来事。
#[derive(Resource, Default)]
pub struct LocalChronicle {
    pub events: Vec<ChronicleEvent>,
    next_id: u64,
    /// 保持する上限。古い低重要度のものから捨てる。
    pub capacity: usize,
}

impl LocalChronicle {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            next_id: 1,
            capacity: 400,
        }
    }

    pub fn record(&mut self, tick: u64, title: impl Into<String>, detail: impl Into<String>, importance: f32) -> u64 {
        self.record_caused_by(tick, title, detail, importance, Vec::new())
    }

    pub fn record_caused_by(
        &mut self,
        tick: u64,
        title: impl Into<String>,
        detail: impl Into<String>,
        importance: f32,
        causes: Vec<u64>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        // 存在しない原因への参照は捨てる（歴史の整合性を壊さない）。
        let causes: Vec<u64> = causes
            .into_iter()
            .filter(|c| *c < id && self.events.iter().any(|e| e.id == *c))
            .collect();

        self.events.push(ChronicleEvent {
            id,
            tick,
            title: title.into(),
            detail: detail.into(),
            importance: importance.clamp(0.0, 1.0),
            causes,
        });
        self.prune();
        id
    }

    /// 容量を超えたら、重要度の低い古い出来事から忘れる。
    /// ただし他の出来事の原因になっているものは残す（因果の鎖を切らない）。
    fn prune(&mut self) {
        if self.events.len() <= self.capacity {
            return;
        }
        let referenced: std::collections::HashSet<u64> = self
            .events
            .iter()
            .flat_map(|e| e.causes.iter().copied())
            .collect();

        let over = self.events.len() - self.capacity;
        let mut removable: Vec<(usize, f32)> = self
            .events
            .iter()
            .enumerate()
            .filter(|(_, e)| !referenced.contains(&e.id))
            .map(|(i, e)| (i, e.importance))
            .collect();
        // 重要度の低い順、同じなら古い順。
        removable.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));

        let mut to_remove: Vec<usize> = removable.into_iter().take(over).map(|(i, _)| i).collect();
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for i in to_remove {
            self.events.remove(i);
        }
    }

    /// 新しい順の一覧。
    pub fn recent(&self, count: usize) -> Vec<&ChronicleEvent> {
        self.events.iter().rev().take(count).collect()
    }

    /// 重要な出来事だけ。
    pub fn notable(&self, threshold: f32) -> Vec<&ChronicleEvent> {
        self.events.iter().filter(|e| e.importance >= threshold).collect()
    }

    /// ある出来事の原因を根本まで遡る（深さ優先、循環に耐える）。
    pub fn trace_causes(&self, id: u64) -> Vec<&ChronicleEvent> {
        let mut out = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![id];

        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }
            let Some(event) = self.events.iter().find(|e| e.id == current) else {
                continue;
            };
            out.push(event);
            for c in &event.causes {
                stack.push(*c);
            }
        }
        // 古い順（原因が先）に並べ替える。
        out.sort_by_key(|e| e.tick);
        out
    }

    /// セーブ用の形式へ。
    pub fn to_save(&self) -> Vec<crate::saves::ChronicleEntry> {
        self.events
            .iter()
            .map(|e| crate::saves::ChronicleEntry {
                tick: e.tick,
                year: e.year() as u32,
                title: e.title.clone(),
                detail: e.detail.clone(),
                importance: e.importance,
            })
            .collect()
    }

    pub fn from_save(entries: &[crate::saves::ChronicleEntry]) -> Self {
        let mut c = Self::new();
        for e in entries {
            c.record(e.tick, e.title.clone(), e.detail.clone(), e.importance);
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_formatted_from_ticks() {
        let e = ChronicleEvent {
            id: 1,
            tick: 86_400 * 365 + 3600,
            title: String::new(),
            detail: String::new(),
            importance: 0.5,
            causes: Vec::new(),
        };
        assert_eq!(e.day(), 365);
        assert_eq!(e.year(), 2); // 1年 = 360日
        assert!(e.formatted_date().contains("2年"));
    }

    #[test]
    fn causal_chains_can_be_traced_to_the_root() {
        let mut c = LocalChronicle::new();
        let quake = c.record(1000, "地震", "北の山脈が揺れた", 0.9);
        let crop = c.record_caused_by(2000, "不作", "地滑りで農地が埋まった", 0.7, vec![quake]);
        let famine = c.record_caused_by(3000, "食糧不足", "備蓄が尽きた", 0.8, vec![crop]);
        let riot = c.record_caused_by(4000, "暴動", "民衆が蔵を襲った", 0.85, vec![famine]);

        let chain = c.trace_causes(riot);
        let titles: Vec<&str> = chain.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["地震", "不作", "食糧不足", "暴動"]);
    }

    #[test]
    fn events_may_have_several_causes() {
        let mut c = LocalChronicle::new();
        let drought = c.record(100, "干ばつ", "雨が降らない", 0.6);
        let war = c.record(200, "戦争", "隣国と開戦した", 0.9);
        let collapse = c.record_caused_by(300, "国家崩壊", "税も兵も尽きた", 1.0, vec![drought, war]);

        let chain = c.trace_causes(collapse);
        assert_eq!(chain.len(), 3);
        assert!(chain.iter().any(|e| e.title == "干ばつ"));
        assert!(chain.iter().any(|e| e.title == "戦争"));
    }

    #[test]
    fn dangling_causes_are_rejected() {
        let mut c = LocalChronicle::new();
        let id = c.record_caused_by(10, "何か", "", 0.5, vec![9999]);
        let event = c.events.iter().find(|e| e.id == id).unwrap();
        assert!(event.causes.is_empty(), "a reference to a nonexistent cause survived");
    }

    #[test]
    fn tracing_a_cycle_terminates() {
        let mut c = LocalChronicle::new();
        let a = c.record(1, "A", "", 0.5);
        let b = c.record_caused_by(2, "B", "", 0.5, vec![a]);
        // 後から A の原因に B を足して循環を作る（本来は起きないが、堅牢性の確認）。
        c.events[0].causes.push(b);

        let chain = c.trace_causes(b);
        assert_eq!(chain.len(), 2, "cycle handling lost or duplicated events");
    }

    #[test]
    fn pruning_keeps_the_chronicle_bounded_but_preserves_causes() {
        let mut c = LocalChronicle::new();
        c.capacity = 20;
        let root = c.record(0, "起点", "全ての始まり", 0.1);
        for i in 1..100u64 {
            c.record(i * 100, format!("些事 {i}"), "", 0.05);
        }
        // 起点を原因とする出来事を最後に足す。
        let last = c.record_caused_by(20_000, "結末", "", 1.0, vec![root]);

        assert!(c.events.len() <= 21, "chronicle grew past its capacity: {}", c.events.len());
        // 参照されている「起点」は、重要度が最低でも残っていなければならない。
        assert!(c.events.iter().any(|e| e.id == root), "a referenced cause was pruned away");
        assert_eq!(c.trace_causes(last).len(), 2);
    }

    #[test]
    fn important_events_survive_and_can_be_listed() {
        let mut c = LocalChronicle::new();
        c.record(1, "些事", "", 0.1);
        c.record(2, "大事件", "", 0.9);
        let notable = c.notable(0.5);
        assert_eq!(notable.len(), 1);
        assert_eq!(notable[0].title, "大事件");
    }

    #[test]
    fn recent_returns_newest_first() {
        let mut c = LocalChronicle::new();
        c.record(1, "古い", "", 0.5);
        c.record(2, "新しい", "", 0.5);
        let r = c.recent(2);
        assert_eq!(r[0].title, "新しい");
        assert_eq!(r[1].title, "古い");
    }

    #[test]
    fn chronicle_round_trips_through_a_save() {
        let mut c = LocalChronicle::new();
        c.record(86_400 * 400, "大噴火", "火山が目覚めた", 0.95);
        c.record(86_400 * 401, "降灰", "空が暗くなった", 0.6);

        let saved = c.to_save();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].year, 2);

        let restored = LocalChronicle::from_save(&saved);
        assert_eq!(restored.events.len(), 2);
        assert_eq!(restored.events[0].title, "大噴火");
        assert_eq!(restored.events[1].importance, 0.6);
    }

    #[test]
    fn importance_is_clamped() {
        let mut c = LocalChronicle::new();
        c.record(1, "a", "", 9.0);
        c.record(2, "b", "", -3.0);
        assert_eq!(c.events[0].importance, 1.0);
        assert_eq!(c.events[1].importance, 0.0);
    }
}
