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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_returns_events_in_publication_order_and_empties_the_bus() {
        let bus = EventBus::new();
        bus.publish(
            SimTick(1),
            None,
            WorldEvent::Drought {
                region_coord: Vec2::new(1.0, 2.0),
                severity: 0.5,
            },
        );
        bus.publish(
            SimTick(2),
            Some(CausalityNodeId(9)),
            WorldEvent::CivilUnrest {
                settlement_id: 3,
                intensity: 0.7,
            },
        );

        let drained = bus.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].tick, SimTick(1));
        assert!(drained[0].causality_id.is_none());
        assert_eq!(drained[1].causality_id, Some(CausalityNodeId(9)));
        assert!(matches!(
            drained[1].event,
            WorldEvent::CivilUnrest {
                settlement_id: 3,
                ..
            }
        ));
        assert!(bus.drain().is_empty());
    }

    #[test]
    fn clones_share_the_same_queue() {
        let bus = EventBus::new();
        let handle = bus.clone();
        handle.publish(
            SimTick(5),
            None,
            WorldEvent::WarDeclared {
                attacker_nation_id: 1,
                defender_nation_id: 2,
            },
        );
        assert_eq!(bus.drain().len(), 1);
        assert!(handle.drain().is_empty());
    }

    #[test]
    fn default_bus_starts_empty() {
        let bus = EventBus::default();
        assert!(bus.drain().is_empty());
    }

    #[test]
    fn events_survive_serde_roundtrip() {
        let event = TimestampedEvent {
            tick: SimTick(42),
            causality_id: Some(CausalityNodeId(3)),
            event: WorldEvent::VolcanicEruption {
                position: Vec2::new(4.0, 5.0),
                vei: 6,
                ash_volume: 12.5,
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: TimestampedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.tick, SimTick(42));
        match restored.event {
            WorldEvent::VolcanicEruption {
                vei,
                ash_volume,
                position,
            } => {
                assert_eq!(vei, 6);
                assert_eq!(ash_volume, 12.5);
                assert_eq!(position, Vec2::new(4.0, 5.0));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
