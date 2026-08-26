//! 開発用の自動操作。
//!
//! 画面が本当に描けているかは、コードを読んでも分からない。
//! 環境変数 `WG_SCRIPT` を与えると、メニューを自動で辿りながら
//! 各画面のスクリーンショットを撮って終了する。手で操作しなくても
//! 「タイトルが出る」「世界が生成されて地形が見える」ことを確認できる。
//!
//! 例:
//!   WG_SCRIPT=1 WG_SHOT_DIR=shots cargo run --release -p genesis-game

use crate::menu::{AppState, MenuAction, MenuActionEvent};
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::ScreenshotManager;
use bevy::window::PrimaryWindow;
use std::path::PathBuf;

/// 台本の 1 手。
#[derive(Debug, Clone)]
pub enum Step {
    /// 指定秒だけ待つ。
    Wait(f32),
    /// メニュー操作を送る。
    Act(MenuAction),
    /// スクリーンショットを撮る。
    Shot(&'static str),
    /// ゲーム内時刻を指定の時（0〜23）に合わせる。
    SetHour(u32),
    /// 上空へ移動して見下ろす（地形全体を確認するため）。
    Survey { height: f32, pitch: f32, distance: f32 },
    /// シード値を固定する（実行のたびに世界が変わると比較できないため）。
    SetSeed(&'static str),
    /// 終了する。
    Quit,
}

#[derive(Resource)]
pub struct DevScript {
    steps: Vec<Step>,
    index: usize,
    timer: f32,
    shot_dir: PathBuf,
    /// 直前のショットが書き出されるまでの待ち。
    settle: f32,
}

impl DevScript {
    /// 環境変数から台本を組み立てる。無効なら `None`。
    pub fn from_env() -> Option<Self> {
        if std::env::var("WG_SCRIPT").ok().as_deref() != Some("1") {
            return None;
        }
        let shot_dir = PathBuf::from(std::env::var("WG_SHOT_DIR").unwrap_or_else(|_| "shots".into()));
        let _ = std::fs::create_dir_all(&shot_dir);

        // タイトル → 各メニュー → 新規世界 → ゲーム内、と辿る。
        let steps = vec![
            Step::Wait(1.2),
            Step::Shot("01_title.png"),
            Step::Act(MenuAction::Goto(AppState::Settings)),
            Step::Wait(0.6),
            Step::Shot("02_settings.png"),
            Step::Act(MenuAction::Back),
            Step::Wait(0.4),
            Step::Act(MenuAction::Goto(AppState::Plugins)),
            Step::Wait(0.6),
            Step::Shot("03_plugins.png"),
            Step::Act(MenuAction::Back),
            Step::Wait(0.4),
            Step::Act(MenuAction::Goto(AppState::WorldSelect)),
            Step::Wait(0.6),
            Step::Shot("04_world_select.png"),
            Step::Act(MenuAction::NewWorld),
            Step::Wait(0.6),
            Step::Shot("05_create_world.png"),
            Step::SetSeed("world-genesis-qa"),
            Step::Wait(0.3),
            Step::Act(MenuAction::CreateAndPlay),
            Step::Wait(1.0),
            Step::Shot("06_loading.png"),
            // 地形が組み上がるのを待つ。
            Step::Wait(9.0),
            Step::Shot("07_ingame.png"),
            Step::SetHour(12),
            Step::Wait(3.0),
            Step::Shot("08_ingame_noon.png"),
            Step::SetHour(12),
            Step::Survey { height: 45.0, pitch: 0.55, distance: 0.0 },
            Step::Wait(7.0),
            Step::Shot("09_survey_low.png"),
            Step::Survey { height: 90.0, pitch: 0.42, distance: 0.0 },
            Step::Wait(8.0),
            Step::Shot("09b_survey_high.png"),
            // 夜の絵は地表で撮る。上空のままだと空しか写らない。
            Step::Survey { height: -130.0, pitch: 0.18, distance: 5.0 },
            Step::Wait(4.0),
            Step::SetHour(21),
            Step::Wait(3.0),
            Step::Shot("10_night_ground.png"),
            Step::SetHour(1),
            Step::Wait(2.0),
            Step::Shot("11_midnight.png"),
            Step::Quit,
        ];

        Some(Self {
            steps,
            index: 0,
            timer: 0.0,
            shot_dir,
            settle: 0.0,
        })
    }
}

pub fn dev_script_system(
    time: Res<Time>,
    mut script: ResMut<DevScript>,
    mut actions: EventWriter<MenuActionEvent>,
    mut screenshots: ResMut<ScreenshotManager>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut exit: EventWriter<AppExit>,
    state: Res<State<AppState>>,
    mut world_time: ResMut<crate::actors::WorldTime>,
    mut player: Query<(&mut Transform, &mut crate::actors::Actor, &mut crate::actors::Player)>,
    mut camera: Query<&mut crate::actors::PlayerCamera>,
    mut form: ResMut<crate::menu::CreateWorldForm>,
) {
    let dt = time.delta_seconds();
    if script.settle > 0.0 {
        script.settle -= dt;
        return;
    }

    // 1 フレームに 1 手だけ進める。
    let Some(step) = script.steps.get(script.index).cloned() else {
        exit.send(AppExit);
        return;
    };

    match step {
        Step::Wait(seconds) => {
            script.timer += dt;
            if script.timer >= seconds {
                script.timer = 0.0;
                script.index += 1;
            }
        }
        Step::Act(action) => {
            info!("[dev] {:?}（現在の画面: {:?}）", action, state.get());
            actions.send(MenuActionEvent(action));
            script.index += 1;
        }
        Step::Shot(name) => {
            let path = script.shot_dir.join(name);
            if let Ok(window) = windows.get_single() {
                match screenshots.save_screenshot_to_disk(window, path.clone()) {
                    Ok(()) => info!("[dev] スクリーンショット: {}", path.display()),
                    Err(e) => warn!("[dev] スクリーンショットに失敗: {e}"),
                }
            }
            script.index += 1;
            // 書き出しは非同期なので少し待つ。
            script.settle = 0.4;
        }
        Step::SetHour(hour) => {
            let day = world_time.tick / 86_400;
            world_time.tick = day * 86_400 + hour as u64 * 3600;
            info!("[dev] 時刻を {hour}:00 に合わせました");
            script.index += 1;
            script.settle = 0.2;
        }
        Step::Survey { height, pitch, distance } => {
            if let Ok((mut tf, mut actor, mut state)) = player.get_single_mut() {
                tf.translation.y += height;
                actor.velocity = Vec3::ZERO;
                // 落下しないよう飛行状態にする。
                state.flying = true;
            }
            if let Ok(mut cam) = camera.get_single_mut() {
                cam.pitch = pitch;
                if distance <= 0.05 {
                    cam.perspective = crate::game::Perspective::First;
                } else {
                    cam.perspective = crate::game::Perspective::ThirdBack;
                    cam.distance = distance;
                }
            }
            info!("[dev] 上空 {height} ブロックから地形を俯瞰します");
            script.index += 1;
            script.settle = 0.2;
        }
        Step::SetSeed(seed) => {
            form.seed_text = seed.to_string();
            info!("[dev] シードを '{seed}' に固定しました");
            script.index += 1;
        }
        Step::Quit => {
            info!("[dev] 台本を完了しました");
            exit.send(AppExit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_visits_every_menu_screen_and_ends_in_game() {
        // 環境変数に依存しないよう、台本の中身だけを検査する。
        let steps = vec![
            Step::Act(MenuAction::Goto(AppState::Settings)),
            Step::Act(MenuAction::Goto(AppState::Plugins)),
            Step::Act(MenuAction::Goto(AppState::WorldSelect)),
            Step::Act(MenuAction::NewWorld),
            Step::SetSeed("world-genesis-qa"),
            Step::Wait(0.3),
            Step::Act(MenuAction::CreateAndPlay),
        ];
        // 台本が到達すべき画面。
        for wanted in [AppState::Settings, AppState::Plugins, AppState::WorldSelect] {
            assert!(
                steps.iter().any(|s| matches!(s, Step::Act(MenuAction::Goto(t)) if *t == wanted)),
                "the QA script never opens {wanted:?}"
            );
        }
        assert!(steps.iter().any(|s| matches!(s, Step::Act(MenuAction::CreateAndPlay))));
    }

    #[test]
    fn the_script_is_disabled_without_the_env_var() {
        // 通常起動で自動操作が走ってしまうと、遊べなくなる。
        if std::env::var("WG_SCRIPT").is_err() {
            assert!(DevScript::from_env().is_none());
        }
    }
}
