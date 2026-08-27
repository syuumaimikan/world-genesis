use genesis_core::time::TICKS_PER_YEAR;
use genesis_sim::benchmark::SimulationBenchmarkRunner;
use genesis_sim::chronicle_export::WorldChronicleExporter;
use genesis_sim::config::WorldGenesisConfig;
use genesis_sim::persistence::WorldSnapshotService;
use genesis_sim::world::WorldSimulation;
use std::env;
use std::process::ExitCode;

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

/// 使い方の誤りは終了コード 2、実行中の失敗は 1 で区別する。
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Usage(message)) => {
            eprintln!("[!] {message}");
            print_usage();
            ExitCode::from(EXIT_USAGE)
        }
        Err(CliError::Failed(message)) => {
            eprintln!("[!] {message}");
            ExitCode::FAILURE
        }
    }
}

enum CliError {
    /// 引数が足りない・解釈できない。
    Usage(String),
    /// 処理の途中で失敗した。
    Failed(String),
}

/// 引数を解釈し、既定値へ黙って落ちずに誤りを報告する。
fn parse_arg<T: std::str::FromStr>(args: &[String], index: usize, name: &str) -> Result<T, CliError>
where
    T::Err: std::fmt::Display,
{
    let raw = args
        .get(index)
        .ok_or_else(|| CliError::Usage(format!("引数 <{name}> が指定されていません")))?;
    raw.parse::<T>()
        .map_err(|e| CliError::Usage(format!("<{name}> の値 '{raw}' を解釈できません: {e}")))
}

/// 省略時は既定値を使うが、書かれている値が壊れていれば黙って捨てずに報告する。
fn parse_optional_arg<T: std::str::FromStr>(
    args: &[String],
    index: usize,
    name: &str,
    default: T,
) -> Result<T, CliError>
where
    T::Err: std::fmt::Display,
{
    match args.get(index) {
        Some(raw) => raw
            .parse::<T>()
            .map_err(|e| CliError::Usage(format!("<{name}> の値 '{raw}' を解釈できません: {e}"))),
        None => Ok(default),
    }
}

fn run() -> Result<(), CliError> {
    let args: Vec<String> = env::args().collect();
    let Some(command) = args.get(1) else {
        print_usage();
        return Ok(());
    };

    match command.as_str() {
        "generate" => {
            let seed_arg = args
                .get(2)
                .ok_or_else(|| CliError::Usage("引数 <seed_hex> が指定されていません".to_string()))?;
            let seed_str = seed_arg.trim_start_matches("0x");
            let seed = u64::from_str_radix(seed_str, 16).map_err(|e| {
                CliError::Usage(format!(
                    "<seed_hex> の値 '{seed_arg}' を16進数として解釈できません: {e}"
                ))
            })?;
            let size: usize = parse_arg(&args, 3, "map_size")?;
            let out_path = args
                .get(4)
                .ok_or_else(|| CliError::Usage("引数 <output_path> が指定されていません".to_string()))?;

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

            WorldSnapshotService::save_world_compressed(&world, out_path).map_err(|e| {
                CliError::Failed(format!("`{out_path}` へのセーブ保存に失敗しました: {e}"))
            })?;
            println!(
                "[+] 生成完了: 圧縮スナップショット `{}` を出力しました。",
                out_path
            );
        }
        "simulate" => {
            let years: u32 = parse_arg(&args, 2, "years")?;
            let size: usize = parse_arg(&args, 3, "map_size")?;
            let chronicle_path = args.get(4).ok_or_else(|| {
                CliError::Usage("引数 <chronicle_output_path> が指定されていません".to_string())
            })?;

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

            WorldChronicleExporter::export_markdown_chronicle(&world, chronicle_path).map_err(
                |e| {
                    CliError::Failed(format!(
                        "`{chronicle_path}` への年代記出力に失敗しました: {e}"
                    ))
                },
            )?;
            println!(
                "[+] シミュレーション完了: 歴史年代記 `{}` を出力しました。",
                chronicle_path
            );
        }
        "bench" => {
            let years: u32 = parse_optional_arg(&args, 2, "years", 500)?;
            let size: usize = parse_optional_arg(&args, 3, "map_size", 64)?;

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
        other => {
            return Err(CliError::Usage(format!("不明なコマンド '{other}'")));
        }
    }

    Ok(())
}
