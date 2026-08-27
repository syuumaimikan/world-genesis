use genesis_renderer::camera::OrbitCamera;
use genesis_renderer::mesh::TerrainMeshGenerator;
use genesis_renderer::sun::CelestialLighting;
use genesis_sim::config::WorldGenesisConfig;
use genesis_sim::interactive::InteractiveController;
use genesis_sim::player::PlayerCharacter;
use genesis_sim::world::WorldSimulation;
use genesis_ui::hud::HudViewModel;
use glam::Vec2;

fn main() -> std::process::ExitCode {
    println!("============================================================");
    println!("          WORLD GENESIS: INTEGRATED GAME & 3D ENGINE        ");
    println!("============================================================");

    let config = WorldGenesisConfig {
        seed: 0xCAFE_BABE,
        map_width: 64,
        map_height: 64,
        plate_count: 8,
        sea_level: 0.0,
        solar_luminosity: 1.0,
        axial_tilt_deg: 23.44,
    };

    println!("[1/3] 世界の原初創成 (Bootstrap Primordial Cycles)...");
    let mut world = WorldSimulation::new(config);
    world.bootstrap_genesis();

    println!("[2/3] 3D地形メッシュ & 天体ライティング初期化...");
    let mesh = TerrainMeshGenerator::build_terrain_mesh(
        &world.heightfield,
        &world.ecology.flora,
        world.config.sea_level,
        1.0,
        0.05,
    );
    println!(
        "      3D Mesh構築完了 (頂点数: {}, 三角形ポリゴン数: {})",
        mesh.vertices.len(),
        mesh.indices.len() / 3
    );

    let camera = OrbitCamera::default();
    let (sun_dir, sun_power) =
        CelestialLighting::compute_sun_direction(&world.planet_state, world.clock.current_tick);
    println!(
        "      Camera Position: {:?} | Sun Vector: {:?} (光度: {:.2})",
        camera.get_eye_position(),
        sun_dir,
        sun_power
    );

    let hud = HudViewModel::build(
        world.clock.current_tick,
        &world.settlements,
        &world.markets,
        world.causality.total_events(),
    );
    println!(
        "      HUD同期完了 (初期総人口: {}人 | 日付: {})",
        hud.total_population, hud.calendar_text
    );

    println!("[3/3] インタラクティブ・コンソールセッション起動...");
    let player = PlayerCharacter::new_citizen(1001, "Marcus", 1, Vec2::new(32.0, 32.0));
    let mut controller = InteractiveController::default();
    if let Err(e) = controller.run_interactive_loop(world, player) {
        eprintln!("[!] セッションを継続できません: {e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
