use crate::causality::CausalityNodeId;
use crate::time::SimTick;
use glam::Vec2;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorldEvent {
    Earthquake {
        epicenter: Vec2,
        magnitude: f32,
        fault_id: u32,
    },
    VolcanicEruption {
        position: Vec2,
        vei: u8,
        ash_volume: f32,
    },
    Flood {
        basin_coord: Vec2,
        inundation_depth: f32,
    },
    Drought {
        region_coord: Vec2,
        severity: f32,
    },
    CropFailure {
        settlement_id: u64,
        shortage_ratio: f32,
    },
    PriceSurge {
        settlement_id: u64,
        resource_id: u16,
        ratio: f32,
    },
    CivilUnrest {
        settlement_id: u64,
        intensity: f32,
    },
    WarDeclared {
        attacker_nation_id: u32,
        defender_nation_id: u32,
    },
    NationFallen {
        nation_id: u32,
        successor_nation_id: Option<u32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedEvent {
    pub tick: SimTick,
    pub causality_id: Option<CausalityNodeId>,
    pub event: WorldEvent,
}

#[derive(Clone, Default)]
pub struct EventBus {
    queue: Arc<RwLock<Vec<TimestampedEvent>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn publish(&self, tick: SimTick, causality_id: Option<CausalityNodeId>, event: WorldEvent) {
        self.queue.write().push(TimestampedEvent {
            tick,
            causality_id,
            event,
        });
    }

    pub fn drain(&self) -> Vec<TimestampedEvent> {
        let mut lock = self.queue.write();
        std::mem::take(&mut *lock)
    }
}
