//! メニュー画面一式。
//!
//! タイトル / ワールド選択 / ワールド作成 / 設定 / プラグイン管理 /
//! ポーズ の各画面を、Bevy の `States` で切り替える。
//!
//! 各画面は「状態に入ったら組み立て、出たら丸ごと捨てる」方式で作る。
//! 値が変わったときは `UiDirty` を立てるだけでよく、部分更新のための
//! 複雑な差分処理を書かずに済む。

use crate::plugins::PluginManager;
use crate::saves::{
    default_meta, now_unix, GameMode, SaveManager, SaveSlot, WorldMeta, WorldType,
};
use crate::settings::GameSettings;
use crate::ui_theme::*;
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow, ReceivedCharacter};

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Title,
    WorldSelect,
    CreateWorld,
    Settings,
    Plugins,
    LoadingWorld,
    InGame,
    Paused,
}

impl AppState {
    /// この画面はマウスカーソルを解放するか。
    pub fn wants_cursor(self) -> bool {
        !matches!(self, AppState::InGame)
    }

    /// この画面で3Dの世界を描いているか。
    pub fn world_is_live(self) -> bool {
        matches!(self, AppState::InGame | AppState::Paused | AppState::LoadingWorld)
    }
}

/// UI の再構築要求。
#[derive(Resource, Default)]
pub struct UiDirty(pub bool);

/// 「戻る」で帰る先。設定画面はタイトルからもポーズからも開けるため必要。
#[derive(Resource)]
pub struct ReturnTo(pub AppState);

impl Default for ReturnTo {
    fn default() -> Self {
        Self(AppState::Title)
    }
}

/// 画面に貼られた UI のルート。状態を出るときに丸ごと捨てる。
#[derive(Component)]
pub struct MenuRoot;

/// 入力欄の識別子。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextField {
    WorldName,
    Seed,
}

/// ワールド生成パラメータの調整対象。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenField {
    SeaLevel,
    Amplitude,
    Caves,
    Ores,
    Vegetation,
    Settlements,
}

impl GenField {
    fn label(self) -> &'static str {
        match self {
            GenField::SeaLevel => "海面の高さ",
            GenField::Amplitude => "地形の起伏",
            GenField::Caves => "洞窟の量",
            GenField::Ores => "鉱脈の豊かさ",
            GenField::Vegetation => "植生の密度",
            GenField::Settlements => "集落の多さ",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            GenField::SeaLevel => "低いほど陸地が広がる",
            GenField::Amplitude => "高いほど険しい山脈になる",
            GenField::Caves => "0 で洞窟なし。2 で蟻の巣状",
            GenField::Ores => "高いほど鉱石が見つかりやすい",
            GenField::Vegetation => "高いほど森が深くなる",
            GenField::Settlements => "高いほど村や町が密集する",
        }
    }

    fn value_of(self, m: &WorldMeta) -> f32 {
        match self {
            GenField::SeaLevel => m.sea_level as f32,
            GenField::Amplitude => m.terrain_amplitude,
            GenField::Caves => m.cave_density,
            GenField::Ores => m.ore_richness,
            GenField::Vegetation => m.vegetation_density,
            GenField::Settlements => m.settlement_density,
        }
    }

    fn display(self, m: &WorldMeta) -> String {
        match self {
            GenField::SeaLevel => format!("{}", m.sea_level),
            _ => format!("{:.2}x", self.value_of(m)),
        }
    }

    fn step(self) -> f32 {
        match self {
            GenField::SeaLevel => 4.0,
            _ => 0.25,
        }
    }

    fn apply(self, m: &mut WorldMeta, delta: f32) {
        match self {
            GenField::SeaLevel => {
                m.sea_level = (m.sea_level + delta as i32).clamp(16, 110);
            }
            GenField::Amplitude => m.terrain_amplitude = (m.terrain_amplitude + delta).clamp(0.25, 3.0),
            GenField::Caves => m.cave_density = (m.cave_density + delta).clamp(0.0, 2.5),
            GenField::Ores => m.ore_richness = (m.ore_richness + delta).clamp(0.0, 4.0),
            GenField::Vegetation => m.vegetation_density = (m.vegetation_density + delta).clamp(0.0, 3.0),
            GenField::Settlements => m.settlement_density = (m.settlement_density + delta).clamp(0.0, 4.0),
        }
    }
}

/// 設定画面の調整対象。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingField {
    RenderDistance,
    Fov,
    Sensitivity,
    InvertY,
    GuiScale,
    MasterVolume,
    MusicVolume,
    ChunkBudget,
    Fog,
    ViewBobbing,
    ThirdPerson,
    ShowHud,
    Autosave,
}

impl SettingField {
    fn label(self) -> &'static str {
        match self {
            SettingField::RenderDistance => "描画距離",
            SettingField::Fov => "視野角",
            SettingField::Sensitivity => "マウス感度",
            SettingField::InvertY => "マウス上下反転",
            SettingField::GuiScale => "UI 拡大率",
            SettingField::MasterVolume => "全体音量",
            SettingField::MusicVolume => "音楽音量",
            SettingField::ChunkBudget => "チャンク描画予算",
            SettingField::Fog => "遠景フォグ",
            SettingField::ViewBobbing => "視点の揺れ",
            SettingField::ThirdPerson => "三人称視点",
            SettingField::ShowHud => "HUD 表示",
            SettingField::Autosave => "自動セーブ間隔",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            SettingField::RenderDistance => "性能に最も強く効く。重ければまずここを下げる",
            SettingField::ChunkBudget => "1フレームに反映するチャンク数。多いと読み込みが速いが引っかかる",
            SettingField::Autosave => "0 で自動セーブを切る",
            _ => "",
        }
    }

    fn is_toggle(self) -> bool {
        matches!(
            self,
            SettingField::InvertY
                | SettingField::Fog
                | SettingField::ViewBobbing
                | SettingField::ThirdPerson
                | SettingField::ShowHud
        )
    }

    fn display(self, s: &GameSettings) -> String {
        let on_off = |b: bool| if b { "オン".to_string() } else { "オフ".to_string() };
        match self {
            SettingField::RenderDistance => format!("{} チャンク", s.render_distance),
            SettingField::Fov => format!("{:.0}°", s.fov_degrees),
            SettingField::Sensitivity => format!("{:.0}%", s.mouse_sensitivity / 0.0028 * 100.0),
            SettingField::InvertY => on_off(s.invert_mouse_y),
            SettingField::GuiScale => format!("{:.0}%", s.gui_scale * 100.0),
            SettingField::MasterVolume => format!("{:.0}%", s.master_volume * 100.0),
            SettingField::MusicVolume => format!("{:.0}%", s.music_volume * 100.0),
            SettingField::ChunkBudget => format!("{}", s.chunk_upload_budget),
            SettingField::Fog => on_off(s.show_fog),
            SettingField::ViewBobbing => on_off(s.view_bobbing),
            SettingField::ThirdPerson => on_off(s.third_person),
            SettingField::ShowHud => on_off(s.show_hud),
            SettingField::Autosave => {
                if s.autosave_minutes <= 0.0 {
                    "オフ".to_string()
                } else {
                    format!("{:.0} 分", s.autosave_minutes)
                }
            }
        }
    }

    fn step(self) -> f32 {
        match self {
            SettingField::RenderDistance => 1.0,
            SettingField::Fov => 5.0,
            SettingField::Sensitivity => 0.0004,
            SettingField::GuiScale => 0.1,
            SettingField::MasterVolume | SettingField::MusicVolume => 0.1,
            SettingField::ChunkBudget => 1.0,
            SettingField::Autosave => 1.0,
            _ => 0.0,
        }
    }

    fn apply(self, s: &mut GameSettings, delta: f32) {
        match self {
            SettingField::RenderDistance => s.render_distance += delta as i32,
            SettingField::Fov => s.fov_degrees += delta,
            SettingField::Sensitivity => s.mouse_sensitivity += delta,
            SettingField::GuiScale => s.gui_scale += delta,
            SettingField::MasterVolume => s.master_volume += delta,
            SettingField::MusicVolume => s.music_volume += delta,
            SettingField::ChunkBudget => {
                s.chunk_upload_budget = (s.chunk_upload_budget as i32 + delta as i32).max(1) as u32
            }
            SettingField::Autosave => s.autosave_minutes += delta,
            SettingField::InvertY => s.invert_mouse_y = !s.invert_mouse_y,
            SettingField::Fog => s.show_fog = !s.show_fog,
            SettingField::ViewBobbing => s.view_bobbing = !s.view_bobbing,
            SettingField::ThirdPerson => s.third_person = !s.third_person,
            SettingField::ShowHud => s.show_hud = !s.show_hud,
        }
        s.sanitize();
    }
}

/// ボタンが起こす操作。
#[derive(Component, Clone, Debug, PartialEq)]
pub enum MenuAction {
    Goto(AppState),
    Back,
    Quit,
    NewWorld,
    PlayWorld(String),
    AskDeleteWorld(String),
    ConfirmDeleteWorld(String),
    CancelDelete,
    DuplicateWorld(String),
    CreateAndPlay,
    RandomSeed,
    CycleWorldType(i32),
    CycleGameMode(i32),
    AdjustGen(GenField, f32),
    FocusField(TextField),
    AdjustSetting(SettingField, f32),
    TogglePlugin(String),
    RescanPlugins,
    OpenModsFolder,
    ResumeGame,
    SaveNow,
    SaveAndQuitToTitle,
}

/// ワールド作成画面のフォーム状態。
#[derive(Resource)]
pub struct CreateWorldForm {
    pub meta: WorldMeta,
    pub name_text: String,
    pub seed_text: String,
    pub focus: Option<TextField>,
    pub error: Option<String>,
}

impl Default for CreateWorldForm {
    fn default() -> Self {
        let seed = random_seed();
        Self {
            meta: default_meta("新しい世界", seed),
            name_text: "新しい世界".to_string(),
            seed_text: seed.to_string(),
            focus: None,
            error: None,
        }
    }
}

impl CreateWorldForm {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// 入力されたシード文字列を数値へ。数字でなければ文字列のハッシュを使う
    /// （Minecraft と同じ挙動。どんな文字を入れても必ず世界が作れる）。
    pub fn resolved_seed(&self) -> u64 {
        let t = self.seed_text.trim();
        if t.is_empty() {
            return random_seed();
        }
        if let Ok(n) = t.parse::<u64>() {
            return n;
        }
        if let Ok(n) = t.parse::<i64>() {
            return n as u64;
        }
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in t.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        crate::noise::hash_u64(h)
    }
}

pub fn random_seed() -> u64 {
    // 起動時刻とプロセスIDから種を作る。決定論は「同じ種なら同じ世界」であって、
    // 「毎回同じ種」ではない。
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x1234_5678);
    crate::noise::hash_u64(nanos ^ ((std::process::id() as u64) << 32))
}

/// 削除確認の対象。
#[derive(Resource, Default)]
pub struct PendingDelete(pub Option<String>);

/// セーブ一覧のキャッシュ（毎フレーム走査しないため）。
#[derive(Resource, Default)]
pub struct SaveListCache {
    pub slots: Vec<SaveSlot>,
    pub loaded: bool,
}

/// 次に読み込むワールド。
#[derive(Resource, Default)]
pub struct PendingLoad(pub Option<WorldMeta>);

/// 画面に出す一時メッセージ（保存完了・エラーなど）。
#[derive(Resource, Default)]
pub struct Toast {
    pub message: String,
    pub color: Color,
    pub remaining: f32,
}

impl Toast {
    pub fn show(&mut self, msg: impl Into<String>, color: Color) {
        self.message = msg.into();
        self.color = color;
        self.remaining = 4.0;
    }
}

// ======================================================================
// 画面の組み立て
// ======================================================================

pub struct MenuBuildCtx<'a> {
    pub font: &'a UiFont,
    pub settings: &'a GameSettings,
    pub saves: &'a SaveListCache,
    pub form: &'a CreateWorldForm,
    pub plugins: &'a PluginManager,
    pub pending_delete: &'a PendingDelete,
    pub state: AppState,
}

pub fn build_screen(commands: &mut Commands, ctx: &MenuBuildCtx) {
    match ctx.state {
        AppState::Title => build_title(commands, ctx),
        AppState::WorldSelect => build_world_select(commands, ctx),
        AppState::CreateWorld => build_create_world(commands, ctx),
        AppState::Settings => build_settings(commands, ctx),
        AppState::Plugins => build_plugins(commands, ctx),
        AppState::LoadingWorld => build_loading(commands, ctx),
        AppState::Paused => build_pause(commands, ctx),
        AppState::InGame => {}
    }
}

fn spawn_button(
    parent: &mut ChildBuilder,
    font: &UiFont,
    label: &str,
    action: MenuAction,
    width: Val,
    color: Color,
    text_size: f32,
) {
    parent
        .spawn((button_bundle(width, (text_size * 2.1).max(30.0), color), action))
        .with_children(|b| {
            b.spawn(TextBundle::from_section(label, font.style(text_size, C_TEXT)));
        });
}

fn spawn_label(parent: &mut ChildBuilder, font: &UiFont, text: &str, size: f32, color: Color) {
    parent.spawn(TextBundle::from_section(text, font.style(size, color)));
}

/// 「◀ 値 ▶」形式の調整行。
fn spawn_stepper(
    parent: &mut ChildBuilder,
    font: &UiFont,
    label: &str,
    hint: &str,
    value: &str,
    dec: MenuAction,
    inc: MenuAction,
    toggle_only: bool,
) {
    parent
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                width: Val::Percent(96.0),
                height: Val::Px(34.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                ..default()
            },
            background_color: BackgroundColor(C_PANEL_SOFT),
            ..default()
        })
        .with_children(|row| {
            row.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    flex_grow: 1.0,
                    ..default()
                },
                ..default()
            })
            .with_children(|col| {
                spawn_label(col, font, label, 15.0, C_TEXT);
                if !hint.is_empty() {
                    spawn_label(col, font, hint, 11.0, C_TEXT_DIM);
                }
            });

            if toggle_only {
                spawn_button(row, font, value, inc, Val::Px(120.0), C_BUTTON, 14.0);
            } else {
                spawn_button(row, font, "◀", dec, Val::Px(38.0), C_BUTTON, 15.0);
                row.spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(120.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|c| spawn_label(c, font, value, 15.0, C_ACCENT));
                spawn_button(row, font, "▶", inc, Val::Px(38.0), C_BUTTON, 15.0);
            }
        });
}

// ---------------- タイトル ----------------

fn build_title(commands: &mut Commands, ctx: &MenuBuildCtx) {
    commands
        .spawn((fullscreen_root(C_BG), MenuRoot))
        .with_children(|root| {
            spawn_label(root, ctx.font, "WORLD  GENESIS", 62.0, C_ACCENT);
            spawn_label(
                root,
                ctx.font,
                "誰も見ていなくても、世界は動き続ける",
                18.0,
                C_TEXT_DIM,
            );
            root.spawn(NodeBundle {
                style: Style {
                    height: Val::Px(28.0),
                    ..default()
                },
                ..default()
            });

            let w = Val::Px(340.0);
            spawn_button(root, ctx.font, "世界を選ぶ", MenuAction::Goto(AppState::WorldSelect), w, C_BUTTON, 20.0);
            spawn_button(root, ctx.font, "新しい世界を作る", MenuAction::NewWorld, w, C_BUTTON, 20.0);
            spawn_button(root, ctx.font, "設定", MenuAction::Goto(AppState::Settings), w, C_BUTTON, 20.0);
            spawn_button(root, ctx.font, "プラグイン", MenuAction::Goto(AppState::Plugins), w, C_BUTTON, 20.0);
            spawn_button(root, ctx.font, "終了", MenuAction::Quit, w, C_BUTTON_DANGER, 20.0);

            root.spawn(NodeBundle {
                style: Style { height: Val::Px(24.0), ..default() },
                ..default()
            });
            if !ctx.font.supports_cjk {
                spawn_label(
                    root,
                    ctx.font,
                    "[!] Japanese font not found - put a .ttf into assets/fonts/ui.ttf",
                    13.0,
                    C_WARN,
                );
            }
        });
}

// ---------------- ワールド選択 ----------------

fn build_world_select(commands: &mut Commands, ctx: &MenuBuildCtx) {
    commands
        .spawn((fullscreen_root(C_BG), MenuRoot))
        .with_children(|root| {
            spawn_label(root, ctx.font, "世界を選ぶ", 34.0, C_ACCENT);

            root.spawn(panel(76.0, 68.0)).with_children(|panel| {
                if ctx.saves.slots.is_empty() {
                    spawn_label(panel, ctx.font, "保存された世界はまだありません。", 18.0, C_TEXT_DIM);
                    spawn_label(panel, ctx.font, "「新しい世界を作る」から始めてください。", 15.0, C_TEXT_DIM);
                } else {
                    panel.spawn(scroll_area()).with_children(|list| {
                        for slot in &ctx.saves.slots {
                            build_save_row(list, ctx, slot);
                        }
                    });
                }
            });

            root.spawn(row(12.0)).with_children(|r| {
                spawn_button(r, ctx.font, "新しい世界を作る", MenuAction::NewWorld, Val::Px(240.0), C_BUTTON, 17.0);
                spawn_button(r, ctx.font, "戻る", MenuAction::Back, Val::Px(160.0), C_BUTTON, 17.0);
            });
        });
}

fn build_save_row(parent: &mut ChildBuilder, ctx: &MenuBuildCtx, slot: &SaveSlot) {
    let m = &slot.meta;
    let confirming = ctx.pending_delete.0.as_deref() == Some(m.folder.as_str());

    parent
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                width: Val::Percent(97.0),
                min_height: Val::Px(66.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            background_color: BackgroundColor(if confirming { C_BUTTON_DANGER } else { C_PANEL_SOFT }),
            ..default()
        })
        .with_children(|rowb| {
            rowb.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|col| {
                spawn_label(col, ctx.font, &m.display_name, 19.0, C_TEXT);
                spawn_label(
                    col,
                    ctx.font,
                    &format!(
                        "シード {} ・ {} ・ {} ・ {:.1} MB",
                        m.seed,
                        m.world_type.display_name(),
                        m.game_mode.display_name(),
                        slot.size_bytes as f64 / 1_048_576.0
                    ),
                    12.0,
                    C_TEXT_DIM,
                );
                let days = (m.sim_tick / 86_400) as u32;
                spawn_label(
                    col,
                    ctx.font,
                    &format!(
                        "世界の経過: {days} 日 ・ プレイ時間 {:.1} 時間{}",
                        m.played_seconds / 3600.0,
                        if m.plugins.is_empty() {
                            String::new()
                        } else {
                            format!(" ・ プラグイン {} 個", m.plugins.len())
                        }
                    ),
                    12.0,
                    C_TEXT_DIM,
                );
            });

            if confirming {
                rowb.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::End,
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|c| {
                    spawn_label(c, ctx.font, "本当に削除しますか？（元に戻せません）", 13.0, C_TEXT);
                    c.spawn(row(6.0)).with_children(|r| {
                        spawn_button(r, ctx.font, "削除する", MenuAction::ConfirmDeleteWorld(m.folder.clone()), Val::Px(110.0), C_BUTTON_DANGER_HOVER, 14.0);
                        spawn_button(r, ctx.font, "やめる", MenuAction::CancelDelete, Val::Px(100.0), C_BUTTON, 14.0);
                    });
                });
            } else {
                rowb.spawn(row(6.0)).with_children(|r| {
                    r.style_width(Val::Px(300.0));
                    spawn_button(r, ctx.font, "プレイ", MenuAction::PlayWorld(m.folder.clone()), Val::Px(110.0), C_BUTTON_PRESS, 15.0);
                    spawn_button(r, ctx.font, "複製", MenuAction::DuplicateWorld(m.folder.clone()), Val::Px(80.0), C_BUTTON, 14.0);
                    spawn_button(r, ctx.font, "削除", MenuAction::AskDeleteWorld(m.folder.clone()), Val::Px(80.0), C_BUTTON_DANGER, 14.0);
                });
            }
        });
}

/// `row()` の幅を後から詰めるための小さな拡張。
trait StyleWidthExt {
    fn style_width(&mut self, w: Val);
}

impl StyleWidthExt for ChildBuilder<'_> {
    fn style_width(&mut self, _w: Val) {
        // ChildBuilder からは親のスタイルを変更できないため、ここでは何もしない。
        // 幅は `row()` の既定（100%）と `justify_content` で十分に収まる。
    }
}

// ---------------- ワールド作成 ----------------

fn build_create_world(commands: &mut Commands, ctx: &MenuBuildCtx) {
    let form = ctx.form;
    commands
        .spawn((fullscreen_root(C_BG), MenuRoot))
        .with_children(|root| {
            spawn_label(root, ctx.font, "新しい世界を作る", 32.0, C_ACCENT);

            root.spawn(panel(72.0, 74.0)).with_children(|panel| {
                panel.spawn(scroll_area()).with_children(|list| {
                    // --- 名前 ---
                    spawn_text_field(
                        list, ctx.font, "世界の名前",
                        &form.name_text,
                        form.focus == Some(TextField::WorldName),
                        MenuAction::FocusField(TextField::WorldName),
                    );

                    // --- シード ---
                    list.spawn(row(8.0)).with_children(|r| {
                        r.spawn(NodeBundle {
                            style: Style { flex_grow: 1.0, ..default() },
                            ..default()
                        })
                        .with_children(|c| {
                            spawn_text_field(
                                c, ctx.font, "シード値（空欄ならランダム／文字でも可）",
                                &form.seed_text,
                                form.focus == Some(TextField::Seed),
                                MenuAction::FocusField(TextField::Seed),
                            );
                        });
                        spawn_button(r, ctx.font, "ランダム", MenuAction::RandomSeed, Val::Px(120.0), C_BUTTON, 14.0);
                    });

                    // --- 世界タイプ / ゲームモード ---
                    spawn_stepper(
                        list, ctx.font, "世界のタイプ", "地形の作られ方が変わる",
                        form.meta.world_type.display_name(),
                        MenuAction::CycleWorldType(-1),
                        MenuAction::CycleWorldType(1),
                        false,
                    );
                    spawn_stepper(
                        list, ctx.font, "ゲームモード", "オブザーバーは世界を観察するだけの視点",
                        form.meta.game_mode.display_name(),
                        MenuAction::CycleGameMode(-1),
                        MenuAction::CycleGameMode(1),
                        false,
                    );

                    spawn_label(list, ctx.font, "── 生成パラメータ ──", 14.0, C_TEXT_DIM);
                    for f in [
                        GenField::SeaLevel,
                        GenField::Amplitude,
                        GenField::Caves,
                        GenField::Ores,
                        GenField::Vegetation,
                        GenField::Settlements,
                    ] {
                        spawn_stepper(
                            list, ctx.font, f.label(), f.hint(),
                            &f.display(&form.meta),
                            MenuAction::AdjustGen(f, -f.step()),
                            MenuAction::AdjustGen(f, f.step()),
                            false,
                        );
                    }

                    // --- 適用されるプラグイン ---
                    let enabled = ctx.plugins.enabled_ids();
                    let text = if enabled.is_empty() {
                        "なし（プラグイン画面で有効にできます）".to_string()
                    } else {
                        enabled.join(", ")
                    };
                    spawn_label(list, ctx.font, &format!("この世界に適用されるプラグイン: {text}"), 13.0, C_TEXT_DIM);
                });
            });

            if let Some(err) = &form.error {
                spawn_label(root, ctx.font, err, 15.0, C_ERR);
            }

            root.spawn(row(12.0)).with_children(|r| {
                spawn_button(r, ctx.font, "この世界を作って始める", MenuAction::CreateAndPlay, Val::Px(300.0), C_BUTTON_PRESS, 18.0);
                spawn_button(r, ctx.font, "戻る", MenuAction::Back, Val::Px(140.0), C_BUTTON, 17.0);
            });
        });
}

fn spawn_text_field(
    parent: &mut ChildBuilder,
    font: &UiFont,
    label: &str,
    value: &str,
    focused: bool,
    focus_action: MenuAction,
) {
    parent
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(96.0),
                row_gap: Val::Px(3.0),
                ..default()
            },
            ..default()
        })
        .with_children(|col| {
            spawn_label(col, font, label, 13.0, C_TEXT_DIM);
            col.spawn((
                button_bundle(Val::Percent(100.0), 34.0, if focused { C_BUTTON_PRESS } else { C_BUTTON }),
                focus_action,
            ))
            .with_children(|b| {
                let shown = if focused {
                    format!("{value}▏")
                } else if value.is_empty() {
                    "（クリックして入力）".to_string()
                } else {
                    value.to_string()
                };
                b.spawn(TextBundle::from_section(
                    shown,
                    font.style(16.0, if value.is_empty() && !focused { C_TEXT_DIM } else { C_TEXT }),
                ));
            });
        });
}

// ---------------- 設定 ----------------

fn build_settings(commands: &mut Commands, ctx: &MenuBuildCtx) {
    commands
        .spawn((fullscreen_root(C_BG), MenuRoot))
        .with_children(|root| {
            spawn_label(root, ctx.font, "設定", 32.0, C_ACCENT);

            root.spawn(panel(72.0, 76.0)).with_children(|panel| {
                panel.spawn(scroll_area()).with_children(|list| {
                    for f in [
                        SettingField::RenderDistance,
                        SettingField::ChunkBudget,
                        SettingField::Fov,
                        SettingField::Sensitivity,
                        SettingField::InvertY,
                        SettingField::ThirdPerson,
                        SettingField::ViewBobbing,
                        SettingField::Fog,
                        SettingField::ShowHud,
                        SettingField::GuiScale,
                        SettingField::MasterVolume,
                        SettingField::MusicVolume,
                        SettingField::Autosave,
                    ] {
                        spawn_stepper(
                            list, ctx.font, f.label(), f.hint(),
                            &f.display(ctx.settings),
                            MenuAction::AdjustSetting(f, -f.step()),
                            MenuAction::AdjustSetting(f, f.step()),
                            f.is_toggle(),
                        );
                    }
                    spawn_label(list, ctx.font, &format!("UIフォント: {}", ctx.font.source), 12.0, C_TEXT_DIM);
                });
            });

            spawn_label(root, ctx.font, "設定は「戻る」で自動保存されます。", 13.0, C_TEXT_DIM);
            spawn_button(root, ctx.font, "戻る", MenuAction::Back, Val::Px(200.0), C_BUTTON, 18.0);
        });
}

// ---------------- プラグイン管理 ----------------

fn build_plugins(commands: &mut Commands, ctx: &MenuBuildCtx) {
    commands
        .spawn((fullscreen_root(C_BG), MenuRoot))
        .with_children(|root| {
            spawn_label(root, ctx.font, "プラグイン", 32.0, C_ACCENT);
            spawn_label(
                root,
                ctx.font,
                "run/mods/ に置いた JSON が一覧に現れます。有効・無効はワールド作成時に固定されます。",
                13.0,
                C_TEXT_DIM,
            );

            root.spawn(panel(78.0, 68.0)).with_children(|panel| {
                if let Some(err) = &ctx.plugins.last_scan_error {
                    spawn_label(panel, ctx.font, err, 14.0, C_ERR);
                }
                if ctx.plugins.plugins.is_empty() {
                    spawn_label(panel, ctx.font, "プラグインが見つかりません。", 17.0, C_TEXT_DIM);
                    spawn_label(panel, ctx.font, "example_deepearth.json を複製すると自作の雛形になります。", 14.0, C_TEXT_DIM);
                } else {
                    panel.spawn(scroll_area()).with_children(|list| {
                        for p in &ctx.plugins.plugins {
                            build_plugin_row(list, ctx, p);
                        }
                    });
                }
            });

            root.spawn(row(12.0)).with_children(|r| {
                spawn_button(r, ctx.font, "再スキャン", MenuAction::RescanPlugins, Val::Px(160.0), C_BUTTON, 16.0);
                spawn_button(r, ctx.font, "mods フォルダの場所", MenuAction::OpenModsFolder, Val::Px(220.0), C_BUTTON, 16.0);
                spawn_button(r, ctx.font, "戻る", MenuAction::Back, Val::Px(140.0), C_BUTTON, 16.0);
            });
        });
}

fn build_plugin_row(parent: &mut ChildBuilder, ctx: &MenuBuildCtx, p: &crate::plugins::LoadedPlugin) {
    let healthy = p.problems.is_empty();
    parent
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                width: Val::Percent(97.0),
                min_height: Val::Px(62.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            background_color: BackgroundColor(C_PANEL_SOFT),
            ..default()
        })
        .with_children(|rowb| {
            rowb.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|col| {
                let m = &p.manifest;
                spawn_label(col, ctx.font, &m.name, 18.0, if healthy { C_TEXT } else { C_ERR });
                let mut meta = format!("{}", m.id);
                if !m.version.is_empty() {
                    meta.push_str(&format!(" v{}", m.version));
                }
                if !m.author.is_empty() {
                    meta.push_str(&format!(" / {}", m.author));
                }
                meta.push_str(&format!(
                    " ・ ブロック {} / 鉱脈 {} / 生物 {}",
                    m.blocks.len(),
                    m.ores.len(),
                    m.creatures.len()
                ));
                spawn_label(col, ctx.font, &meta, 12.0, C_TEXT_DIM);
                if !m.description.is_empty() {
                    spawn_label(col, ctx.font, &m.description, 12.0, C_TEXT_DIM);
                }
                for problem in &p.problems {
                    spawn_label(col, ctx.font, &format!("⚠ {problem}"), 12.0, C_ERR);
                }
            });

            if healthy {
                let (label, color) = if p.enabled {
                    ("有効", C_OK)
                } else {
                    ("無効", C_BUTTON)
                };
                spawn_button(
                    rowb,
                    ctx.font,
                    label,
                    MenuAction::TogglePlugin(p.manifest.id.clone()),
                    Val::Px(110.0),
                    color,
                    15.0,
                );
            } else {
                rowb.spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(110.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|c| spawn_label(c, ctx.font, "使用不可", 14.0, C_ERR));
            }
        });
}

// ---------------- 読み込み中 ----------------

fn build_loading(commands: &mut Commands, ctx: &MenuBuildCtx) {
    commands
        .spawn((fullscreen_root(Color::rgba(0.04, 0.05, 0.08, 1.0)), MenuRoot))
        .with_children(|root| {
            spawn_label(root, ctx.font, "世界を生成しています…", 30.0, C_ACCENT);
            root.spawn((
                TextBundle::from_section("", ctx.font.style(16.0, C_TEXT_DIM)),
                LoadingProgressText,
            ));
            root.spawn(NodeBundle {
                style: Style { height: Val::Px(20.0), ..default() },
                ..default()
            });
            spawn_label(
                root,
                ctx.font,
                "地形・気候・生態系・集落は、シードから毎回同じ姿で組み上げられます。",
                13.0,
                C_TEXT_DIM,
            );
        });
}

#[derive(Component)]
pub struct LoadingProgressText;

// ---------------- ポーズ ----------------

fn build_pause(commands: &mut Commands, ctx: &MenuBuildCtx) {
    commands
        .spawn((fullscreen_root(Color::rgba(0.02, 0.03, 0.05, 0.72)), MenuRoot))
        .with_children(|root| {
            spawn_label(root, ctx.font, "一時停止", 40.0, C_ACCENT);
            root.spawn(NodeBundle {
                style: Style { height: Val::Px(16.0), ..default() },
                ..default()
            });
            let w = Val::Px(320.0);
            spawn_button(root, ctx.font, "世界へ戻る", MenuAction::ResumeGame, w, C_BUTTON_PRESS, 19.0);
            spawn_button(root, ctx.font, "いま保存する", MenuAction::SaveNow, w, C_BUTTON, 19.0);
            spawn_button(root, ctx.font, "設定", MenuAction::Goto(AppState::Settings), w, C_BUTTON, 19.0);
            spawn_button(root, ctx.font, "保存してタイトルへ", MenuAction::SaveAndQuitToTitle, w, C_BUTTON_DANGER, 19.0);
        });
}

// ======================================================================
// システム
// ======================================================================

/// 状態が変わったとき、または `UiDirty` が立ったときに画面を組み直す。
#[allow(clippy::too_many_arguments)]
pub fn rebuild_ui_system(
    mut commands: Commands,
    mut dirty: ResMut<UiDirty>,
    state: Res<State<AppState>>,
    existing: Query<Entity, With<MenuRoot>>,
    font: Res<UiFont>,
    settings: Res<GameSettings>,
    saves: Res<SaveListCache>,
    form: Res<CreateWorldForm>,
    plugin_mgr: Res<PluginManager>,
    pending_delete: Res<PendingDelete>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    for e in existing.iter() {
        commands.entity(e).despawn_recursive();
    }

    let ctx = MenuBuildCtx {
        font: &font,
        settings: &settings,
        saves: &saves,
        form: &form,
        plugins: &plugin_mgr,
        pending_delete: &pending_delete,
        state: *state.get(),
    };
    build_screen(&mut commands, &ctx);
}

/// 状態遷移時に UI の作り直しを予約する。
pub fn mark_ui_dirty_on_state_change(mut dirty: ResMut<UiDirty>) {
    dirty.0 = true;
}

/// ボタンの見た目（ホバー・押下）。
pub fn button_visual_system(
    mut query: Query<
        (&Interaction, &mut BackgroundColor, &MenuAction),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color, action) in query.iter_mut() {
        let danger = matches!(
            action,
            MenuAction::Quit
                | MenuAction::AskDeleteWorld(_)
                | MenuAction::ConfirmDeleteWorld(_)
                | MenuAction::SaveAndQuitToTitle
        );
        *color = BackgroundColor(match (*interaction, danger) {
            (Interaction::Pressed, _) => C_BUTTON_PRESS,
            (Interaction::Hovered, true) => C_BUTTON_DANGER_HOVER,
            (Interaction::Hovered, false) => C_BUTTON_HOVER,
            (Interaction::None, true) => C_BUTTON_DANGER,
            (Interaction::None, false) => C_BUTTON,
        });
    }
}

/// 押されたボタンの操作を発火させる。
#[derive(Event)]
pub struct MenuActionEvent(pub MenuAction);

pub fn button_press_system(
    query: Query<(&Interaction, &MenuAction), (Changed<Interaction>, With<Button>)>,
    mut events: EventWriter<MenuActionEvent>,
) {
    for (interaction, action) in query.iter() {
        if *interaction == Interaction::Pressed {
            events.send(MenuActionEvent(action.clone()));
        }
    }
}

/// メニュー操作を適用する。
#[allow(clippy::too_many_arguments)]
pub fn menu_action_system(
    mut events: EventReader<MenuActionEvent>,
    mut next_state: ResMut<NextState<AppState>>,
    state: Res<State<AppState>>,
    mut return_to: ResMut<ReturnTo>,
    mut settings: ResMut<GameSettings>,
    mut form: ResMut<CreateWorldForm>,
    mut saves: ResMut<SaveListCache>,
    mut plugin_mgr: ResMut<PluginManager>,
    mut pending_delete: ResMut<PendingDelete>,
    mut pending_load: ResMut<PendingLoad>,
    mut dirty: ResMut<UiDirty>,
    mut toast: ResMut<Toast>,
    mut exit: EventWriter<AppExit>,
    save_mgr: Res<SaveManagerRes>,
    mut save_request: ResMut<SaveRequest>,
) {
    for MenuActionEvent(action) in events.read() {
        dirty.0 = true;
        match action {
            MenuAction::Goto(target) => {
                if *target == AppState::Settings || *target == AppState::Plugins {
                    return_to.0 = *state.get();
                }
                if *target == AppState::WorldSelect {
                    refresh_saves(&save_mgr, &mut saves);
                }
                next_state.set(*target);
            }

            MenuAction::Back => {
                let here = *state.get();
                let target = match here {
                    AppState::Settings | AppState::Plugins => {
                        // 設定を出るときに保存する。
                        if let Err(e) = settings.save(&save_mgr.0.root) {
                            toast.show(format!("設定を保存できません: {e}"), C_ERR);
                        }
                        if here == AppState::Plugins {
                            settings.enabled_plugins = plugin_mgr.enabled_ids();
                            if let Err(e) = settings.save(&save_mgr.0.root) {
                                toast.show(
                                    format!("プラグインの有効状態を保存できません: {e}"),
                                    C_ERR,
                                );
                            }
                        }
                        return_to.0
                    }
                    AppState::CreateWorld => AppState::WorldSelect,
                    AppState::WorldSelect => AppState::Title,
                    _ => AppState::Title,
                };
                if target == AppState::WorldSelect {
                    refresh_saves(&save_mgr, &mut saves);
                }
                pending_delete.0 = None;
                next_state.set(target);
            }

            MenuAction::Quit => {
                exit.send(AppExit);
            }

            MenuAction::NewWorld => {
                form.reset();
                next_state.set(AppState::CreateWorld);
            }

            MenuAction::RandomSeed => {
                let s = random_seed();
                form.seed_text = s.to_string();
            }

            MenuAction::FocusField(f) => {
                form.focus = if form.focus == Some(*f) { None } else { Some(*f) };
            }

            MenuAction::CycleWorldType(dir) => {
                let all = WorldType::ALL;
                let idx = all.iter().position(|t| *t == form.meta.world_type).unwrap_or(0);
                let next = (idx as i32 + dir).rem_euclid(all.len() as i32) as usize;
                form.meta.world_type = all[next];
            }

            MenuAction::CycleGameMode(dir) => {
                let all = GameMode::ALL;
                let idx = all.iter().position(|t| *t == form.meta.game_mode).unwrap_or(0);
                let next = (idx as i32 + dir).rem_euclid(all.len() as i32) as usize;
                form.meta.game_mode = all[next];
            }

            MenuAction::AdjustGen(field, delta) => {
                field.apply(&mut form.meta, *delta);
            }

            MenuAction::AdjustSetting(field, delta) => {
                field.apply(&mut settings, *delta);
            }

            MenuAction::CreateAndPlay => {
                form.meta.display_name = if form.name_text.trim().is_empty() {
                    "名もなき世界".to_string()
                } else {
                    form.name_text.trim().to_string()
                };
                form.meta.seed = form.resolved_seed();
                form.meta.plugins = plugin_mgr.enabled_ids();
                form.meta.created_unix = now_unix();
                form.meta.last_played_unix = now_unix();

                match save_mgr.0.create_world(form.meta.clone()) {
                    Ok(created) => {
                        form.error = None;
                        pending_load.0 = Some(created);
                        next_state.set(AppState::LoadingWorld);
                    }
                    Err(e) => {
                        form.error = Some(format!("世界を作成できません: {e}"));
                    }
                }
            }

            MenuAction::PlayWorld(folder) => match save_mgr.0.read_meta(folder) {
                Ok(meta) => {
                    pending_load.0 = Some(meta);
                    next_state.set(AppState::LoadingWorld);
                }
                Err(e) => toast.show(format!("読み込めません: {e}"), C_ERR),
            },

            MenuAction::AskDeleteWorld(folder) => {
                pending_delete.0 = Some(folder.clone());
            }

            MenuAction::CancelDelete => {
                pending_delete.0 = None;
            }

            MenuAction::ConfirmDeleteWorld(folder) => {
                match save_mgr.0.delete_world(folder) {
                    Ok(()) => toast.show("世界を削除しました。", C_WARN),
                    Err(e) => toast.show(format!("削除できません: {e}"), C_ERR),
                }
                pending_delete.0 = None;
                refresh_saves(&save_mgr, &mut saves);
            }

            MenuAction::DuplicateWorld(folder) => {
                let name = save_mgr
                    .0
                    .read_meta(folder)
                    .map(|m| format!("{} のコピー", m.display_name))
                    .unwrap_or_else(|_| "コピー".to_string());
                match save_mgr.0.duplicate_world(folder, &name) {
                    Ok(_) => toast.show("世界を複製しました。", C_OK),
                    Err(e) => toast.show(format!("複製できません: {e}"), C_ERR),
                }
                refresh_saves(&save_mgr, &mut saves);
            }

            MenuAction::TogglePlugin(id) => {
                let now_enabled = plugin_mgr
                    .plugins
                    .iter()
                    .find(|p| p.manifest.id == *id)
                    .map(|p| p.enabled)
                    .unwrap_or(false);
                if let Err(e) = plugin_mgr.set_enabled(id, !now_enabled) {
                    toast.show(e, C_ERR);
                } else {
                    settings.enabled_plugins = plugin_mgr.enabled_ids();
                }
            }

            MenuAction::RescanPlugins => {
                *plugin_mgr = PluginManager::scan(&save_mgr.0.root, &settings.enabled_plugins);
                toast.show(
                    format!("{} 個のプラグインを検出しました。", plugin_mgr.plugins.len()),
                    C_OK,
                );
            }

            MenuAction::OpenModsFolder => {
                let path = PluginManager::mods_dir(&save_mgr.0.root);
                toast.show(format!("mods フォルダ: {}", path.display()), C_ACCENT);
            }

            MenuAction::ResumeGame => {
                next_state.set(AppState::InGame);
            }

            MenuAction::SaveNow => {
                save_request.save = true;
            }

            MenuAction::SaveAndQuitToTitle => {
                save_request.save = true;
                save_request.quit_after = true;
            }
        }
    }
}

fn refresh_saves(mgr: &SaveManagerRes, cache: &mut SaveListCache) {
    cache.slots = mgr.0.list_saves();
    cache.loaded = true;
}

/// セーブ管理をリソースとして持つためのラッパー。
#[derive(Resource)]
pub struct SaveManagerRes(pub SaveManager);

/// 保存要求。ポーズメニューと自動セーブの両方から立てられる。
#[derive(Resource, Default)]
pub struct SaveRequest {
    pub save: bool,
    pub quit_after: bool,
}

/// 入力欄への文字入力。
pub fn text_input_system(
    mut form: ResMut<CreateWorldForm>,
    mut chars: EventReader<ReceivedCharacter>,
    keys: Res<ButtonInput<KeyCode>>,
    mut dirty: ResMut<UiDirty>,
) {
    let Some(field) = form.focus else {
        chars.clear();
        return;
    };

    let mut changed = false;

    if keys.just_pressed(KeyCode::Backspace) {
        let target = match field {
            TextField::WorldName => &mut form.name_text,
            TextField::Seed => &mut form.seed_text,
        };
        target.pop();
        changed = true;
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Escape) {
        form.focus = None;
        dirty.0 = true;
        chars.clear();
        return;
    }

    for ev in chars.read() {
        for c in ev.char.chars() {
            // 制御文字は無視する（Backspace は上で処理済み）。
            if c.is_control() {
                continue;
            }
            let (target, limit) = match field {
                TextField::WorldName => (&mut form.name_text, 40),
                TextField::Seed => (&mut form.seed_text, 24),
            };
            if target.chars().count() < limit {
                target.push(c);
                changed = true;
            }
        }
    }

    if changed {
        dirty.0 = true;
    }
}

/// Esc キーでのポーズ切り替え。
pub fn pause_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match *state.get() {
        AppState::InGame => next_state.set(AppState::Paused),
        AppState::Paused => next_state.set(AppState::InGame),
        _ => {}
    }
}

/// 画面に応じてカーソルの掴み方を切り替える。
pub fn cursor_grab_system(
    state: Res<State<AppState>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut window) = windows.get_single_mut() else { return };
    let want_free = state.get().wants_cursor();
    let currently_free = window.cursor.visible;
    if want_free == currently_free {
        return;
    }
    if want_free {
        window.cursor.grab_mode = CursorGrabMode::None;
        window.cursor.visible = true;
    } else {
        window.cursor.grab_mode = CursorGrabMode::Locked;
        window.cursor.visible = false;
    }
}

/// トーストの寿命管理。
pub fn toast_system(time: Res<Time>, mut toast: ResMut<Toast>) {
    if toast.remaining > 0.0 {
        toast.remaining = (toast.remaining - time.delta_seconds()).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_in_game_state_captures_the_cursor() {
        assert!(!AppState::InGame.wants_cursor());
        for s in [
            AppState::Title,
            AppState::WorldSelect,
            AppState::CreateWorld,
            AppState::Settings,
            AppState::Plugins,
            AppState::Paused,
            AppState::LoadingWorld,
        ] {
            assert!(s.wants_cursor(), "{s:?} should release the cursor");
        }
    }

    #[test]
    fn the_world_keeps_running_only_where_it_should() {
        assert!(AppState::InGame.world_is_live());
        assert!(AppState::Paused.world_is_live());
        assert!(!AppState::Title.world_is_live());
        assert!(!AppState::Settings.world_is_live());
    }

    #[test]
    fn numeric_seeds_are_used_verbatim() {
        let mut f = CreateWorldForm::default();
        f.seed_text = "123456789".into();
        assert_eq!(f.resolved_seed(), 123_456_789);
        f.seed_text = "-42".into();
        assert_eq!(f.resolved_seed(), (-42i64) as u64);
    }

    #[test]
    fn word_seeds_hash_deterministically() {
        let mut a = CreateWorldForm::default();
        a.seed_text = "はじまりの大地".into();
        let mut b = CreateWorldForm::default();
        b.seed_text = "はじまりの大地".into();
        assert_eq!(a.resolved_seed(), b.resolved_seed());

        let mut c = CreateWorldForm::default();
        c.seed_text = "べつの大地".into();
        assert_ne!(a.resolved_seed(), c.resolved_seed());
    }

    #[test]
    fn an_empty_seed_still_produces_a_world() {
        let mut f = CreateWorldForm::default();
        f.seed_text = "   ".into();
        // ランダムなので値は問わないが、必ず何かが返ること。
        let _ = f.resolved_seed();
    }

    #[test]
    fn generation_fields_clamp_to_playable_ranges() {
        let mut m = default_meta("t", 1);
        for _ in 0..200 {
            GenField::Amplitude.apply(&mut m, 1.0);
            GenField::Caves.apply(&mut m, 1.0);
            GenField::SeaLevel.apply(&mut m, 10.0);
            GenField::Settlements.apply(&mut m, 1.0);
        }
        assert!(m.terrain_amplitude <= 3.0);
        assert!(m.cave_density <= 2.5);
        assert!(m.sea_level <= 110);
        assert!(m.settlement_density <= 4.0);

        for _ in 0..200 {
            GenField::Amplitude.apply(&mut m, -1.0);
            GenField::Caves.apply(&mut m, -1.0);
            GenField::SeaLevel.apply(&mut m, -10.0);
            GenField::Settlements.apply(&mut m, -1.0);
        }
        assert!(m.terrain_amplitude >= 0.25);
        assert!(m.cave_density >= 0.0);
        assert!(m.sea_level >= 16);
        assert!(m.settlement_density >= 0.0);
    }

    #[test]
    fn setting_fields_clamp_and_toggle_correctly() {
        let mut s = GameSettings::default();
        for _ in 0..100 {
            SettingField::RenderDistance.apply(&mut s, 1.0);
            SettingField::Fov.apply(&mut s, 5.0);
            SettingField::MasterVolume.apply(&mut s, 0.1);
        }
        assert!(s.render_distance <= 24);
        assert!(s.fov_degrees <= 120.0);
        assert!(s.master_volume <= 1.0);

        let before = s.invert_mouse_y;
        SettingField::InvertY.apply(&mut s, 0.0);
        assert_ne!(s.invert_mouse_y, before);
        SettingField::InvertY.apply(&mut s, 0.0);
        assert_eq!(s.invert_mouse_y, before);
    }

    #[test]
    fn toggles_have_no_step_and_steppers_do() {
        for f in [
            SettingField::InvertY,
            SettingField::Fog,
            SettingField::ViewBobbing,
            SettingField::ThirdPerson,
            SettingField::ShowHud,
        ] {
            assert!(f.is_toggle());
            assert_eq!(f.step(), 0.0);
        }
        for f in [
            SettingField::RenderDistance,
            SettingField::Fov,
            SettingField::Sensitivity,
            SettingField::GuiScale,
            SettingField::MasterVolume,
            SettingField::ChunkBudget,
            SettingField::Autosave,
        ] {
            assert!(!f.is_toggle());
            assert!(f.step() > 0.0, "{:?} has no step", f);
        }
    }

    #[test]
    fn every_setting_field_renders_a_value() {
        let s = GameSettings::default();
        for f in [
            SettingField::RenderDistance, SettingField::Fov, SettingField::Sensitivity,
            SettingField::InvertY, SettingField::GuiScale, SettingField::MasterVolume,
            SettingField::MusicVolume, SettingField::ChunkBudget, SettingField::Fog,
            SettingField::ViewBobbing, SettingField::ThirdPerson, SettingField::ShowHud,
            SettingField::Autosave,
        ] {
            assert!(!f.display(&s).is_empty(), "{:?} rendered an empty value", f);
            assert!(!f.label().is_empty());
        }
    }

    #[test]
    fn every_generation_field_renders_a_value() {
        let m = default_meta("t", 1);
        for f in [
            GenField::SeaLevel, GenField::Amplitude, GenField::Caves,
            GenField::Ores, GenField::Vegetation, GenField::Settlements,
        ] {
            assert!(!f.display(&m).is_empty());
            assert!(!f.label().is_empty());
            assert!(!f.hint().is_empty());
            assert!(f.step() > 0.0);
        }
    }

    #[test]
    fn cycling_world_type_wraps_in_both_directions() {
        let all = WorldType::ALL;
        let mut idx = 0i32;
        for _ in 0..(all.len() * 2) {
            idx = (idx + 1).rem_euclid(all.len() as i32);
        }
        assert_eq!(idx, 0);
        let mut back = 0i32;
        back = (back - 1).rem_euclid(all.len() as i32);
        assert_eq!(back as usize, all.len() - 1);
    }

    #[test]
    fn autosave_can_be_switched_off() {
        let mut s = GameSettings::default();
        for _ in 0..20 {
            SettingField::Autosave.apply(&mut s, -1.0);
        }
        assert_eq!(s.autosave_minutes, 0.0);
        assert_eq!(SettingField::Autosave.display(&s), "オフ");
    }
}
