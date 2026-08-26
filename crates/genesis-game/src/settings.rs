//! ゲーム設定の永続化。
//!
//! `run/config/settings.json` に保存され、起動時に読み込まれる。
//! 壊れたファイルや未知のフィールドがあっても既定値へフォールバックし、
//! 設定が原因でゲームが起動できなくなることがないようにしてある。

use crate::keybinds::KeyBindings;
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn d_render_distance() -> i32 { 8 }
fn d_fov() -> f32 { 75.0 }
fn d_sensitivity() -> f32 { 0.0028 }
fn d_gui_scale() -> f32 { 1.0 }
fn d_master_volume() -> f32 { 0.8 }
fn d_music_volume() -> f32 { 0.5 }
fn d_chunk_budget() -> u32 { 4 }
fn d_true() -> bool { true }
fn d_view_bobbing() -> bool { true }
fn d_third_person() -> bool { true }
fn d_autosave_minutes() -> f32 { 5.0 }

#[derive(Debug, Clone, Serialize, Deserialize, Resource)]
#[serde(default)]
pub struct GameSettings {
    /// 描画チャンク半径。性能へ最も強く効く設定。
    #[serde(default = "d_render_distance")]
    pub render_distance: i32,
    #[serde(default = "d_fov")]
    pub fov_degrees: f32,
    #[serde(default = "d_sensitivity")]
    pub mouse_sensitivity: f32,
    #[serde(default = "d_true")]
    pub invert_mouse_y: bool,
    #[serde(default = "d_gui_scale")]
    pub gui_scale: f32,
    #[serde(default = "d_master_volume")]
    pub master_volume: f32,
    #[serde(default = "d_music_volume")]
    pub music_volume: f32,
    /// 1フレームあたりに適用するチャンクメッシュ数の上限。
    #[serde(default = "d_chunk_budget")]
    pub chunk_upload_budget: u32,
    #[serde(default = "d_true")]
    pub smooth_lighting: bool,
    #[serde(default = "d_true")]
    pub show_fog: bool,
    #[serde(default = "d_view_bobbing")]
    pub view_bobbing: bool,
    #[serde(default = "d_third_person")]
    pub third_person: bool,
    #[serde(default = "d_true")]
    pub show_hud: bool,
    #[serde(default = "d_autosave_minutes")]
    pub autosave_minutes: f32,
    /// 有効化されているプラグインのID一覧。
    #[serde(default)]
    pub enabled_plugins: Vec<String>,
    /// キー割り当て。
    #[serde(default)]
    pub keybinds: KeyBindings,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            render_distance: d_render_distance(),
            fov_degrees: d_fov(),
            mouse_sensitivity: d_sensitivity(),
            invert_mouse_y: true,
            gui_scale: d_gui_scale(),
            master_volume: d_master_volume(),
            music_volume: d_music_volume(),
            chunk_upload_budget: d_chunk_budget(),
            smooth_lighting: true,
            show_fog: true,
            view_bobbing: true,
            third_person: true,
            show_hud: true,
            autosave_minutes: d_autosave_minutes(),
            enabled_plugins: Vec::new(),
            keybinds: KeyBindings::default(),
        }
    }
}

impl GameSettings {
    /// 値が壊れていてもゲームが動くよう、全項目を安全域へ丸める。
    pub fn sanitize(&mut self) {
        self.render_distance = self.render_distance.clamp(2, 24);
        self.fov_degrees = clamp_finite(self.fov_degrees, 30.0, 120.0, d_fov());
        self.mouse_sensitivity = clamp_finite(self.mouse_sensitivity, 0.0004, 0.02, d_sensitivity());
        self.gui_scale = clamp_finite(self.gui_scale, 0.6, 2.0, 1.0);
        self.master_volume = clamp_finite(self.master_volume, 0.0, 1.0, 0.8);
        self.music_volume = clamp_finite(self.music_volume, 0.0, 1.0, 0.5);
        self.chunk_upload_budget = self.chunk_upload_budget.clamp(1, 32);
        self.autosave_minutes = clamp_finite(self.autosave_minutes, 0.0, 60.0, 5.0);
        self.enabled_plugins.sort();
        self.enabled_plugins.dedup();
    }

    pub fn path(root: &Path) -> PathBuf {
        root.join("config").join("settings.json")
    }

    pub fn load(root: &Path) -> Self {
        let path = Self::path(root);
        let mut s = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<GameSettings>(&text).unwrap_or_else(|e| {
                // 設定ファイルが壊れていても既定値で起動する。
                bevy::log::warn!("settings.json を読めなかったため既定値を使います: {e}");
                GameSettings::default()
            }),
            Err(_) => GameSettings::default(),
        };
        s.sanitize();
        s
    }

    pub fn save(&self, root: &Path) -> std::io::Result<()> {
        let path = Self::path(root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, text)
    }
}

#[inline]
fn clamp_finite(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_survive_sanitize_unchanged() {
        let mut a = GameSettings::default();
        let b = a.clone();
        a.sanitize();
        assert_eq!(a.render_distance, b.render_distance);
        assert_eq!(a.fov_degrees, b.fov_degrees);
        assert_eq!(a.chunk_upload_budget, b.chunk_upload_budget);
    }

    #[test]
    fn sanitize_repairs_hostile_values() {
        let mut s = GameSettings {
            render_distance: 9999,
            fov_degrees: f32::NAN,
            mouse_sensitivity: -5.0,
            gui_scale: f32::INFINITY,
            master_volume: 12.0,
            chunk_upload_budget: 0,
            autosave_minutes: -3.0,
            enabled_plugins: vec!["b".into(), "a".into(), "a".into()],
            ..Default::default()
        };
        s.sanitize();
        assert_eq!(s.render_distance, 24);
        assert_eq!(s.fov_degrees, d_fov());
        assert!(s.mouse_sensitivity > 0.0);
        assert!(s.gui_scale.is_finite() && s.gui_scale <= 2.0);
        assert_eq!(s.master_volume, 1.0);
        assert_eq!(s.chunk_upload_budget, 1);
        assert_eq!(s.autosave_minutes, 0.0);
        assert_eq!(s.enabled_plugins, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("wg_settings_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let mut s = GameSettings::default();
        s.render_distance = 12;
        s.fov_degrees = 90.0;
        s.enabled_plugins.push("example:more_ores".into());
        s.save(&dir).expect("save failed");

        let loaded = GameSettings::load(&dir);
        assert_eq!(loaded.render_distance, 12);
        assert_eq!(loaded.fov_degrees, 90.0);
        assert_eq!(loaded.enabled_plugins, vec!["example:more_ores".to_string()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("wg_settings_corrupt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::write(GameSettings::path(&dir), "{ this is not json ][").unwrap();

        let loaded = GameSettings::load(&dir);
        assert_eq!(loaded.render_distance, d_render_distance());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn partial_json_keeps_defaults_for_missing_fields() {
        let dir = std::env::temp_dir().join(format!("wg_settings_partial_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::write(GameSettings::path(&dir), r#"{"render_distance": 6}"#).unwrap();

        let loaded = GameSettings::load(&dir);
        assert_eq!(loaded.render_distance, 6);
        assert_eq!(loaded.fov_degrees, d_fov(), "missing field did not fall back to its default");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
