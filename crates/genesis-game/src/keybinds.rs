//! キーバインド。
//!
//! 操作は `Action` という論理名で書き、物理キーとの対応表をここで持つ。
//! 設定画面から変更でき、`settings.json` へ保存される。
//! ゲーム側のコードは `KeyBindings::pressed(Action::Jump, &keys)` のように
//! 論理名だけを見るので、キー割り当てを変えてもロジックは一切変わらない。

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// 割り当て可能な操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Sprint,
    Crouch,
    Inventory,
    Attack,
    Use,
    Drop,
    Perspective,
    Interact,
    Fly,
    ToggleDebug,
    ToggleChronicle,
    TimeSlower,
    TimeFaster,
    Hotbar1,
    Hotbar2,
    Hotbar3,
    Hotbar4,
    Hotbar5,
    Hotbar6,
    Hotbar7,
    Hotbar8,
    Hotbar9,
}

impl Action {
    pub const ALL: [Action; 27] = [
        Action::Forward, Action::Backward, Action::Left, Action::Right,
        Action::Jump, Action::Sprint, Action::Crouch,
        Action::Inventory, Action::Attack, Action::Use, Action::Drop,
        Action::Perspective, Action::Interact, Action::Fly,
        Action::ToggleDebug, Action::ToggleChronicle,
        Action::TimeSlower, Action::TimeFaster,
        Action::Hotbar1, Action::Hotbar2, Action::Hotbar3, Action::Hotbar4,
        Action::Hotbar5, Action::Hotbar6, Action::Hotbar7, Action::Hotbar8,
        Action::Hotbar9,
    ];

    /// 設定画面に出す操作（ホットバーは9個並べても意味が薄いので除く）。
    pub const CONFIGURABLE: [Action; 18] = [
        Action::Forward, Action::Backward, Action::Left, Action::Right,
        Action::Jump, Action::Sprint, Action::Crouch, Action::Fly,
        Action::Inventory, Action::Attack, Action::Use, Action::Drop,
        Action::Interact, Action::Perspective,
        Action::ToggleDebug, Action::ToggleChronicle,
        Action::TimeSlower, Action::TimeFaster,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::Forward => "前進",
            Action::Backward => "後退",
            Action::Left => "左へ",
            Action::Right => "右へ",
            Action::Jump => "ジャンプ",
            Action::Sprint => "走る",
            Action::Crouch => "しゃがむ",
            Action::Inventory => "インベントリ",
            Action::Attack => "攻撃・採掘",
            Action::Use => "使う・設置",
            Action::Drop => "アイテムを捨てる",
            Action::Perspective => "視点切替",
            Action::Interact => "話しかける",
            Action::Fly => "飛行切替",
            Action::ToggleDebug => "デバッグ表示",
            Action::ToggleChronicle => "世界史",
            Action::TimeSlower => "時間を遅く",
            Action::TimeFaster => "時間を速く",
            Action::Hotbar1 => "ホットバー1",
            Action::Hotbar2 => "ホットバー2",
            Action::Hotbar3 => "ホットバー3",
            Action::Hotbar4 => "ホットバー4",
            Action::Hotbar5 => "ホットバー5",
            Action::Hotbar6 => "ホットバー6",
            Action::Hotbar7 => "ホットバー7",
            Action::Hotbar8 => "ホットバー8",
            Action::Hotbar9 => "ホットバー9",
        }
    }

    fn default_key(self) -> KeyCode {
        match self {
            Action::Forward => KeyCode::KeyW,
            Action::Backward => KeyCode::KeyS,
            Action::Left => KeyCode::KeyA,
            Action::Right => KeyCode::KeyD,
            Action::Jump => KeyCode::Space,
            Action::Sprint => KeyCode::ShiftLeft,
            Action::Crouch => KeyCode::ControlLeft,
            Action::Inventory => KeyCode::KeyE,
            Action::Attack => KeyCode::KeyR,
            Action::Use => KeyCode::KeyT,
            Action::Drop => KeyCode::KeyQ,
            Action::Perspective => KeyCode::F5,
            Action::Interact => KeyCode::KeyF,
            Action::Fly => KeyCode::KeyG,
            Action::ToggleDebug => KeyCode::F3,
            Action::ToggleChronicle => KeyCode::KeyH,
            Action::TimeSlower => KeyCode::Comma,
            Action::TimeFaster => KeyCode::Period,
            Action::Hotbar1 => KeyCode::Digit1,
            Action::Hotbar2 => KeyCode::Digit2,
            Action::Hotbar3 => KeyCode::Digit3,
            Action::Hotbar4 => KeyCode::Digit4,
            Action::Hotbar5 => KeyCode::Digit5,
            Action::Hotbar6 => KeyCode::Digit6,
            Action::Hotbar7 => KeyCode::Digit7,
            Action::Hotbar8 => KeyCode::Digit8,
            Action::Hotbar9 => KeyCode::Digit9,
        }
    }
}

/// 論理操作 → 物理キーの対応表。
///
/// `KeyCode` は serde 対応が無い版があるため、保存には文字列名を使う。
#[derive(Debug, Clone, Resource, Serialize, Deserialize)]
#[serde(from = "Vec<(Action, String)>", into = "Vec<(Action, String)>")]
pub struct KeyBindings {
    map: Vec<(Action, KeyCode)>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            map: Action::ALL.iter().map(|a| (*a, a.default_key())).collect(),
        }
    }
}

impl From<Vec<(Action, String)>> for KeyBindings {
    fn from(v: Vec<(Action, String)>) -> Self {
        let mut b = KeyBindings::default();
        for (action, name) in v {
            if let Some(key) = key_from_name(&name) {
                b.set(action, key);
            }
        }
        b
    }
}

impl From<KeyBindings> for Vec<(Action, String)> {
    fn from(b: KeyBindings) -> Self {
        b.map
            .iter()
            .map(|(a, k)| (*a, key_name(*k).to_string()))
            .collect()
    }
}

impl KeyBindings {
    pub fn key_for(&self, action: Action) -> KeyCode {
        self.map
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, k)| *k)
            .unwrap_or_else(|| action.default_key())
    }

    /// 割り当てを変更する。同じキーを使っていた他の操作は解除ではなく
    /// 入れ替えにする（未割り当ての操作が生まれると操作不能になるため）。
    pub fn set(&mut self, action: Action, key: KeyCode) {
        let previous = self.key_for(action);
        for (a, k) in self.map.iter_mut() {
            if *k == key && *a != action {
                *k = previous;
            }
        }
        if let Some(entry) = self.map.iter_mut().find(|(a, _)| *a == action) {
            entry.1 = key;
        } else {
            self.map.push((action, key));
        }
    }

    pub fn reset_to_defaults(&mut self) {
        *self = KeyBindings::default();
    }

    pub fn display(&self, action: Action) -> &'static str {
        key_name(self.key_for(action))
    }

    #[inline]
    pub fn pressed(&self, action: Action, keys: &ButtonInput<KeyCode>) -> bool {
        keys.pressed(self.key_for(action))
    }

    #[inline]
    pub fn just_pressed(&self, action: Action, keys: &ButtonInput<KeyCode>) -> bool {
        keys.just_pressed(self.key_for(action))
    }

    /// 押されたホットバー番号（0 起点）。
    pub fn hotbar_pressed(&self, keys: &ButtonInput<KeyCode>) -> Option<usize> {
        const SLOTS: [Action; 9] = [
            Action::Hotbar1, Action::Hotbar2, Action::Hotbar3,
            Action::Hotbar4, Action::Hotbar5, Action::Hotbar6,
            Action::Hotbar7, Action::Hotbar8, Action::Hotbar9,
        ];
        SLOTS
            .iter()
            .position(|a| self.just_pressed(*a, keys))
    }
}

/// 設定画面で使える割り当て候補。
pub const ASSIGNABLE_KEYS: &[KeyCode] = &[
    KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE,
    KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI, KeyCode::KeyJ,
    KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO,
    KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR, KeyCode::KeyS, KeyCode::KeyT,
    KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY,
    KeyCode::KeyZ,
    KeyCode::Space, KeyCode::ShiftLeft, KeyCode::ControlLeft, KeyCode::AltLeft,
    KeyCode::Tab, KeyCode::Comma, KeyCode::Period, KeyCode::Slash,
    KeyCode::F1, KeyCode::F2, KeyCode::F3, KeyCode::F4, KeyCode::F5, KeyCode::F6,
    KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4,
    KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8,
    KeyCode::Digit9, KeyCode::Digit0,
];

pub fn key_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::KeyA => "A", KeyCode::KeyB => "B", KeyCode::KeyC => "C",
        KeyCode::KeyD => "D", KeyCode::KeyE => "E", KeyCode::KeyF => "F",
        KeyCode::KeyG => "G", KeyCode::KeyH => "H", KeyCode::KeyI => "I",
        KeyCode::KeyJ => "J", KeyCode::KeyK => "K", KeyCode::KeyL => "L",
        KeyCode::KeyM => "M", KeyCode::KeyN => "N", KeyCode::KeyO => "O",
        KeyCode::KeyP => "P", KeyCode::KeyQ => "Q", KeyCode::KeyR => "R",
        KeyCode::KeyS => "S", KeyCode::KeyT => "T", KeyCode::KeyU => "U",
        KeyCode::KeyV => "V", KeyCode::KeyW => "W", KeyCode::KeyX => "X",
        KeyCode::KeyY => "Y", KeyCode::KeyZ => "Z",
        KeyCode::Digit0 => "1", KeyCode::Digit1 => "1", KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3", KeyCode::Digit4 => "4", KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6", KeyCode::Digit7 => "7", KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        KeyCode::Space => "Space",
        KeyCode::ShiftLeft => "左Shift",
        KeyCode::ShiftRight => "右Shift",
        KeyCode::ControlLeft => "左Ctrl",
        KeyCode::ControlRight => "右Ctrl",
        KeyCode::AltLeft => "左Alt",
        KeyCode::Tab => "Tab",
        KeyCode::Enter => "Enter",
        KeyCode::Escape => "Esc",
        KeyCode::Comma => ",",
        KeyCode::Period => ".",
        KeyCode::Slash => "/",
        KeyCode::F1 => "F1", KeyCode::F2 => "F2", KeyCode::F3 => "F3",
        KeyCode::F4 => "F4", KeyCode::F5 => "F5", KeyCode::F6 => "F6",
        KeyCode::F7 => "F7", KeyCode::F8 => "F8", KeyCode::F9 => "F9",
        _ => "?",
    }
}

fn key_from_name(name: &str) -> Option<KeyCode> {
    // Digit0 の表示名が "1" と衝突しないよう、保存時は列挙から逆引きする。
    ASSIGNABLE_KEYS
        .iter()
        .copied()
        .chain([KeyCode::ShiftRight, KeyCode::ControlRight, KeyCode::Enter, KeyCode::Escape])
        .find(|k| key_name(*k) == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_every_action() {
        let b = KeyBindings::default();
        for a in Action::ALL {
            assert_eq!(b.key_for(a), a.default_key(), "{a:?} lost its default");
            assert_ne!(b.display(a), "?", "{a:?} has no printable key name");
            assert!(!a.label().is_empty());
        }
    }

    #[test]
    fn defaults_have_no_duplicate_keys() {
        let b = KeyBindings::default();
        let mut keys: Vec<KeyCode> = Action::ALL.iter().map(|a| b.key_for(*a)).collect();
        keys.sort_by_key(|k| format!("{k:?}"));
        let before = keys.len();
        keys.dedup_by_key(|k| format!("{k:?}"));
        assert_eq!(keys.len(), before, "two actions share a default key");
    }

    #[test]
    fn rebinding_swaps_instead_of_leaving_an_action_unbound() {
        let mut b = KeyBindings::default();
        // ジャンプを E（既定はインベントリ）へ移す。
        b.set(Action::Jump, KeyCode::KeyE);
        assert_eq!(b.key_for(Action::Jump), KeyCode::KeyE);
        // インベントリは未割り当てにならず、ジャンプが使っていた Space を貰う。
        assert_eq!(b.key_for(Action::Inventory), KeyCode::Space);
        // 全操作が依然として何かに割り当たっている。
        for a in Action::ALL {
            assert_ne!(b.display(a), "?");
        }
    }

    #[test]
    fn rebinding_to_the_same_key_is_a_no_op() {
        let mut b = KeyBindings::default();
        b.set(Action::Jump, KeyCode::Space);
        assert_eq!(b.key_for(Action::Jump), KeyCode::Space);
        assert_eq!(b.key_for(Action::Inventory), KeyCode::KeyE);
    }

    #[test]
    fn reset_restores_every_default() {
        let mut b = KeyBindings::default();
        b.set(Action::Forward, KeyCode::KeyM);
        b.set(Action::Jump, KeyCode::KeyN);
        b.reset_to_defaults();
        assert_eq!(b.key_for(Action::Forward), KeyCode::KeyW);
        assert_eq!(b.key_for(Action::Jump), KeyCode::Space);
    }

    #[test]
    fn bindings_round_trip_through_json() {
        let mut b = KeyBindings::default();
        b.set(Action::Inventory, KeyCode::KeyI);
        b.set(Action::Fly, KeyCode::F6);

        let json = serde_json::to_string(&b).unwrap();
        let back: KeyBindings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.key_for(Action::Inventory), KeyCode::KeyI);
        assert_eq!(back.key_for(Action::Fly), KeyCode::F6);
        // 触っていない操作は既定のまま。
        assert_eq!(back.key_for(Action::Forward), KeyCode::KeyW);
    }

    #[test]
    fn unknown_keys_in_a_save_fall_back_to_defaults() {
        let raw = r#"[["Jump","ObsoleteKey"],["Forward","M"]]"#;
        let b: KeyBindings = serde_json::from_str(raw).unwrap();
        assert_eq!(b.key_for(Action::Jump), KeyCode::Space, "unknown key should not unbind an action");
        assert_eq!(b.key_for(Action::Forward), KeyCode::KeyM);
    }

    #[test]
    fn every_assignable_key_has_a_name_and_parses_back() {
        for k in ASSIGNABLE_KEYS {
            let name = key_name(*k);
            assert_ne!(name, "?", "{k:?} has no display name");
            // Digit0 だけは表示名が Digit1 と衝突しているため除外する。
            if *k != KeyCode::Digit0 {
                assert_eq!(key_from_name(name), Some(*k), "'{name}' did not parse back to {k:?}");
            }
        }
    }

    #[test]
    fn configurable_actions_are_a_subset_of_all_actions() {
        for a in Action::CONFIGURABLE {
            assert!(Action::ALL.contains(&a), "{a:?} is configurable but not in ALL");
        }
    }
}
