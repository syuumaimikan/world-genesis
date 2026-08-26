//! 魔法とマナ。
//!
//! この世界には電気エネルギーのほかに**マナ**が流れている。
//! 魔法は決められた呪文の一覧から選ぶのではなく、**グリフ**を並べて組み立てる。
//!
//!   [形式] → [効果] → [修飾]…
//!   例: 投射 → 火炎 → 拡大 → 貫通
//!
//! 文法さえ満たしていれば、どんな並びでも呪文として成立する。
//! そのため利用者は JSON も Rust も触らずに、ゲーム内で新しい魔法を発明できる。
//! 魔道具（`Device`）も同じ仕組みで動き、マナを消費して効果を発する。

use serde::{Deserialize, Serialize};

/// グリフの分類。呪文の文法はこの並び順で決まる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlyphClass {
    /// 発動形式（投射・自身・範囲・接触…）。呪文の先頭に必ず 1 つ。
    Form,
    /// 効果（火炎・治癒・成長…）。1 つ以上必要。
    Effect,
    /// 修飾（拡大・延長・貫通…）。効果の後ろに任意個。
    Modifier,
}

/// グリフ 1 つ。
#[derive(Debug, Clone)]
pub struct Glyph {
    pub id: &'static str,
    pub name: &'static str,
    pub class: GlyphClass,
    /// 基礎マナ消費。
    pub mana_cost: f32,
    /// 修飾の場合、後続の効果へ掛かる倍率。
    pub power_scale: f32,
    /// 修飾の場合、マナ消費へ掛かる倍率。
    pub cost_scale: f32,
    pub description: &'static str,
}

/// 世界に存在するグリフ。プラグインからも追加できる。
pub const GLYPHS: &[Glyph] = &[
    // --- 形式 ---
    Glyph { id: "form_projectile", name: "投射", class: GlyphClass::Form, mana_cost: 6.0, power_scale: 1.0, cost_scale: 1.0,
        description: "術者の前方へ魔力の弾を飛ばす" },
    Glyph { id: "form_self", name: "自身", class: GlyphClass::Form, mana_cost: 3.0, power_scale: 1.0, cost_scale: 1.0,
        description: "術者自身に効果を及ぼす" },
    Glyph { id: "form_touch", name: "接触", class: GlyphClass::Form, mana_cost: 4.0, power_scale: 1.2, cost_scale: 1.0,
        description: "手の届く範囲の対象へ、強く効果を及ぼす" },
    Glyph { id: "form_area", name: "範囲", class: GlyphClass::Form, mana_cost: 14.0, power_scale: 0.7, cost_scale: 1.0,
        description: "術者を中心とした領域へ効果を撒く" },
    Glyph { id: "form_rune", name: "地脈", class: GlyphClass::Form, mana_cost: 18.0, power_scale: 1.0, cost_scale: 1.0,
        description: "地面に魔法陣を刻み、踏んだものへ発動する" },
    Glyph { id: "form_beam", name: "光条", class: GlyphClass::Form, mana_cost: 11.0, power_scale: 0.9, cost_scale: 1.0,
        description: "直線状に効果を貫き通す" },

    // --- 効果 ---
    Glyph { id: "effect_flame", name: "火炎", class: GlyphClass::Effect, mana_cost: 10.0, power_scale: 1.0, cost_scale: 1.0,
        description: "対象を焼く。可燃物に引火する" },
    Glyph { id: "effect_frost", name: "凍結", class: GlyphClass::Effect, mana_cost: 9.0, power_scale: 1.0, cost_scale: 1.0,
        description: "対象を凍らせ、動きを鈍らせる。水を氷に変える" },
    Glyph { id: "effect_shock", name: "雷撃", class: GlyphClass::Effect, mana_cost: 13.0, power_scale: 1.0, cost_scale: 1.0,
        description: "電撃を放つ。金属を伝い、水中で広がる" },
    Glyph { id: "effect_heal", name: "治癒", class: GlyphClass::Effect, mana_cost: 12.0, power_scale: 1.0, cost_scale: 1.0,
        description: "傷を塞ぎ、出血を止める" },
    Glyph { id: "effect_cure", name: "解毒", class: GlyphClass::Effect, mana_cost: 15.0, power_scale: 1.0, cost_scale: 1.0,
        description: "病と毒を体から追い出す" },
    Glyph { id: "effect_growth", name: "成長", class: GlyphClass::Effect, mana_cost: 8.0, power_scale: 1.0, cost_scale: 1.0,
        description: "植物を育て、作物を実らせる" },
    Glyph { id: "effect_break", name: "破砕", class: GlyphClass::Effect, mana_cost: 11.0, power_scale: 1.0, cost_scale: 1.0,
        description: "岩を砕く。採掘に使える" },
    Glyph { id: "effect_lift", name: "浮遊", class: GlyphClass::Effect, mana_cost: 10.0, power_scale: 1.0, cost_scale: 1.0,
        description: "対象を持ち上げ、落下を和らげる" },
    Glyph { id: "effect_light", name: "灯火", class: GlyphClass::Effect, mana_cost: 4.0, power_scale: 1.0, cost_scale: 1.0,
        description: "光源を生む" },
    Glyph { id: "effect_ward", name: "護り", class: GlyphClass::Effect, mana_cost: 14.0, power_scale: 1.0, cost_scale: 1.0,
        description: "受ける傷を軽減する膜を張る" },
    Glyph { id: "effect_haste", name: "疾駆", class: GlyphClass::Effect, mana_cost: 9.0, power_scale: 1.0, cost_scale: 1.0,
        description: "動きを速める" },
    Glyph { id: "effect_summon_water", name: "湧水", class: GlyphClass::Effect, mana_cost: 12.0, power_scale: 1.0, cost_scale: 1.0,
        description: "水を生み出す" },

    // --- 修飾 ---
    Glyph { id: "mod_amplify", name: "拡大", class: GlyphClass::Modifier, mana_cost: 0.0, power_scale: 1.6, cost_scale: 1.7,
        description: "効果を強める" },
    Glyph { id: "mod_extend", name: "延長", class: GlyphClass::Modifier, mana_cost: 0.0, power_scale: 1.15, cost_scale: 1.35,
        description: "効果の持続を延ばす" },
    Glyph { id: "mod_pierce", name: "貫通", class: GlyphClass::Modifier, mana_cost: 0.0, power_scale: 1.1, cost_scale: 1.45,
        description: "遮蔽を貫き、複数の対象を捉える" },
    Glyph { id: "mod_split", name: "分裂", class: GlyphClass::Modifier, mana_cost: 0.0, power_scale: 0.65, cost_scale: 1.8,
        description: "効果を複数に分ける" },
    Glyph { id: "mod_delay", name: "遅延", class: GlyphClass::Modifier, mana_cost: 0.0, power_scale: 1.0, cost_scale: 1.1,
        description: "発動を遅らせる" },
    Glyph { id: "mod_frugal", name: "節約", class: GlyphClass::Modifier, mana_cost: 0.0, power_scale: 0.55, cost_scale: 0.45,
        description: "威力と引き換えに消費を抑える" },
];

pub fn glyph(id: &str) -> Option<&'static Glyph> {
    GLYPHS.iter().find(|g| g.id == id)
}

/// 呪文の組み立てに失敗した理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpellError {
    Empty,
    /// 先頭が形式グリフでない。
    MustStartWithForm,
    /// 効果グリフが 1 つも無い。
    NoEffect,
    /// 形式グリフが複数ある。
    MultipleForms,
    /// 効果の前に修飾が置かれている。
    ModifierBeforeEffect,
    UnknownGlyph(String),
    /// 長すぎる（詠唱が破綻する）。
    TooLong,
}

impl SpellError {
    pub fn message(&self) -> String {
        match self {
            SpellError::Empty => "グリフが並んでいません。".into(),
            SpellError::MustStartWithForm => "呪文は形式のグリフ（投射・自身・範囲など）から始めます。".into(),
            SpellError::NoEffect => "効果のグリフが必要です。".into(),
            SpellError::MultipleForms => "形式のグリフは 1 つだけです。".into(),
            SpellError::ModifierBeforeEffect => "修飾のグリフは、効果のグリフの後ろに置きます。".into(),
            SpellError::UnknownGlyph(id) => format!("未知のグリフ『{id}』です。"),
            SpellError::TooLong => "グリフが多すぎて詠唱が保ちません（最大 12）。".into(),
        }
    }
}

pub const MAX_GLYPHS: usize = 12;

/// 組み上がった呪文。
#[derive(Debug, Clone)]
pub struct Spell {
    pub name: String,
    pub glyph_ids: Vec<String>,
    pub form: &'static Glyph,
    /// 効果ごとの (グリフ, 威力)。修飾の倍率が掛かった後の値。
    pub effects: Vec<(&'static Glyph, f32)>,
    pub mana_cost: f32,
    /// 発動までの詠唱時間（秒）。
    pub cast_time: f32,
}

impl Spell {
    /// グリフ列を検証して呪文を組む。
    pub fn compose(glyph_ids: &[String]) -> Result<Spell, SpellError> {
        if glyph_ids.is_empty() {
            return Err(SpellError::Empty);
        }
        if glyph_ids.len() > MAX_GLYPHS {
            return Err(SpellError::TooLong);
        }

        let mut resolved = Vec::with_capacity(glyph_ids.len());
        for id in glyph_ids {
            match glyph(id) {
                Some(g) => resolved.push(g),
                None => return Err(SpellError::UnknownGlyph(id.clone())),
            }
        }

        if resolved[0].class != GlyphClass::Form {
            return Err(SpellError::MustStartWithForm);
        }
        if resolved.iter().filter(|g| g.class == GlyphClass::Form).count() > 1 {
            return Err(SpellError::MultipleForms);
        }
        if !resolved.iter().any(|g| g.class == GlyphClass::Effect) {
            return Err(SpellError::NoEffect);
        }
        // 最初の効果より前に修飾があってはいけない。
        let first_effect = resolved
            .iter()
            .position(|g| g.class == GlyphClass::Effect)
            .unwrap();
        if resolved[..first_effect].iter().any(|g| g.class == GlyphClass::Modifier) {
            return Err(SpellError::ModifierBeforeEffect);
        }

        let form = resolved[0];
        let mut mana = form.mana_cost;
        let mut effects: Vec<(&'static Glyph, f32)> = Vec::new();

        // 効果を読み、その直後に続く修飾をその効果へ適用する。
        let mut i = 1;
        while i < resolved.len() {
            let g = resolved[i];
            if g.class != GlyphClass::Effect {
                i += 1;
                continue;
            }
            let mut power = form.power_scale;
            let mut cost = g.mana_cost;
            let mut j = i + 1;
            while j < resolved.len() && resolved[j].class == GlyphClass::Modifier {
                power *= resolved[j].power_scale;
                cost *= resolved[j].cost_scale;
                j += 1;
            }
            effects.push((g, power));
            mana += cost;
            i = j;
        }

        // 詠唱時間はグリフ数とマナ量から。
        let cast_time = 0.35 + resolved.len() as f32 * 0.12 + mana * 0.008;
        let name = Self::auto_name(&resolved);

        Ok(Spell {
            name,
            glyph_ids: glyph_ids.to_vec(),
            form,
            effects,
            mana_cost: mana,
            cast_time,
        })
    }

    /// グリフの並びから、それらしい呪文名を作る。
    fn auto_name(glyphs: &[&'static Glyph]) -> String {
        let form = glyphs[0].name;
        let effects: Vec<&str> = glyphs
            .iter()
            .filter(|g| g.class == GlyphClass::Effect)
            .map(|g| g.name)
            .collect();
        let mods: Vec<&str> = glyphs
            .iter()
            .filter(|g| g.class == GlyphClass::Modifier)
            .map(|g| g.name)
            .collect();
        if mods.is_empty() {
            format!("{}の{}", effects.join("・"), form)
        } else {
            format!("{}{}の{}", mods.join(""), effects.join("・"), form)
        }
    }

    /// 術者の技量に応じた実効マナ消費。熟練するほど無駄が減る。
    pub fn effective_cost(&self, magic_effectiveness: f32) -> f32 {
        (self.mana_cost / magic_effectiveness.max(0.2)).max(1.0)
    }

    /// 効果の説明（UI 用）。
    pub fn describe(&self) -> String {
        let mut lines = vec![format!("《{}》", self.name)];
        lines.push(format!("形式: {} — {}", self.form.name, self.form.description));
        for (g, power) in &self.effects {
            lines.push(format!("効果: {} ×{:.2} — {}", g.name, power, g.description));
        }
        lines.push(format!("消費マナ {:.0} / 詠唱 {:.1}秒", self.mana_cost, self.cast_time));
        lines
            .into_iter()
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// 術者のマナ。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManaPool {
    pub current: f32,
    pub max: f32,
    /// 毎秒の自然回復量。
    pub regen: f32,
}

impl ManaPool {
    /// 才能（魔力の器）から作る。
    pub fn from_affinity(affinity: f32) -> Self {
        let max = 40.0 + 60.0 * affinity.clamp(0.0, 2.5);
        Self {
            current: max,
            max,
            regen: 0.6 * affinity.clamp(0.2, 2.5),
        }
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            0.0
        } else {
            (self.current / self.max).clamp(0.0, 1.0)
        }
    }

    /// 環境のマナ濃度を加味して回復する。
    /// `ley_density` は地脈の濃さ（1.0 が標準、地脈の上では高い）。
    pub fn regenerate(&mut self, dt_seconds: f32, ley_density: f32) {
        self.current = (self.current + self.regen * ley_density.clamp(0.0, 4.0) * dt_seconds)
            .clamp(0.0, self.max);
    }

    pub fn can_pay(&self, cost: f32) -> bool {
        self.current >= cost
    }

    /// 消費する。足りなければ false（呪文は不発）。
    pub fn pay(&mut self, cost: f32) -> bool {
        if !self.can_pay(cost) {
            return false;
        }
        self.current -= cost;
        true
    }
}

/// 地脈（マナの流れ）の濃さ。場所によって魔法の効きが変わる。
pub fn ley_density(seed: u64, wx: f32, wz: f32) -> f32 {
    // 大きなうねりと、点在する濃い結節点。
    let broad = crate::noise::fbm2(wx * 0.0012, wz * 0.0012, seed ^ 0x1E7A, 3, 2.0, 0.5);
    let (node_dist, _) = crate::noise::voronoi2(wx * 0.0035, wz * 0.0035, seed ^ 0x7EA1);
    let node = (1.0 - node_dist * 3.0).clamp(0.0, 1.0);
    (1.0 + broad * 0.5 + node * 1.8).clamp(0.15, 4.0)
}

/// 魔道具：マナを消費して呪文を自動で発動する装置。
#[derive(Debug, Clone)]
pub struct Device {
    pub name: String,
    pub spell: Spell,
    /// 蓄えているマナ。
    pub stored: f32,
    pub capacity: f32,
    /// 発動間隔（秒）。
    pub interval: f32,
    pub cooldown: f32,
    pub active: bool,
}

impl Device {
    pub fn new(name: impl Into<String>, spell: Spell, capacity: f32, interval: f32) -> Self {
        Self {
            name: name.into(),
            spell,
            stored: 0.0,
            capacity,
            interval: interval.max(0.1),
            cooldown: 0.0,
            active: true,
        }
    }

    /// 地脈からマナを吸い上げる。
    pub fn absorb(&mut self, dt_seconds: f32, ley: f32) {
        self.stored = (self.stored + dt_seconds * ley * 1.2).min(self.capacity);
    }

    /// 時間を進め、発動できたら true。
    pub fn tick(&mut self, dt_seconds: f32) -> bool {
        if !self.active {
            return false;
        }
        self.cooldown -= dt_seconds;
        if self.cooldown > 0.0 {
            return false;
        }
        if self.stored < self.spell.mana_cost {
            return false;
        }
        self.stored -= self.spell.mana_cost;
        self.cooldown = self.interval;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn every_glyph_is_well_formed() {
        for g in GLYPHS {
            assert!(!g.id.is_empty() && !g.name.is_empty());
            assert!(!g.description.is_empty(), "{} has no description", g.id);
            assert!(g.mana_cost >= 0.0);
            assert!(g.power_scale > 0.0);
            assert!(g.cost_scale > 0.0);
            // 修飾はそれ自体では消費しない（後続へ倍率で効く）。
            if g.class == GlyphClass::Modifier {
                assert_eq!(g.mana_cost, 0.0, "{} is a modifier but has its own cost", g.id);
            }
        }
        let mut seen: Vec<&str> = GLYPHS.iter().map(|g| g.id).collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before, "duplicate glyph ids");
    }

    #[test]
    fn a_simple_spell_composes() {
        let s = Spell::compose(&ids(&["form_projectile", "effect_flame"])).unwrap();
        assert_eq!(s.form.id, "form_projectile");
        assert_eq!(s.effects.len(), 1);
        assert!(s.mana_cost > 0.0);
        assert!(s.cast_time > 0.0);
        assert!(s.name.contains("火炎"), "auto-name should mention the effect: {}", s.name);
    }

    #[test]
    fn grammar_is_enforced() {
        assert_eq!(Spell::compose(&[]).unwrap_err(), SpellError::Empty);
        assert_eq!(
            Spell::compose(&ids(&["effect_flame"])).unwrap_err(),
            SpellError::MustStartWithForm
        );
        assert_eq!(
            Spell::compose(&ids(&["form_self"])).unwrap_err(),
            SpellError::NoEffect
        );
        assert_eq!(
            Spell::compose(&ids(&["form_self", "form_area", "effect_heal"])).unwrap_err(),
            SpellError::MultipleForms
        );
        assert_eq!(
            Spell::compose(&ids(&["form_self", "mod_amplify", "effect_heal"])).unwrap_err(),
            SpellError::ModifierBeforeEffect
        );
        assert!(matches!(
            Spell::compose(&ids(&["form_self", "effect_nope"])).unwrap_err(),
            SpellError::UnknownGlyph(_)
        ));
        let long = ids(&["form_self"]).into_iter().chain((0..20).map(|_| "effect_heal".to_string())).collect::<Vec<_>>();
        assert_eq!(Spell::compose(&long).unwrap_err(), SpellError::TooLong);
    }

    #[test]
    fn every_error_explains_itself_in_japanese() {
        for e in [
            SpellError::Empty,
            SpellError::MustStartWithForm,
            SpellError::NoEffect,
            SpellError::MultipleForms,
            SpellError::ModifierBeforeEffect,
            SpellError::TooLong,
            SpellError::UnknownGlyph("x".into()),
        ] {
            assert!(!e.message().is_empty());
        }
    }

    #[test]
    fn amplify_raises_both_power_and_cost() {
        let plain = Spell::compose(&ids(&["form_projectile", "effect_flame"])).unwrap();
        let amped = Spell::compose(&ids(&["form_projectile", "effect_flame", "mod_amplify"])).unwrap();
        assert!(amped.effects[0].1 > plain.effects[0].1, "amplify should raise power");
        assert!(amped.mana_cost > plain.mana_cost, "amplify should cost more");
    }

    #[test]
    fn frugal_trades_power_for_cost() {
        let plain = Spell::compose(&ids(&["form_projectile", "effect_shock"])).unwrap();
        let cheap = Spell::compose(&ids(&["form_projectile", "effect_shock", "mod_frugal"])).unwrap();
        assert!(cheap.mana_cost < plain.mana_cost);
        assert!(cheap.effects[0].1 < plain.effects[0].1);
    }

    #[test]
    fn modifiers_only_affect_the_effect_they_follow() {
        // 火炎だけを強め、治癒はそのまま。
        let s = Spell::compose(&ids(&[
            "form_area", "effect_flame", "mod_amplify", "effect_heal",
        ]))
        .unwrap();
        assert_eq!(s.effects.len(), 2);
        let flame = s.effects.iter().find(|(g, _)| g.id == "effect_flame").unwrap().1;
        let heal = s.effects.iter().find(|(g, _)| g.id == "effect_heal").unwrap().1;
        assert!(flame > heal, "amplify leaked onto the wrong effect");
    }

    #[test]
    fn area_form_spreads_but_weakens_while_touch_concentrates() {
        let area = Spell::compose(&ids(&["form_area", "effect_flame"])).unwrap();
        let touch = Spell::compose(&ids(&["form_touch", "effect_flame"])).unwrap();
        assert!(touch.effects[0].1 > area.effects[0].1, "touch should hit harder than a spread");
        assert!(area.mana_cost > touch.mana_cost, "an area spell should cost more");
    }

    #[test]
    fn users_can_invent_spells_that_were_never_predefined() {
        // 「分裂させた凍結の光条」——どこにも定義されていない組み合わせ。
        let s = Spell::compose(&ids(&[
            "form_beam", "effect_frost", "mod_split", "mod_extend",
        ]))
        .unwrap();
        assert!(s.mana_cost > 0.0);
        assert!(s.describe().contains("凍結"));
        // 複数の効果と修飾を自由に混ぜられる。
        let complex = Spell::compose(&ids(&[
            "form_rune", "effect_ward", "mod_extend", "effect_haste", "mod_amplify", "effect_light",
        ]))
        .unwrap();
        assert_eq!(complex.effects.len(), 3);
    }

    #[test]
    fn mana_pool_scales_with_affinity_and_never_goes_negative() {
        let weak = ManaPool::from_affinity(0.3);
        let strong = ManaPool::from_affinity(2.2);
        assert!(strong.max > weak.max * 1.5);

        let mut p = ManaPool::from_affinity(1.0);
        assert!(p.pay(10.0));
        let before = p.current;
        assert!(!p.pay(p.max * 10.0), "should not be able to overdraw mana");
        assert_eq!(p.current, before);
        assert!(p.current >= 0.0);
    }

    #[test]
    fn mana_regenerates_faster_on_a_ley_line() {
        let mut normal = ManaPool::from_affinity(1.0);
        let mut ley = ManaPool::from_affinity(1.0);
        normal.current = 0.0;
        ley.current = 0.0;
        normal.regenerate(10.0, 1.0);
        ley.regenerate(10.0, 3.5);
        assert!(ley.current > normal.current, "ley lines should speed up recovery");
        // 上限は超えない。
        ley.regenerate(100_000.0, 4.0);
        assert!((ley.current - ley.max).abs() < 1e-3);
    }

    #[test]
    fn ley_density_is_deterministic_and_bounded() {
        for i in 0..500 {
            let x = i as f32 * 13.7;
            let z = i as f32 * -7.3;
            let a = ley_density(99, x, z);
            let b = ley_density(99, x, z);
            assert_eq!(a, b);
            assert!(a.is_finite() && (0.15..=4.0).contains(&a), "ley density out of range: {a}");
        }
    }

    #[test]
    fn ley_density_actually_varies_across_the_world() {
        let samples: Vec<f32> = (0..200).map(|i| ley_density(7, i as f32 * 90.0, i as f32 * 55.0)).collect();
        let min = samples.iter().cloned().fold(f32::MAX, f32::min);
        let max = samples.iter().cloned().fold(f32::MIN, f32::max);
        assert!(max - min > 0.3, "the world has a uniform ley field ({min}..{max})");
    }

    #[test]
    fn skill_reduces_the_real_cost_of_casting() {
        let s = Spell::compose(&ids(&["form_projectile", "effect_flame"])).unwrap();
        let novice = s.effective_cost(0.8);
        let master = s.effective_cost(2.4);
        assert!(master < novice, "a skilled mage should waste less mana");
        assert!(master >= 1.0, "cost should never drop to zero");
    }

    #[test]
    fn a_magical_device_charges_from_the_ley_and_fires_on_a_cycle() {
        let spell = Spell::compose(&ids(&["form_area", "effect_light"])).unwrap();
        let mut d = Device::new("常夜灯", spell, 200.0, 5.0);
        // 充電前は撃てない。
        assert!(!d.tick(1.0));
        d.absorb(60.0, 2.0);
        assert!(d.tick(0.0), "a charged device should fire");
        // 撃った直後は間隔待ち。
        assert!(!d.tick(1.0));
        // 間隔を過ぎれば、マナがある限りまた撃つ。
        assert!(d.tick(10.0));
    }

    #[test]
    fn an_inactive_device_never_fires() {
        let spell = Spell::compose(&ids(&["form_self", "effect_heal"])).unwrap();
        let mut d = Device::new("休止中の護符", spell, 500.0, 1.0);
        d.absorb(500.0, 4.0);
        d.active = false;
        assert!(!d.tick(100.0));
    }
}
