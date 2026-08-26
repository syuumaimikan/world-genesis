use crate::world::WorldSimulation;
use genesis_core::chronicle::ChronicleEngine;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct WorldChronicleExporter;

impl WorldChronicleExporter {
    pub fn export_markdown_chronicle(world: &WorldSimulation, output_path: impl AsRef<Path>) -> std::io::Result<()> {
        let mut file = File::create(output_path)?;

        writeln!(file, "# WORLD GENESIS — 歴史年代記 (The Grand Chronicle)\n")?;
        writeln!(file, "世界シード値: `0x{:X}` | マップ解像度: {}x{}\n", world.config.seed, world.config.map_width, world.config.map_height)?;
        writeln!(file, "---\n")?;

        writeln!(file, "## 1. 記録された歴史的因果イベント一覧\n")?;
        let total_nodes = world.causality.total_events();

        for i in 1..=total_nodes {
            let node_id = genesis_core::causality::CausalityNodeId(i as u64);
            if let Some(node) = world.causality.get_node(node_id) {
                let cal = genesis_core::time::SimCalendar::from_tick(node.tick);
                let parents_str = if node.parent_causes.is_empty() {
                    "根源事象 (Root Event)".to_string()
                } else {
                    format!("起因: {:?}", node.parent_causes.iter().map(|p| p.0).collect::<Vec<_>>())
                };

                writeln!(
                    file,
                    "* **[YEAR {:04}/M{:02}] (Node #{}) [{}]** {} *(重要度: {:.2} / {})*",
                    cal.year, cal.month, node.id.0, node.category, node.headline, node.severity, parents_str
                )?;
            }
        }

        writeln!(file, "\n---\n")?;
        writeln!(file, "## 2. 現存する諸国家・都市の勢力概要\n")?;

        for nation in &world.nations {
            writeln!(
                file,
                "### 国家: {} (ID: {})\n- 政体: {:?}\n- 国庫残高: {:.1} コイン\n- 正統性: {:.1}%\n",
                nation.name, nation.id, nation.government, nation.treasury_balance, nation.legitimacy_pct
            )?;
        }

        for settlement in &world.settlements {
            writeln!(
                file,
                "- **都市 `{}`** (人口: {}人 | 規模: {:?} | 不満度: {:.1}%)",
                settlement.name, settlement.population, settlement.tier, settlement.unrest_level * 100.0
            )?;
        }

        Ok(())
    }
}
