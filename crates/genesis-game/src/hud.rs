//! ゲーム中の画面表示：状態バー・ホットバー・照準・会話・世界史・デバッグ情報。
//!
//! HUD の各パーツは個別のマーカー型ではなく、`HudSlot` / `HudPanel` という
//! 1つの列挙で区別する。こうすると更新側が巨大な `Without<...>` の連鎖を
//! 持たずに済み、1つのクエリで全テキストを回せる。

use crate::actors::*;
use crate::ai::PopulationTracker;
use crate::biome::{biome_def, Biome, ALL_BIOMES};
use crate::chronicle::LocalChronicle;
use crate::chunk::ChunkPos;
use crate::game::{ActiveWorld, DialogueState, MiningState};
use crate::items::{Inventory, ItemRegistry, HOTBAR_SLOTS};
use crate::keybinds::Action;
use crate::menu::{AppState, Toast};
use crate::settings::GameSettings;
use crate::streaming::VoxelWorld;
use crate::ui_theme::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

#[derive(Component)]
pub struct HudRoot;

/// 文字を出す枠。
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudSlot {
    Status,
    Hotbar,
    Debug,
    Toast,
    Dialogue,
    Chronicle,
}

/// 大きさや表示を切り替える枠。
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudPanel {
    Root,
    DialogueBox,
    MiningBar,
    Crosshair,
}

/// デバッグ情報（F3）の表示切り替え。
#[derive(Resource, Default)]
pub struct DebugOverlay(pub bool);

/// 世界史パネル（H キー）の表示切り替え。
#[derive(Resource, Default)]
pub struct ChroniclePanel(pub bool);

pub fn spawn_hud(mut commands: Commands, font: Res<UiFont>) {
    let panel_bg = Color::rgba(0.04, 0.06, 0.10, 0.74);

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                // HUD 自体はクリックを吸わない。
                focus_policy: bevy::ui::FocusPolicy::Pass,
                ..default()
            },
            HudRoot,
            HudPanel::Root,
        ))
        .with_children(|root| {
            // --- 左上：状態 ---
            root.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(12.0),
                    left: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                background_color: BackgroundColor(panel_bg),
                ..default()
            })
            .with_children(|p| {
                p.spawn((TextBundle::from_section("", font.style(14.0, C_TEXT)), HudSlot::Status));
            });

            // --- 右上：デバッグ ---
            root.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(12.0),
                    right: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                background_color: BackgroundColor(panel_bg),
                ..default()
            })
            .with_children(|p| {
                p.spawn((TextBundle::from_section("", font.style(12.0, C_TEXT_DIM)), HudSlot::Debug));
            });

            // --- 中央：照準 ---
            root.spawn((
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(50.0),
                        top: Val::Percent(50.0),
                        width: Val::Px(6.0),
                        height: Val::Px(6.0),
                        margin: UiRect::new(Val::Px(-3.0), Val::Px(0.0), Val::Px(-3.0), Val::Px(0.0)),
                        ..default()
                    },
                    background_color: BackgroundColor(Color::rgba(1.0, 1.0, 1.0, 0.78)),
                    ..default()
                },
                HudPanel::Crosshair,
            ));

            // --- 採掘ゲージ ---
            root.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(55.0),
                    width: Val::Px(120.0),
                    height: Val::Px(6.0),
                    margin: UiRect::left(Val::Px(-60.0)),
                    ..default()
                },
                background_color: BackgroundColor(Color::rgba(0.0, 0.0, 0.0, 0.45)),
                ..default()
            })
            .with_children(|p| {
                p.spawn((
                    NodeBundle {
                        style: Style {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        background_color: BackgroundColor(C_ACCENT),
                        ..default()
                    },
                    HudPanel::MiningBar,
                ));
            });

            // --- 下部：ホットバー ---
            root.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(14.0),
                    left: Val::Percent(50.0),
                    width: Val::Px(900.0),
                    margin: UiRect::left(Val::Px(-450.0)),
                    justify_content: JustifyContent::Center,
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                background_color: BackgroundColor(Color::rgba(0.04, 0.06, 0.10, 0.80)),
                ..default()
            })
            .with_children(|p| {
                p.spawn((TextBundle::from_section("", font.style(14.0, C_TEXT)), HudSlot::Hotbar));
            });

            // --- 会話ウィンドウ ---
            root.spawn((
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(70.0),
                        left: Val::Percent(18.0),
                        width: Val::Percent(64.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(14.0)),
                        display: Display::None,
                        ..default()
                    },
                    background_color: BackgroundColor(Color::rgba(0.04, 0.06, 0.10, 0.94)),
                    ..default()
                },
                HudPanel::DialogueBox,
            ))
            .with_children(|p| {
                p.spawn((TextBundle::from_section("", font.style(15.0, C_TEXT)), HudSlot::Dialogue));
            });

            // --- 世界史パネル ---
            root.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(20.0),
                    left: Val::Px(14.0),
                    width: Val::Px(450.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                ..default()
            })
            .with_children(|p| {
                p.spawn((TextBundle::from_section("", font.style(12.0, C_TEXT_DIM)), HudSlot::Chronicle));
            });

            // --- トースト ---
            root.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(130.0),
                    left: Val::Percent(50.0),
                    width: Val::Px(760.0),
                    margin: UiRect::left(Val::Px(-380.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ..default()
            })
            .with_children(|p| {
                p.spawn((TextBundle::from_section("", font.style(15.0, C_ACCENT)), HudSlot::Toast));
            });
        });
}

pub fn despawn_hud(mut commands: Commands, hud: Query<Entity, With<HudRoot>>) {
    for e in hud.iter() {
        commands.entity(e).despawn_recursive();
    }
}

/// F3 / H キーの切り替え。
pub fn hud_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    mut debug: ResMut<DebugOverlay>,
    mut chronicle: ResMut<ChroniclePanel>,
) {
    if settings.keybinds.just_pressed(Action::ToggleDebug, &keys) {
        debug.0 = !debug.0;
    }
    if settings.keybinds.just_pressed(Action::ToggleChronicle, &keys) {
        chronicle.0 = !chronicle.0;
    }
}

const SPEED_LADDER: [f32; 6] = [0.0, 1.0, 5.0, 20.0, 100.0, 1000.0];

/// 時間倍率の操作。`,` で遅く、`.` で速く。
pub fn time_control_system(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    mut world_time: ResMut<WorldTime>,
    mut toast: ResMut<Toast>,
) {
    let binds = &settings.keybinds;
    let idx = current_speed_index(&world_time, &SPEED_LADDER);
    let changed = if binds.just_pressed(Action::TimeSlower, &keys) {
        Some(SPEED_LADDER[idx.saturating_sub(1)])
    } else if binds.just_pressed(Action::TimeFaster, &keys) {
        Some(SPEED_LADDER[(idx + 1).min(SPEED_LADDER.len() - 1)])
    } else {
        None
    };

    if let Some(speed) = changed {
        world_time.paused = speed <= 0.0;
        world_time.speed = speed.max(0.0);
        toast.show(
            if world_time.paused {
                "時間を停止しました。世界は静止する。".to_string()
            } else {
                format!("時間の流れ: {speed} 倍")
            },
            C_ACCENT,
        );
    }
}

fn current_speed_index(t: &WorldTime, speeds: &[f32]) -> usize {
    if t.paused {
        return 0;
    }
    speeds
        .iter()
        .position(|s| (*s - t.speed).abs() < 0.01)
        .unwrap_or(1)
}

/// HUD が読む値をひとまとめにする。
#[derive(SystemParam)]
pub struct HudContext<'w> {
    pub settings: Res<'w, GameSettings>,
    pub world_time: Res<'w, WorldTime>,
    pub world: Option<Res<'w, VoxelWorld>>,
    pub active: Option<Res<'w, ActiveWorld>>,
    pub tracker: Res<'w, PopulationTracker>,
    pub items: Res<'w, ItemRegistry>,
    pub mining: Res<'w, MiningState>,
    pub dialogue: Res<'w, DialogueState>,
    pub toast: Res<'w, Toast>,
    pub chronicle: Res<'w, LocalChronicle>,
    pub chronicle_panel: Res<'w, ChroniclePanel>,
    pub debug: Res<'w, DebugOverlay>,
    pub diagnostics: Res<'w, DiagnosticsStore>,
}

/// HUD の文字と枠を毎フレーム更新する。
pub fn update_hud_system(
    ctx: HudContext,
    player: Query<(&Transform, &Player, &Health, &Actor, &Inventory)>,
    mut texts: Query<(&HudSlot, &mut Text)>,
    mut panels: Query<(&HudPanel, &mut Style)>,
) {
    let Ok((tf, player_state, health, actor, inventory)) = player.get_single() else { return };
    let pos = tf.translation;

    // --- 現在地のバイオーム ---
    let biome = ctx
        .world
        .as_ref()
        .and_then(|w| {
            let cp = ChunkPos::from_world(pos.x, pos.z);
            w.chunks.get(&cp).map(|c| {
                let (ox, oz) = cp.origin();
                ALL_BIOMES
                    .get(c.biome_at(pos.x.floor() as i32 - ox, pos.z.floor() as i32 - oz) as usize)
                    .copied()
                    .unwrap_or(Biome::Plains)
            })
        })
        .unwrap_or(Biome::Plains);

    // --- 枠の表示切り替え ---
    for (panel, mut style) in panels.iter_mut() {
        match panel {
            HudPanel::Root => {
                style.display = if ctx.settings.show_hud { Display::Flex } else { Display::None };
            }
            HudPanel::DialogueBox => {
                style.display = if ctx.dialogue.speaker.is_some() { Display::Flex } else { Display::None };
            }
            HudPanel::MiningBar => {
                let pct = if ctx.mining.required > 0.0 && ctx.mining.target.is_some() {
                    (ctx.mining.progress / ctx.mining.required * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                style.width = Val::Percent(pct);
            }
            HudPanel::Crosshair => {}
        }
    }

    // --- 文字 ---
    for (slot, mut text) in texts.iter_mut() {
        let Some(section) = text.sections.first_mut() else { continue };
        match slot {
            HudSlot::Status => {
                let speed_label = if ctx.world_time.paused {
                    "停止".to_string()
                } else {
                    format!("{}倍", ctx.world_time.speed)
                };
                let temp_warning = if player_state.body_temp < 34.5 {
                    "  ⚠ 低体温"
                } else if player_state.body_temp > 39.0 {
                    "  ⚠ 高熱"
                } else {
                    ""
                };
                section.value = format!(
                    "{}\n\
                     {}日目 {:02}:{:02}（時間 {}）　{}\n\
                     体力 {:>3.0}/100　空腹 {:>3.0}/100　体温 {:.1}℃{}\n\
                     所持金 {:.0}　職業 {}　年齢 {:.0}歳\n\
                     座標 {:.0}, {:.0}, {:.0}{}",
                    ctx.active.as_ref().map(|a| a.meta.display_name.clone()).unwrap_or_default(),
                    ctx.world_time.day_number() + 1,
                    ctx.world_time.hour(),
                    ctx.world_time.minute(),
                    speed_label,
                    biome_def(biome).display_name,
                    health.current,
                    player_state.hunger,
                    player_state.body_temp,
                    temp_warning,
                    player_state.money,
                    player_state.profession,
                    player_state.age_days / 360.0,
                    pos.x, pos.y, pos.z,
                    if actor.in_liquid { "　【水中】" } else { "" },
                );
            }

            HudSlot::Hotbar => {
                let mut parts = Vec::new();
                for i in 0..HOTBAR_SLOTS {
                    let label = match inventory.get(i) {
                        Some(stack) => {
                            let name = ctx.items.display_name(&stack.key);
                            if stack.count > 1 {
                                format!("{name}×{}", stack.count)
                            } else {
                                name
                            }
                        }
                        None => "―".to_string(),
                    };
                    parts.push(if i == player_state.selected_slot {
                        format!("▶{}:{label}◀", i + 1)
                    } else {
                        format!(" {}:{label} ", i + 1)
                    });
                }
                section.value = parts.join("");
            }

            HudSlot::Dialogue => {
                if ctx.dialogue.speaker.is_some() {
                    section.value = format!(
                        "■ {}\n\n{}\n\n[F] 会話を終える",
                        ctx.dialogue.name, ctx.dialogue.text
                    );
                }
            }

            HudSlot::Chronicle => {
                section.value = if ctx.chronicle_panel.0 {
                    let mut s = String::from("【 世界史 】  [H] で閉じる\n");
                    let events = ctx.chronicle.recent(14);
                    if events.is_empty() {
                        s.push_str("まだ記録すべき出来事は起きていない。");
                    } else {
                        for e in events {
                            s.push_str(&format!("{} — {}\n", e.formatted_date(), e.title));
                        }
                    }
                    s
                } else {
                    String::new()
                };
            }

            HudSlot::Toast => {
                if ctx.toast.remaining > 0.0 {
                    section.value = ctx.toast.message.clone();
                    section.style.color = ctx.toast.color.with_a((ctx.toast.remaining / 1.2).min(1.0));
                } else {
                    section.value.clear();
                }
            }

            HudSlot::Debug => {
                if !ctx.debug.0 {
                    section.value =
                        "[F3] 詳細  [H] 世界史  [,] [.] 時間速度  [F] 会話  [Esc] メニュー".to_string();
                    continue;
                }
                let fps = ctx
                    .diagnostics
                    .get(&FrameTimeDiagnosticsPlugin::FPS)
                    .and_then(|d| d.smoothed())
                    .unwrap_or(0.0);
                let stats = ctx.world.as_ref().map(|w| w.stats).unwrap_or_default();
                section.value = format!(
                    "FPS {fps:.0}\n\
                     チャンク  生成済 {} / 描画 {} / 生成待ち {} / メッシュ待ち {}\n\
                     直近メッシュのクアッド数 {}\n\
                     改変チャンク {}（保存対象）\n\
                     村人 {} / 動物 {}\n\
                     描画距離 {} チャンク\n\
                     バイオーム {}\n\
                     [F3] 閉じる",
                    stats.loaded_chunks,
                    stats.rendered_chunks,
                    stats.pending_gen,
                    stats.pending_mesh,
                    stats.quads_last_build,
                    stats.modified_chunks,
                    ctx.tracker.npc_count,
                    ctx.tracker.creature_count,
                    ctx.settings.render_distance,
                    biome_def(biome).display_name,
                );
            }
        }
    }
}

/// 照準が向いている相手の名前を出す。
pub fn target_info_system(
    dialogue: Res<DialogueState>,
    mut toast: ResMut<Toast>,
    camera: Query<&Transform, With<PlayerCamera>>,
    player: Query<&Transform, With<Player>>,
    named: Query<(&Transform, &Nameplate), Without<PlayerCamera>>,
) {
    // 会話中や、他のメッセージが出ている間は上書きしない。
    if dialogue.speaker.is_some() || toast.remaining > 0.2 {
        return;
    }
    let (Ok(cam), Ok(player_tf)) = (camera.get_single(), player.get_single()) else {
        return;
    };
    let eye = player_tf.translation + Vec3::Y * 1.62;
    let dir = *cam.forward();

    let mut best: Option<(f32, &Nameplate)> = None;
    for (tf, plate) in named.iter() {
        let to = tf.translation + Vec3::Y * 0.9 - eye;
        let dist = to.length();
        if dist > 16.0 || dist < 0.2 {
            continue;
        }
        if to.normalize_or_zero().dot(dir) > 0.985 && best.map(|(d, _)| dist < d).unwrap_or(true) {
            best = Some((dist, plate));
        }
    }

    if let Some((dist, plate)) = best {
        toast.message = format!("{}  ({:.0}m)", plate.text, dist);
        toast.color = plate.color;
        // すぐ消える短い表示にして、他の通知を邪魔しない。
        toast.remaining = 0.15;
    }
}

/// ポーズ中は照準を隠す。
pub fn crosshair_visibility_system(
    state: Res<State<AppState>>,
    mut crosshair: Query<(&HudPanel, &mut Visibility)>,
) {
    let show = *state.get() == AppState::InGame;
    for (panel, mut visibility) in crosshair.iter_mut() {
        if *panel == HudPanel::Crosshair {
            *visibility = if show { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_stepping_walks_the_ladder_in_both_directions() {
        let mut t = WorldTime::default();
        assert_eq!(current_speed_index(&t, &SPEED_LADDER), 1, "the game should start at 1x");

        for expected in [5.0f32, 20.0, 100.0, 1000.0, 1000.0] {
            let idx = current_speed_index(&t, &SPEED_LADDER);
            t.speed = SPEED_LADDER[(idx + 1).min(SPEED_LADDER.len() - 1)];
            t.paused = t.speed <= 0.0;
            assert_eq!(t.speed, expected);
        }

        for expected in [100.0f32, 20.0, 5.0, 1.0, 0.0, 0.0] {
            let idx = current_speed_index(&t, &SPEED_LADDER);
            t.speed = SPEED_LADDER[idx.saturating_sub(1)];
            t.paused = t.speed <= 0.0;
            assert_eq!(t.speed, expected);
        }
        assert!(t.paused, "stepping all the way down should stop time");
    }

    #[test]
    fn a_paused_clock_reports_the_pause_slot() {
        let mut t = WorldTime::default();
        t.paused = true;
        t.speed = 20.0;
        assert_eq!(current_speed_index(&t, &SPEED_LADDER), 0);
    }

    #[test]
    fn an_unknown_speed_falls_back_to_normal() {
        let mut t = WorldTime::default();
        t.speed = 3.7;
        assert_eq!(current_speed_index(&t, &SPEED_LADDER), 1);
    }

    #[test]
    fn the_ladder_covers_the_speeds_the_spec_asks_for() {
        for wanted in [1.0f32, 5.0, 20.0, 100.0, 1000.0] {
            assert!(
                SPEED_LADDER.contains(&wanted),
                "time speed {wanted}x is not reachable"
            );
        }
        assert_eq!(SPEED_LADDER[0], 0.0, "there must be a pause step");
    }
}
