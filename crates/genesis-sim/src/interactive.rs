use crate::benchmark::SimulationBenchmarkRunner;
use crate::building::{BuildingType, ConstructionService};
use crate::chronicle_export::WorldChronicleExporter;
use crate::inspector::{ViewMode, WorldInspector};
use crate::player::PlayerCharacter;
use crate::world::WorldSimulation;
use genesis_core::chronicle::ChronicleEngine;
use genesis_core::time::{SimCalendar, TICKS_PER_DAY, TICKS_PER_MONTH, TICKS_PER_YEAR};
use genesis_economy::CommodityType;
use std::io::{self, BufRead, Write};

pub struct InteractiveController {
    pub current_view_mode: ViewMode,
}

impl Default for InteractiveController {
    fn default() -> Self {
        Self {
            current_view_mode: ViewMode::Elevation,
        }
    }
}

/// 端末へプロンプトを出す。改行しないため明示的に flush する必要がある。
fn prompt(text: &str) -> io::Result<()> {
    print!("{text}");
    io::stdout().flush()
}

/// 次の入力行を読む。`Ok(None)` は入力の終端（EOF）。
fn read_line<I: Iterator<Item = io::Result<String>>>(lines: &mut I) -> io::Result<Option<String>> {
    match lines.next() {
        Some(Ok(line)) => Ok(Some(line)),
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

impl InteractiveController {
    /// 対話セッションを回す。端末の入出力が壊れた場合はエラーを返し、
    /// 呼び出し側（プロセス終了コード）まで伝える。
    pub fn run_interactive_loop(
        &mut self,
        mut world: WorldSimulation,
        mut player: PlayerCharacter,
    ) -> io::Result<()> {
        let stdin = io::stdin();
        let mut lines = stdin.lock().lines();

        loop {
            WorldInspector::render_ansi_viewport(&world, self.current_view_mode, 64, 20);

            let cal = SimCalendar::from_tick(world.clock.current_tick);
            let grain_price = world
                .markets
                .first()
                .and_then(|m| m.listings.get(&CommodityType::Grain))
                .map(|l| l.last_clearing_price)
                .unwrap_or(0.0);

            let current_epoch = ChronicleEngine::determine_epoch(cal.year, 0, 0.1, 2);

            println!(
                "\x1B[1m[YEAR {:04}/M{:02}/D{:02}] (時代: {:?})\x1B[0m | Pop: {} | Grain: {:.2}c | History Nodes: {}",
                cal.year,
                cal.month,
                cal.day,
                current_epoch,
                world.settlements.first().map(|s| s.population).unwrap_or(0),
                grain_price,
                world.causality.total_events()
            );

            println!(
                "\x1B[36m[Player: {} ({:?})]\x1B[0m Coins: \x1B[33m{:.1}\x1B[0m | Hunger: {:.1}% | Bread: {:.1} | Wood: {:.1} | Tools: {:.1}",
                player.name,
                player.profession,
                player.coin_purse,
                player.needs.hunger,
                player.inventory.get(&CommodityType::Bread).unwrap_or(&0.0),
                player.inventory.get(&CommodityType::Timber).unwrap_or(&0.0),
                player.inventory.get(&CommodityType::Tools).unwrap_or(&0.0)
            );

            for ev in world.event_bus.drain() {
                println!("  \x1B[33m[EVENT]\x1B[0m {:?}", ev.event);
            }

            println!("------------------------------------------------------------------");
            println!(" [1] 時間進行 (1日/1ヶ月/1年)       [2] マップ表示切替 (標高/気候/植生/政治)");
            println!(" [3] プレイヤー行動 (労働/食事)       [4] 市場取引 (小麦/パン/木材/工具)");
            println!(" [5] 都市建築 (農場/工房)            [6] 因果遡及トレーサー");
            println!(" [7] 100年タイムラプス＆年代記出力   [8] 500年高負荷性能ベンチマーク");
            println!(" [9] セーブして終了");
            prompt("コマンドを入力してください (1-9): ")?;

            let Some(choice) = read_line(&mut lines)? else {
                break;
            };

            match choice.trim() {
                "1" => {
                    println!("進める期間を選択: [1] 1日  [2] 1ヶ月 (30日)  [3] 1年 (360日)");
                    prompt("> ")?;
                    if let Some(sub) = read_line(&mut lines)? {
                        match sub.trim() {
                            "1" => world.tick_step(TICKS_PER_DAY),
                            "2" => world.tick_step(TICKS_PER_MONTH),
                            "3" => world.tick_step(TICKS_PER_YEAR),
                            _ => {}
                        }
                    }
                }
                "2" => {
                    println!("表示レイヤー: [1] 標高地形  [2] 気温気候  [3] 植生生態  [4] 都市勢力");
                    prompt("> ")?;
                    if let Some(sub) = read_line(&mut lines)? {
                        self.current_view_mode = match sub.trim() {
                            "1" => ViewMode::Elevation,
                            "2" => ViewMode::Temperature,
                            "3" => ViewMode::Vegetation,
                            "4" => ViewMode::Political,
                            _ => self.current_view_mode,
                        };
                    }
                }
                "3" => {
                    println!("行動選択: [1] 労働に従事  [2] パンを食べる");
                    prompt("> ")?;
                    if let Some(sub) = read_line(&mut lines)? {
                        match sub.trim() {
                            "1" => println!("{}", player.perform_work()),
                            "2" => match player.consume_meal() {
                                Ok(msg) => println!("{}", msg),
                                Err(err) => println!("\x1B[31m{}\x1B[0m", err),
                            },
                            _ => {}
                        }
                    }
                }
                "4" => {
                    if let Some(market) = world.markets.first_mut() {
                        println!("市場取引: [1] パンを買う  [2] 木材を買う  [3] 小麦を売る");
                        prompt("> ")?;
                        if let Some(sub) = read_line(&mut lines)? {
                            let res = match sub.trim() {
                                "1" => player.buy_commodity(market, CommodityType::Bread, 2.0),
                                "2" => player.buy_commodity(market, CommodityType::Timber, 10.0),
                                "3" => player.sell_commodity(market, CommodityType::Grain, 10.0),
                                _ => Ok("キャンセルしました。".to_string()),
                            };
                            match res {
                                Ok(m) => println!("{}", m),
                                Err(e) => println!("\x1B[31m{}\x1B[0m", e),
                            }
                        }
                    }
                }
                "5" => {
                    if let Some(settlement) = world.settlements.first_mut() {
                        println!("建築選択: [1] 開拓農場  [2] 鍛冶工房");
                        prompt("> ")?;
                        if let Some(sub) = read_line(&mut lines)? {
                            let b_type = match sub.trim() {
                                "1" => Some(BuildingType::Farmstead),
                                "2" => Some(BuildingType::SmithyWorkshop),
                                _ => None,
                            };
                            if let Some(bt) = b_type {
                                match ConstructionService::construct_in_settlement(
                                    bt,
                                    settlement,
                                    &mut player.inventory,
                                    &mut player.coin_purse,
                                ) {
                                    Ok(b) => println!("建設完了: {:?} (ID: {})", b.building_type, b.id),
                                    Err(err) => println!("\x1B[31m{}\x1B[0m", err),
                                }
                            }
                        }
                    }
                }
                "6" => {
                    println!("因果チェーンを遡及検索する事象IDを入力してください: ");
                    prompt("> ")?;
                    if let Some(sub) = read_line(&mut lines)? {
                        let text = sub.trim();
                        match text.parse::<u64>() {
                            Ok(target_id) => {
                                let path = ChronicleEngine::trace_causal_lineage(
                                    &world.causality,
                                    genesis_core::causality::CausalityNodeId(target_id),
                                );
                                println!("\n=== 因果遡及パス ===");
                                for node in path {
                                    println!("  └── [Year {:04}] Node #{}: [{}] \"{}\"", node.year, node.node_id.0, node.category, node.headline);
                                }
                            }
                            Err(e) => println!(
                                "\x1B[31m事象ID '{}' を数値として解釈できません: {}\x1B[0m",
                                text, e
                            ),
                        }
                    }
                }
                "7" => {
                    println!("100年間の自律進化タイムラプスを開始します...");
                    for _ in 0..100 {
                        world.tick_step(TICKS_PER_YEAR);
                    }
                    let chronicle_file = "world_chronicle.md";
                    match WorldChronicleExporter::export_markdown_chronicle(&world, chronicle_file) {
                        Ok(()) => println!(
                            "\x1B[32m100年間の進行が完了し、`{}` に歴史年代記を出力しました。\x1B[0m",
                            chronicle_file
                        ),
                        Err(e) => println!(
                            "\x1B[31m100年間の進行は完了しましたが、`{}` への年代記出力に失敗しました: {}\x1B[0m",
                            chronicle_file, e
                        ),
                    }
                }
                "8" => {
                    println!("500年間・64x64グリッドの高負荷ストレステストを実行中...");
                    let bench = SimulationBenchmarkRunner::run_stress_benchmark(500, 64);
                    println!("\n================ BENCHMARK RESULT ================");
                    println!("  シミュレーション年数 : {} 年", bench.simulated_years);
                    println!("  合計実実行時間      : {} ms", bench.elapsed_millis);
                    println!("  計算スループット    : \x1B[32m{:.2} Years/sec\x1B[0m", bench.years_per_sec);
                    println!("  歴史因果ノード数    : {} nodes", bench.total_causality_nodes);
                    println!("  推定グリッドメモリ  : {:.2} MB", bench.peak_memory_estimate_mb);
                    println!("==================================================\n");
                }
                "9" => {
                    println!("シミュレーション状態を保存しています...");
                    let save_path = "save_world.bin.zst";
                    crate::persistence::WorldSnapshotService::save_world_compressed(&world, save_path)
                        .map_err(|e| {
                            io::Error::other(format!("`{save_path}` への保存に失敗しました: {e}"))
                        })?;
                    println!("保存完了。ゲームを終了します。");
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
