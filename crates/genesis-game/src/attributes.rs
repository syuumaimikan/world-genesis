//! 才能と熟練度。
//!
//! すべての生物（プレイヤー・NPC・動物）は、生まれつきの**才能**と、
//! 経験で伸びる**熟練度**を持つ。才能は個体差（同じ狼でも足の速い個体・
//! 頑丈な個体がいる）、熟練度は「やればやるほど上手くなる」成長を表す。
//!
//! 決定論的に生成でき、Bevy に依存しない純関数として書いてある。

use serde::{Deserialize, Serialize};

/// 生まれつきの才能。基準値 1.0 からの倍率で表す。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Talents {
    /// 移動速度。
    pub agility: f32,
    /// 頑健さ（部位耐久の倍率）。
    pub toughness: f32,
    /// 体格（見た目と当たり判定の大きさ）。
    pub size: f32,
    /// 力（攻撃・採掘・運搬）。
    pub strength: f32,
    /// 知力（研究・魔法・学習速度）。
    pub intellect: f32,
    /// 魔力の器（最大マナ）。
    pub affinity: f32,
    /// 学習の速さ（熟練度の伸び）。
    pub learning: f32,
}

impl Default for Talents {
    fn default() -> Self {
        Self {
            agility: 1.0,
            toughness: 1.0,
            size: 1.0,
            strength: 1.0,
            intellect: 1.0,
            affinity: 1.0,
            learning: 1.0,
        }
    }
}

impl Talents {
    /// ハッシュ値から才能を生成する。同じ ID なら常に同じ才能。
    ///
    /// `spread` は個体差の大きさ（0 で全個体同一、0.3 でおよそ ±30%）。
    pub fn from_hash(h: u64, spread: f32) -> Self {
        // 各才能ごとに独立したハッシュを取り、相関を避ける。
        let roll = |salt: u64| -> f32 {
            let x = crate::noise::hash_u64(h ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            // 正規分布に近づけるため 3 つの一様乱数を平均する（中心極限）。
            let a = ((x >> 40) & 0xFFFF) as f32 / 65535.0;
            let b = ((x >> 24) & 0xFFFF) as f32 / 65535.0;
            let c = ((x >> 8) & 0xFFFF) as f32 / 65535.0;
            let n = (a + b + c) / 3.0; // 0..1、中心 0.5
            1.0 + (n - 0.5) * 2.0 * spread
        };
        Self {
            agility: roll(1).clamp(0.5, 1.8),
            toughness: roll(2).clamp(0.5, 1.8),
            size: roll(3).clamp(0.6, 1.6),
            strength: roll(4).clamp(0.5, 1.8),
            intellect: roll(5).clamp(0.4, 1.9),
            affinity: roll(6).clamp(0.2, 2.2),
            learning: roll(7).clamp(0.5, 1.7),
        }
    }
}

/// 熟練できる技能。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skill {
    Hunting,
    Farming,
    Mining,
    Woodcutting,
    Combat,
    Crafting,
    Cooking,
    Alchemy,
    Magic,
    Medicine,
    Trading,
    Research,
    Building,
    Fishing,
}

impl Skill {
    pub const ALL: [Skill; 14] = [
        Skill::Hunting, Skill::Farming, Skill::Mining, Skill::Woodcutting,
        Skill::Combat, Skill::Crafting, Skill::Cooking, Skill::Alchemy,
        Skill::Magic, Skill::Medicine, Skill::Trading, Skill::Research,
        Skill::Building, Skill::Fishing,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Skill::Hunting => "狩猟",
            Skill::Farming => "農耕",
            Skill::Mining => "採掘",
            Skill::Woodcutting => "伐採",
            Skill::Combat => "戦闘",
            Skill::Crafting => "工作",
            Skill::Cooking => "料理",
            Skill::Alchemy => "錬金術",
            Skill::Magic => "魔法",
            Skill::Medicine => "医術",
            Skill::Trading => "商才",
            Skill::Research => "研究",
            Skill::Building => "建築",
            Skill::Fishing => "釣り",
        }
    }
}

/// 1 技能の熟練状態。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SkillState {
    /// 累積経験値。
    pub xp: f32,
}

impl SkillState {
    /// 経験値から算出したレベル（0〜100）。
    /// 伸びは逓減する（最初は速く、後半は遅い）。
    pub fn level(&self) -> u32 {
        // level = 100 * (1 - exp(-xp / 800))
        let l = 100.0 * (1.0 - (-self.xp / 800.0).exp());
        l.round().clamp(0.0, 100.0) as u32
    }

    /// 熟練による効率倍率 1.0〜2.5。
    pub fn multiplier(&self) -> f32 {
        1.0 + self.level() as f32 / 100.0 * 1.5
    }

    /// 次のレベルまでの進捗 0〜1。
    pub fn progress(&self) -> f32 {
        let lvl = self.level();
        if lvl >= 100 {
            return 1.0;
        }
        // 現在レベルと次レベルの必要 xp を逆算。
        let xp_for = |l: u32| -800.0 * (1.0 - l as f32 / 100.0).max(1e-4).ln();
        let cur = xp_for(lvl);
        let next = xp_for(lvl + 1);
        if next <= cur {
            return 0.0;
        }
        ((self.xp - cur) / (next - cur)).clamp(0.0, 1.0)
    }
}

/// 生物 1 体の才能＋全技能の熟練度。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proficiency {
    pub talents: Talents,
    skills: Vec<(Skill, SkillState)>,
}

impl Proficiency {
    pub fn new(talents: Talents) -> Self {
        Self {
            talents,
            skills: Vec::new(),
        }
    }

    pub fn from_hash(h: u64, spread: f32) -> Self {
        Self::new(Talents::from_hash(h, spread))
    }

    fn state(&self, skill: Skill) -> SkillState {
        self.skills
            .iter()
            .find(|(s, _)| *s == skill)
            .map(|(_, st)| *st)
            .unwrap_or(SkillState { xp: 0.0 })
    }

    pub fn level(&self, skill: Skill) -> u32 {
        self.state(skill).level()
    }

    /// その技能の効率倍率（才能 × 熟練）。
    ///
    /// 才能はどの技能に効くかが異なる。例えば採掘は力、魔法は魔力の器。
    pub fn effectiveness(&self, skill: Skill) -> f32 {
        let talent = match skill {
            Skill::Mining | Skill::Woodcutting | Skill::Building => self.talents.strength,
            Skill::Combat | Skill::Hunting => (self.talents.strength + self.talents.agility) * 0.5,
            Skill::Magic => self.talents.affinity,
            Skill::Research | Skill::Alchemy => self.talents.intellect,
            Skill::Medicine => (self.talents.intellect + self.talents.learning) * 0.5,
            _ => 1.0,
        };
        (0.6 + 0.4 * talent) * self.state(skill).multiplier()
    }

    /// 技能を使ったときに経験値を得る。
    pub fn gain(&mut self, skill: Skill, base_xp: f32) {
        let gained = base_xp * self.talents.learning;
        if let Some(entry) = self.skills.iter_mut().find(|(s, _)| *s == skill) {
            entry.1.xp += gained;
        } else {
            self.skills.push((skill, SkillState { xp: gained }));
        }
    }

    /// 習得済みの技能一覧（レベル降順）。
    pub fn known_skills(&self) -> Vec<(Skill, u32)> {
        let mut v: Vec<(Skill, u32)> = self
            .skills
            .iter()
            .map(|(s, st)| (*s, st.level()))
            .filter(|(_, l)| *l > 0)
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn talents_are_deterministic() {
        let a = Talents::from_hash(12345, 0.3);
        let b = Talents::from_hash(12345, 0.3);
        assert_eq!(a.agility, b.agility);
        assert_eq!(a.affinity, b.affinity);
    }

    #[test]
    fn talents_stay_in_reasonable_bounds() {
        for i in 0..5000u64 {
            let t = Talents::from_hash(i, 0.35);
            for v in [t.agility, t.toughness, t.size, t.strength, t.intellect, t.affinity, t.learning] {
                assert!(v.is_finite() && v > 0.1 && v < 2.5, "talent out of range: {v}");
            }
        }
    }

    #[test]
    fn zero_spread_makes_every_individual_average() {
        let t = Talents::from_hash(999, 0.0);
        assert!((t.agility - 1.0).abs() < 1e-4);
        assert!((t.strength - 1.0).abs() < 1e-4);
    }

    #[test]
    fn individuals_actually_differ_with_spread() {
        let n = 200;
        let agilities: Vec<f32> = (0..n).map(|i| Talents::from_hash(i, 0.3).agility).collect();
        let min = agilities.iter().cloned().fold(f32::MAX, f32::min);
        let max = agilities.iter().cloned().fold(f32::MIN, f32::max);
        assert!(max - min > 0.3, "population shows no meaningful variation ({min}..{max})");
    }

    #[test]
    fn practice_raises_the_level_and_effectiveness() {
        let mut p = Proficiency::new(Talents::default());
        let before = p.effectiveness(Skill::Mining);
        for _ in 0..500 {
            p.gain(Skill::Mining, 5.0);
        }
        assert!(p.level(Skill::Mining) > 20, "500 uses should build real skill");
        assert!(p.effectiveness(Skill::Mining) > before * 1.2);
    }

    #[test]
    fn skill_growth_has_diminishing_returns() {
        let mut p = Proficiency::new(Talents::default());
        for _ in 0..100 {
            p.gain(Skill::Combat, 10.0);
        }
        let early = p.level(Skill::Combat);
        for _ in 0..100 {
            p.gain(Skill::Combat, 10.0);
        }
        let mid = p.level(Skill::Combat);
        for _ in 0..100 {
            p.gain(Skill::Combat, 10.0);
        }
        let late = p.level(Skill::Combat);
        // 同じ経験量でも、後半のレベルの伸びは前半より小さい。
        assert!(mid - early >= late - mid, "growth should slow down, not speed up");
        assert!(late <= 100);
    }

    #[test]
    fn fast_learners_progress_quicker() {
        let mut slow = Proficiency::new(Talents { learning: 0.6, ..Default::default() });
        let mut fast = Proficiency::new(Talents { learning: 1.6, ..Default::default() });
        for _ in 0..200 {
            slow.gain(Skill::Research, 4.0);
            fast.gain(Skill::Research, 4.0);
        }
        assert!(fast.level(Skill::Research) > slow.level(Skill::Research));
    }

    #[test]
    fn talent_shapes_which_skills_you_excel_at() {
        let strong = Proficiency::new(Talents { strength: 1.7, affinity: 0.5, ..Default::default() });
        let mage = Proficiency::new(Talents { strength: 0.5, affinity: 1.9, ..Default::default() });
        // 力自慢は採掘、魔力持ちは魔法が得意（同じ未熟練でも才能差が出る）。
        assert!(strong.effectiveness(Skill::Mining) > mage.effectiveness(Skill::Mining));
        assert!(mage.effectiveness(Skill::Magic) > strong.effectiveness(Skill::Magic));
    }

    #[test]
    fn known_skills_are_listed_by_level() {
        let mut p = Proficiency::new(Talents::default());
        for _ in 0..300 {
            p.gain(Skill::Farming, 5.0);
        }
        for _ in 0..50 {
            p.gain(Skill::Cooking, 5.0);
        }
        let known = p.known_skills();
        assert_eq!(known[0].0, Skill::Farming, "the most-practised skill should be first");
        assert!(known.iter().all(|(_, l)| *l > 0));
    }

    #[test]
    fn serialization_round_trips() {
        let mut p = Proficiency::from_hash(77, 0.3);
        p.gain(Skill::Magic, 100.0);
        let json = serde_json::to_string(&p).unwrap();
        let back: Proficiency = serde_json::from_str(&json).unwrap();
        assert_eq!(back.level(Skill::Magic), p.level(Skill::Magic));
        assert_eq!(back.talents.affinity, p.talents.affinity);
    }
}
