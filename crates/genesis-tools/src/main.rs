use genesis_core::time::TICKS_PER_YEAR;
use genesis_sim::benchmark::SimulationBenchmarkRunner;
use genesis_sim::chronicle_export::WorldChronicleExporter;
use genesis_sim::config::WorldGenesisConfig;
use genesis_sim::persistence::WorldSnapshotService;
use genesis_sim::world::WorldSimulation;
use std::env;

/// マップは size² のグリッドを確保するので、入力をそのまま信じると
/// 数字をひとつ間違えるだけでメモリを使い尽くす。
const MAX_MAP_SIZE: usize = 4096;
const MAX_YEARS: u32 = 1_000_000;

fn parse_map_size(s: Option<&String>) -> usize {
    s.and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(64)
        .clamp(8, MAX_MAP_SIZE)
}

fn parse_years(s: Option<&String>, default: u32) -> u32 {
    s.and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(default)
        .min(MAX_YEARS)
}

fn print_usage() {
    println!("WORLD GENESIS - Command Line Tools");
    println!("Usage:");
    println!("  genesis-tools generate <seed_hex> <map_size> <output_path>");
    println!("  genesis-tools simulate <years> <map_size> <chronicle_output_path>");
    println!("  genesis-tools bench <years> <map_size>");
    println!();
    println!("Examples:");
    println!("  genesis-tools generate 0xDEADBEEF 128 world_save.bin.zst");
    println!("  genesis-tools simulate 200 64 chronicle.md");
    println!("  genesis-tools bench 1000 64");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "generate" => {
            if args.len() < 5 {
                print_usage();
                return;
            }
            let seed_str = args[2].trim_start_matches("0x");
            let seed = u64::from_str_radix(seed_str, 16).unwrap_or(0xCAFE_BABE);
            let size = parse_map_size(args.get(3));
            let out_path = &args[4];

            println!(
                "[*] 世界生成開始: Seed=0x{:X}, Size={}x{}",
                seed, size, size
            );
            let config = WorldGenesisConfig {
                seed,
                map_width: size,
                map_height: size,
                plate_count: 8,
                sea_level: 0.0,
                solar_luminosity: 1.0,
                axial_tilt_deg: 23.44,
            };

            let mut world = WorldSimulation::new(config);
            world.bootstrap_genesis();

            WorldSnapshotService::save_world_compressed(&world, out_path)
                .expect("セーブ保存に失敗しました");
            println!(
                "[+] 生成完了: 圧縮スナップショット `{}` を出力しました。",
                out_path
            );
        }
        "simulate" => {
            if args.len() < 5 {
                print_usage();
                return;
            }
            let years = parse_years(args.get(2), 100);
            let size = parse_map_size(args.get(3));
            let chronicle_path = &args[4];

            println!(
                "[*] バッチシミュレーション開始: {}年間進行 (Size: {}x{})...",
                years, size, size
            );
            let config = WorldGenesisConfig {
                seed: 0x9876_5432,
                map_width: size,
                map_height: size,
                plate_count: 8,
                sea_level: 0.0,
                solar_luminosity: 1.0,
                axial_tilt_deg: 23.44,
            };

            let mut world = WorldSimulation::new(config);
            world.bootstrap_genesis();

            for y in 1..=years {
                world.tick_step(TICKS_PER_YEAR);
                if y % 50 == 0 || y == years {
                    println!("    進行度: Year {:04} / {:04} 完了", y, years);
                }
            }

            WorldChronicleExporter::export_markdown_chronicle(&world, chronicle_path)
                .expect("年代記出力に失敗しました");
            println!(
                "[+] シミュレーション完了: 歴史年代記 `{}` を出力しました。",
                chronicle_path
            );
        }
        "bench" => {
            let years = parse_years(args.get(2), 500);
            let size = parse_map_size(args.get(3));

            println!(
                "[*] ベンチマーク実行中: {}年間, マップ解像度: {}x{} ...",
                years, size, size
            );
            let bench = SimulationBenchmarkRunner::run_stress_benchmark(years, size);

            println!("\n================ BENCHMARK REPORT ================");
            println!("  シミュレーション年数 : {} 年", bench.simulated_years);
            println!("  実実行時間          : {} ms", bench.elapsed_millis);
            println!(
                "  計算スループット    : {:.2} Years/sec",
                bench.years_per_sec
            );
            println!(
                "  記録因果イベント数  : {} nodes",
                bench.total_causality_nodes
            );
            println!(
                "  推定グリッドメモリ  : {:.2} MB",
                bench.peak_memory_estimate_mb
            );
            println!("==================================================\n");
        }
        _ => print_usage(),
    }
}
