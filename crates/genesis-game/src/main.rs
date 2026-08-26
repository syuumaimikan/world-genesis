//! World Genesis — ボクセル3Dクライアント。
//!
//! アプリ全体の組み立て：状態遷移、システムの登録と実行順、
//! そして起動時の初期化（設定・セーブ・プラグインの読み込み）。

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::PresentMode;
use genesis_game::actors::WorldTime;
use genesis_game::ai;
use genesis_game::blocks::BlockRegistry;
use genesis_game::blocky::{animate_limbs, BlockyAssets};
use genesis_game::chronicle::LocalChronicle;
use genesis_game::dev::{dev_script_system, DevScript};
use genesis_game::game;
use genesis_game::hud;
use genesis_game::items::ItemRegistry;
use genesis_game::menu::{self, AppState};
use genesis_game::plugins::PluginManager;
use genesis_game::saves::SaveManager;
use genesis_game::settings::GameSettings;
use genesis_game::streaming;
use genesis_game::ui_theme::{resolve_ui_font, UiFont};
use std::path::PathBuf;

/// セーブ・設定・プラグインを置く場所。
/// 実行ファイルの隣に `run/` を作る（プロジェクト内で完結させるため）。
fn data_root() -> PathBuf {
    // カレントディレクトリ優先。開発中は `cargo run` の作業ディレクトリになる。
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join("run")
}

fn main() {
    let root = data_root();
    if let Err(e) = std::fs::create_dir_all(&root) {
        eprintln!("データフォルダを作成できません ({}): {e}", root.display());
    }

    // 初回起動時にサンプルプラグインを配置する。
    if let Err(e) = PluginManager::write_example_plugin(&root) {
        eprintln!("サンプルプラグインを書き出せません: {e}");
    }

    let settings = GameSettings::load(&root);
    let plugin_mgr = PluginManager::scan(&root, &settings.enabled_plugins);
    let save_mgr = SaveManager::new(root.clone());

    let mut app = App::new();
    if let Some(script) = DevScript::from_env() {
        app.insert_resource(script);
    }

    app
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "World Genesis — 自律世界シミュレータ".into(),
                        resolution: (1600.0_f32, 900.0_f32).into(),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: "assets".into(),
                    ..default()
                }),
        )
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .init_state::<AppState>()
        .add_event::<menu::MenuActionEvent>()
        // --- グローバルなリソース ---
        .insert_resource(settings)
        .insert_resource(plugin_mgr)
        .insert_resource(menu::SaveManagerRes(save_mgr))
        .insert_resource(DataRoot(root))
        .init_resource::<menu::UiDirty>()
        .init_resource::<menu::ReturnTo>()
        .init_resource::<menu::CreateWorldForm>()
        .init_resource::<menu::PendingDelete>()
        .init_resource::<menu::SaveListCache>()
        .init_resource::<menu::PendingLoad>()
        .init_resource::<menu::SaveRequest>()
        .init_resource::<menu::Toast>()
        .init_resource::<hud::DebugOverlay>()
        .init_resource::<hud::ChroniclePanel>()
        .init_resource::<BlockRegistry>()
        .insert_resource(WorldTime::default())
        .insert_resource(LocalChronicle::new())
        .insert_resource(ai::PopulationTracker::new())
        .insert_resource(ai::ThreatBoard::default())
        .insert_resource(ai::SpawnTimers::default())
        .insert_resource(game::DialogueState::default())
        .insert_resource(game::MiningState::default())
        .insert_resource(streaming::StreamConfig::default())
        .insert_resource(ClearColor(Color::rgb(0.05, 0.06, 0.09)))
        // --- 起動時の初期化 ---
        .add_systems(PreStartup, bootstrap_resources)
        .add_systems(Startup, spawn_ui_camera)
        // --- メニュー（常時動く） ---
        .add_systems(
            Update,
            (
                menu::rebuild_ui_system,
                menu::button_visual_system,
                menu::button_press_system,
                menu::menu_action_system,
                menu::text_input_system,
                menu::cursor_grab_system,
                menu::toast_system,
            )
                .chain(),
        )
        .add_systems(OnEnter(AppState::Title), menu::mark_ui_dirty_on_state_change)
        .add_systems(OnEnter(AppState::WorldSelect), (menu::mark_ui_dirty_on_state_change, refresh_save_list))
        .add_systems(OnEnter(AppState::CreateWorld), menu::mark_ui_dirty_on_state_change)
        .add_systems(OnEnter(AppState::Settings), menu::mark_ui_dirty_on_state_change)
        .add_systems(OnEnter(AppState::Plugins), menu::mark_ui_dirty_on_state_change)
        .add_systems(OnEnter(AppState::LoadingWorld), menu::mark_ui_dirty_on_state_change)
        .add_systems(OnEnter(AppState::Paused), menu::mark_ui_dirty_on_state_change)
        .add_systems(OnEnter(AppState::InGame), menu::mark_ui_dirty_on_state_change)
        // --- 世界の読み込みと破棄 ---
        .add_systems(OnEnter(AppState::LoadingWorld), game::enter_world_system)
        .add_systems(OnEnter(AppState::Title), game::exit_world_system)
        .add_systems(OnEnter(AppState::InGame), hud::spawn_hud)
        .add_systems(OnExit(AppState::InGame), keep_hud_when_pausing)
        // --- 読み込み中も世界は組み上がり続ける ---
        .add_systems(
            Update,
            (
                streaming::chunk_generation_system,
                streaming::chunk_meshing_system,
                game::loading_progress_system,
            )
                .run_if(in_state(AppState::LoadingWorld)),
        )
        // --- ゲーム中 ---
        .add_systems(
            Update,
            (
                game::advance_time_system,
                game::player_control_system,
                game::player_interaction_system,
                game::player_vitals_system,
                game::projectile_system,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (
                ai::collect_threats_system,
                ai::npc_ai_system,
                ai::wildlife_ai_system,
                ai::actor_movement_system,
                ai::combat_system,
                ai::dying_system,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (
                ai::spawn_village_npcs_system,
                ai::spawn_wildlife_system,
                ai::despawn_far_npcs_system,
                ai::despawn_far_wildlife_system,
                ai::count_population_system,
            )
                .run_if(in_state(AppState::InGame)),
        )
        // --- ゲーム中とポーズ中の両方で動くもの ---
        .add_systems(
            Update,
            (
                streaming::chunk_generation_system,
                streaming::chunk_meshing_system,
                streaming::chunk_unload_system,
                game::sky_system,
                game::held_light_system,
                game::apply_settings_system,
                game::save_system,
                animate_limbs,
                hud::update_hud_system,
                hud::crosshair_visibility_system,
            )
                .run_if(in_state(AppState::InGame).or_else(in_state(AppState::Paused))),
        )
        .add_systems(
            Update,
            (
                hud::hud_toggle_system,
                hud::time_control_system,
                hud::target_info_system,
                genesis_game::actors::sync_limb_animation,
            )
                .run_if(in_state(AppState::InGame)),
        )
        // --- 開発用の自動操作（WG_SCRIPT=1 のときだけ） ---
        .add_systems(Update, dev_script_system.run_if(resource_exists::<DevScript>))
        // --- ポーズ切り替え ---
        .add_systems(
            Update,
            menu::pause_toggle_system
                .run_if(in_state(AppState::InGame).or_else(in_state(AppState::Paused))),
        )
        .run();
}

#[derive(Resource)]
pub struct DataRoot(pub PathBuf);

/// フォント・アイテムなど、アセットに依存するリソースを最初に用意する。
fn bootstrap_resources(
    mut commands: Commands,
    mut fonts: ResMut<Assets<Font>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    blocks: Res<BlockRegistry>,
    root: Res<DataRoot>,
    mut dirty: ResMut<menu::UiDirty>,
) {
    // フォントは assets/ の中を先に探し、無ければシステムから拾う。
    let assets_dir = root.0.parent().map(|p| p.join("assets")).unwrap_or_else(|| PathBuf::from("assets"));
    let font: UiFont = resolve_ui_font(&assets_dir, &mut fonts);
    commands.insert_resource(font);

    commands.insert_resource(ItemRegistry::build(&blocks));
    commands.insert_resource(BlockyAssets::new(&mut meshes));
    commands.insert_resource(streaming::VoxelMaterials::new(&mut materials));

    // 最初の画面（タイトル）を組み立てる。
    dirty.0 = true;
}

/// UI 用の 2D カメラ。3D カメラが無い画面でも UI を描くために常駐させる。
fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle {
        camera: Camera {
            order: 1,
            // 3D の描画結果を消さない。
            clear_color: ClearColorConfig::None,
            ..default()
        },
        ..default()
    });
}

fn refresh_save_list(save_mgr: Res<menu::SaveManagerRes>, mut cache: ResMut<menu::SaveListCache>) {
    cache.slots = save_mgr.0.list_saves();
    cache.loaded = true;
}

/// ポーズへ入るときは HUD を残し、タイトルへ戻るときだけ片付ける。
fn keep_hud_when_pausing(
    state: Res<State<AppState>>,
    commands: Commands,
    hud: Query<Entity, With<hud::HudRoot>>,
) {
    if *state.get() == AppState::Paused {
        return;
    }
    hud::despawn_hud(commands, hud);
}
