use genesis_climate::atmosphere::AtmosphericGrid;
use genesis_core::causality::CausalityGraph;
use genesis_core::events::{EventBus, WorldEvent};
use genesis_core::time::SimTick;
use genesis_geology::terrain::HeightField;
use glam::Vec2;

pub struct CascadingDisasterEngine;

impl CascadingDisasterEngine {
    /// 地震から始まる連鎖災害パイプライン:
    /// 地震 -> 山崩れ/河道閉塞 -> 鉄砲水/洪水 -> 農業被害
    pub fn process_seismic_cascade(
        tick: SimTick,
        epicenter: Vec2,
        magnitude: f32,
        heightfield: &mut HeightField,
        causality: &mut CausalityGraph,
        event_bus: &EventBus,
    ) {
        let quake_event_id = causality.record_event(
            tick,
            "Geology",
            format!("Magnitude {:.1} Tectonic Earthquake", magnitude),
            Vec::new(),
            (magnitude / 100.0).clamp(0.1, 1.0),
            "{}",
        );

        event_bus.publish(
            tick,
            Some(quake_event_id),
            WorldEvent::Earthquake {
                epicenter,
                magnitude,
                fault_id: 1,
            },
        );

        // マグニチュード60以上で地滑り・斜面崩壊
        if magnitude > 60.0 {
            let x = epicenter.x.clamp(1.0, (heightfield.width - 2) as f32) as usize;
            let y = epicenter.y.clamp(1.0, (heightfield.height - 2) as f32) as usize;

            let center_idx = heightfield.index(x, y);
            let slope = heightfield.calculate_normal(x, y).z;

            if slope < 0.85 {
                // 急峻な斜面での崩落
                heightfield.elevation[center_idx] -= 25.0;
                let downstream_idx = heightfield.index(x + 1, y);
                heightfield.elevation[downstream_idx] += 20.0; // 天然ダム形成

                let landslide_ev = causality.record_event(
                    tick,
                    "Disaster",
                    "Massive Landslide blocks river valley creating unstable natural dam",
                    vec![quake_event_id],
                    0.75,
                    "{}",
                );

                // 天然ダム決壊と大洪水
                let flood_ev = causality.record_event(
                    tick,
                    "Hydrology",
                    "Catastrophic Dam-break Flash Flood inundates downstream basin",
                    vec![landslide_ev],
                    0.90,
                    "{}",
                );

                event_bus.publish(
                    tick,
                    Some(flood_ev),
                    WorldEvent::Flood {
                        basin_coord: Vec2::new(x as f32, y as f32),
                        inundation_depth: 6.5,
                    },
                );
            }
        }
    }

    /// 火山噴火と成層圏火山灰エアロゾルによる寒冷化
    pub fn process_volcanic_eruption(
        tick: SimTick,
        volcano_pos: Vec2,
        vei: u8,
        atmosphere: &mut AtmosphericGrid,
        causality: &mut CausalityGraph,
        event_bus: &EventBus,
    ) {
        let ash_volume_km3 = 10.0f32.powi(vei as i32 - 4).max(0.1);
        let eruption_ev = causality.record_event(
            tick,
            "Volcanism",
            format!("Plinian Volcanic Eruption (VEI-{}) ejects {:.2} km³ ash", vei, ash_volume_km3),
            Vec::new(),
            (vei as f32 / 8.0).clamp(0.1, 1.0),
            "{}",
        );

        event_bus.publish(
            tick,
            Some(eruption_ev),
            WorldEvent::VolcanicEruption {
                position: volcano_pos,
                vei,
                ash_volume: ash_volume_km3,
            },
        );

        // 火山灰散布による日射遮蔽と気温急降下 (Volcanic Winter)
        if vei >= 5 {
            for cell in &mut atmosphere.cells {
                cell.temperature_c -= ash_volume_km3 * 0.4;
            }

            causality.record_event(
                tick,
                "Climate",
                "Atmospheric solar dimming triggers regional Volcanic Winter",
                vec![eruption_ev],
                0.85,
                "{}",
            );
        }
    }
}
