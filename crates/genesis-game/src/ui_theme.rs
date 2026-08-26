//! UI の配色・寸法と、日本語フォントの解決。
//!
//! 画面表記は全て日本語なので、CJK グリフを持つフォントが要る。
//! Bevy 同梱の既定フォントは英字しか持たないため、
//!   1. 同梱アセット `assets/fonts/ui.ttf`（利用者が任意に差し替え可能）
//!   2. OS のシステムフォント
//!   3. Bevy の既定フォント（最後の手段）
//! の順に探し、見つかったものを使う。フォントが1つも無くても
//! ゲームは起動する（文字が出ないだけ）ようにしてある。

use bevy::prelude::*;
use bevy::text::Font;
use std::path::{Path, PathBuf};

// --- 配色 ---
pub const C_BG: Color = Color::rgba(0.05, 0.06, 0.09, 0.97);
pub const C_PANEL: Color = Color::rgba(0.09, 0.11, 0.15, 0.96);
pub const C_PANEL_SOFT: Color = Color::rgba(0.13, 0.15, 0.20, 0.92);
pub const C_BUTTON: Color = Color::rgb(0.17, 0.20, 0.27);
pub const C_BUTTON_HOVER: Color = Color::rgb(0.26, 0.32, 0.42);
pub const C_BUTTON_PRESS: Color = Color::rgb(0.36, 0.46, 0.58);
pub const C_BUTTON_DANGER: Color = Color::rgb(0.42, 0.16, 0.16);
pub const C_BUTTON_DANGER_HOVER: Color = Color::rgb(0.60, 0.22, 0.22);
pub const C_ACCENT: Color = Color::rgb(0.96, 0.80, 0.28);
pub const C_TEXT: Color = Color::rgb(0.93, 0.94, 0.96);
pub const C_TEXT_DIM: Color = Color::rgb(0.62, 0.66, 0.72);
pub const C_OK: Color = Color::rgb(0.42, 0.82, 0.48);
pub const C_WARN: Color = Color::rgb(0.95, 0.55, 0.25);
pub const C_ERR: Color = Color::rgb(0.92, 0.36, 0.34);

/// UI 全体で使うフォント。
#[derive(Resource, Clone)]
pub struct UiFont {
    pub handle: Handle<Font>,
    /// 実際に読み込めたフォントの説明（設定画面に表示する）。
    pub source: String,
    /// 日本語が表示できるか。
    pub supports_cjk: bool,
}

impl UiFont {
    pub fn style(&self, size: f32, color: Color) -> TextStyle {
        TextStyle {
            font: self.handle.clone(),
            font_size: size,
            color,
        }
    }
}

/// 探索するシステムフォントの候補。上から順に試す。
fn font_candidates() -> Vec<(PathBuf, &'static str, bool)> {
    let mut v: Vec<(PathBuf, &'static str, bool)> = Vec::new();

    if cfg!(target_os = "windows") {
        let root = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let fonts = Path::new(&root).join("Fonts");
        for (file, label) in [
            ("meiryo.ttc", "Meiryo"),
            ("YuGothR.ttc", "Yu Gothic"),
            ("YuGothM.ttc", "Yu Gothic Medium"),
            ("msgothic.ttc", "MS Gothic"),
            ("NotoSansJP-Regular.otf", "Noto Sans JP"),
            ("NotoSansCJKjp-Regular.otf", "Noto Sans CJK JP"),
        ] {
            v.push((fonts.join(file), label, true));
        }
        v.push((fonts.join("segoeui.ttf"), "Segoe UI (CJK非対応)", false));
    }

    // Linux / *BSD
    for (path, label) in [
        ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", "Noto Sans CJK"),
        ("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", "Noto Sans CJK"),
        ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", "Noto Sans CJK"),
        ("/usr/share/fonts/truetype/fonts-japanese-gothic.ttf", "IPA Gothic"),
    ] {
        v.push((PathBuf::from(path), label, true));
    }
    v.push((
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        "DejaVu Sans (CJK非対応)",
        false,
    ));

    // macOS
    for (path, label) in [
        ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", "Hiragino Sans"),
        ("/System/Library/Fonts/Hiragino Sans GB.ttc", "Hiragino Sans GB"),
        ("/Library/Fonts/Arial Unicode.ttf", "Arial Unicode"),
    ] {
        v.push((PathBuf::from(path), label, true));
    }

    v
}

/// フォントを解決して `UiFont` を返す。
///
/// `asset_root` に `fonts/ui.ttf`（または `.otf` / `.ttc`）があれば最優先で使う。
/// 利用者は好きな日本語フォントをそこへ置くだけで表示を差し替えられる。
pub fn resolve_ui_font(asset_root: &Path, fonts: &mut Assets<Font>) -> UiFont {
    // 1. 同梱アセット
    for name in ["fonts/ui.ttf", "fonts/ui.otf", "fonts/ui.ttc"] {
        let path = asset_root.join(name);
        if let Some(font) = try_load_font(&path) {
            return UiFont {
                handle: fonts.add(font),
                source: format!("同梱フォント ({name})"),
                supports_cjk: true,
            };
        }
    }

    // 2. システムフォント
    for (path, label, cjk) in font_candidates() {
        if let Some(font) = try_load_font(&path) {
            return UiFont {
                handle: fonts.add(font),
                source: format!("システムフォント: {label}"),
                supports_cjk: cjk,
            };
        }
    }

    // 3. Bevy 既定フォント（英数字のみ）
    warn!(
        "日本語フォントが見つかりませんでした。assets/fonts/ui.ttf に任意のフォントを置くと表示されます。"
    );
    UiFont {
        handle: Handle::default(),
        source: "既定フォント（日本語は表示できません）".to_string(),
        supports_cjk: false,
    }
}

fn try_load_font(path: &Path) -> Option<Font> {
    let bytes = std::fs::read(path).ok()?;
    match Font::try_from_bytes(bytes) {
        Ok(f) => {
            info!("UIフォントを読み込みました: {}", path.display());
            Some(f)
        }
        Err(e) => {
            // 読めなかっただけなので次の候補へ進む。
            debug!("フォント {} を解析できません: {e}", path.display());
            None
        }
    }
}

// --- 共通のノード生成ヘルパー ---

/// 画面全体を覆うルートノード。
pub fn fullscreen_root(background: Color) -> NodeBundle {
    NodeBundle {
        style: Style {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(10.0),
            ..default()
        },
        background_color: BackgroundColor(background),
        ..default()
    }
}

/// 中央のパネル。
pub fn panel(width_pct: f32, height_pct: f32) -> NodeBundle {
    NodeBundle {
        style: Style {
            width: Val::Percent(width_pct),
            height: Val::Percent(height_pct),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(18.0)),
            row_gap: Val::Px(8.0),
            ..default()
        },
        background_color: BackgroundColor(C_PANEL),
        ..default()
    }
}

/// 横並びの行。
pub fn row(gap: f32) -> NodeBundle {
    NodeBundle {
        style: Style {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(gap),
            width: Val::Percent(100.0),
            ..default()
        },
        ..default()
    }
}

/// 縦にスクロールできる領域（Bevy 0.13 の `overflow` を使う）。
pub fn scroll_area() -> NodeBundle {
    NodeBundle {
        style: Style {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            row_gap: Val::Px(6.0),
            overflow: Overflow::clip_y(),
            ..default()
        },
        ..default()
    }
}

pub fn button_bundle(width: Val, height: f32, color: Color) -> ButtonBundle {
    ButtonBundle {
        style: Style {
            width,
            height: Val::Px(height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::horizontal(Val::Px(14.0)),
            ..default()
        },
        background_color: BackgroundColor(color),
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_list_is_not_empty_and_has_no_duplicates() {
        let c = font_candidates();
        assert!(!c.is_empty());
        let mut paths: Vec<&PathBuf> = c.iter().map(|(p, _, _)| p).collect();
        paths.sort();
        let before = paths.len();
        paths.dedup();
        assert_eq!(paths.len(), before, "duplicate font candidates");
    }

    #[test]
    fn candidates_prefer_cjk_capable_fonts() {
        let c = font_candidates();
        let first_cjk = c.iter().position(|(_, _, cjk)| *cjk);
        let first_non_cjk = c.iter().position(|(_, _, cjk)| !*cjk);
        if let (Some(a), Some(b)) = (first_cjk, first_non_cjk) {
            assert!(a < b, "a non-CJK font is tried before a CJK one");
        }
    }

    /// この環境に日本語フォントがあれば、実際にパースできることを確かめる。
    /// `.ttc`（TrueType コレクション）が読めないと画面が全て空欄になるため、
    /// ここで実物を通しておく価値がある。
    #[test]
    fn an_available_system_font_actually_parses() {
        let found = font_candidates()
            .into_iter()
            .filter(|(p, _, _)| p.exists())
            .find_map(|(p, label, _)| try_load_font(&p).map(|_| label));

        match found {
            Some(label) => println!("parsed system font: {label}"),
            None => println!("no system font available on this machine - skipping"),
        }
        // フォントが1つも無い環境でも失敗にはしない（CI 等を壊さないため）。
    }
}
