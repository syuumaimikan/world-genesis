//! 部位別ダメージモデル（First Aid 方式）。
//!
//! このゲームには単一の「HP」は存在しない。体は複数の部位に分かれ、
//! それぞれが独立した損傷度を持つ。頭や胴を潰されれば即死し、
//! 手足を失えば戦闘・移動に支障が出る。自然回復はほとんど無く、
//! 包帯・添え木・ポーションといった手当てで初めて傷が塞がる。
//!
//! Bevy に依存しない純関数として書いてあるため、戦闘計算も死亡判定も
//! 単体テストで検証できる。

use serde::{Deserialize, Serialize};

/// 体の部位。人型と獣型で共通に使う（獣は腕を前脚として扱う）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPart {
    Head,
    Torso,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

impl BodyPart {
    pub const ALL: [BodyPart; 6] = [
        BodyPart::Head,
        BodyPart::Torso,
        BodyPart::LeftArm,
        BodyPart::RightArm,
        BodyPart::LeftLeg,
        BodyPart::RightLeg,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BodyPart::Head => "頭",
            BodyPart::Torso => "胴",
            BodyPart::LeftArm => "左腕",
            BodyPart::RightArm => "右腕",
            BodyPart::LeftLeg => "左脚",
            BodyPart::RightLeg => "右脚",
        }
    }

    /// この部位が壊れると即死するか。
    pub fn is_vital(self) -> bool {
        matches!(self, BodyPart::Head | BodyPart::Torso)
    }

    /// 部位ごとの最大耐久。頭は脆く、胴は頑丈。
    pub fn max_condition(self) -> f32 {
        match self {
            BodyPart::Head => 25.0,
            BodyPart::Torso => 45.0,
            BodyPart::LeftArm | BodyPart::RightArm => 22.0,
            BodyPart::LeftLeg | BodyPart::RightLeg => 26.0,
        }
    }

    /// ランダムな被弾時にこの部位へ当たる相対確率。胴が最も広い。
    pub fn hit_weight(self) -> f32 {
        match self {
            BodyPart::Head => 0.12,
            BodyPart::Torso => 0.40,
            BodyPart::LeftArm | BodyPart::RightArm => 0.11,
            BodyPart::LeftLeg | BodyPart::RightLeg => 0.13,
        }
    }
}

/// 部位1つの状態。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PartState {
    /// 現在の耐久（0 で機能停止）。
    pub condition: f32,
    /// 出血の強さ（毎秒失われる耐久）。手当てするまで止まらない。
    pub bleeding: f32,
    /// 骨折しているか。添え木が要る。移動・攻撃に影響。
    pub fractured: bool,
    /// 包帯が巻かれているか（出血を止め、ゆっくり回復する）。
    pub bandaged: bool,
}

impl PartState {
    fn full(part: BodyPart) -> Self {
        Self {
            condition: part.max_condition(),
            bleeding: 0.0,
            fractured: false,
            bandaged: false,
        }
    }

    pub fn is_broken(&self) -> bool {
        self.condition <= 0.0
    }
}

/// 損傷の種類。ダメージの入り方が変わる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageKind {
    /// 斬撃：出血しやすい。
    Slashing,
    /// 刺突：深く入り、強い出血。
    Piercing,
    /// 打撲：骨折しやすいが出血は少ない。
    Blunt,
    /// 火傷：出血なし、じわじわ痛む。
    Burn,
    /// 毒・病：出血・骨折なし。
    Toxic,
    /// 落下・衝撃：脚に骨折を起こしやすい。
    Impact,
}

/// 生体全体の状態。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    pub head: PartState,
    pub torso: PartState,
    pub left_arm: PartState,
    pub right_arm: PartState,
    pub left_leg: PartState,
    pub right_leg: PartState,
    /// 失血ショックの進行（0〜1）。1 で失血死。
    pub blood_loss: f32,
    /// 痛み（0〜1）。行動効率を下げる。
    pub pain: f32,
    /// 才能による頑健さの倍率。部位の最大耐久すべてに掛かる。
    pub toughness: f32,
    pub dead: bool,
    /// 死因（歴史記録用）。
    pub cause_of_death: Option<String>,
}

impl Default for Body {
    fn default() -> Self {
        Self::healthy()
    }
}

impl Body {
    pub fn healthy() -> Self {
        Self {
            head: PartState::full(BodyPart::Head),
            torso: PartState::full(BodyPart::Torso),
            left_arm: PartState::full(BodyPart::LeftArm),
            right_arm: PartState::full(BodyPart::RightArm),
            left_leg: PartState::full(BodyPart::LeftLeg),
            right_leg: PartState::full(BodyPart::RightLeg),
            blood_loss: 0.0,
            pain: 0.0,
            toughness: 1.0,
            dead: false,
            cause_of_death: None,
        }
    }

    /// この体における部位の最大耐久（才能の倍率込み）。
    #[inline]
    pub fn max_of(&self, part: BodyPart) -> f32 {
        part.max_condition() * self.toughness
    }

    /// 才能に応じて全部位の耐久を倍率でスケールする（頑健な個体）。
    pub fn scaled(toughness: f32) -> Self {
        let mut b = Self::healthy();
        b.toughness = toughness.clamp(0.4, 2.5);
        for part in BodyPart::ALL {
            let max = b.max_of(part);
            b.part_mut(part).condition = max;
        }
        b
    }

    pub fn part(&self, part: BodyPart) -> &PartState {
        match part {
            BodyPart::Head => &self.head,
            BodyPart::Torso => &self.torso,
            BodyPart::LeftArm => &self.left_arm,
            BodyPart::RightArm => &self.right_arm,
            BodyPart::LeftLeg => &self.left_leg,
            BodyPart::RightLeg => &self.right_leg,
        }
    }

    pub fn part_mut(&mut self, part: BodyPart) -> &mut PartState {
        match part {
            BodyPart::Head => &mut self.head,
            BodyPart::Torso => &mut self.torso,
            BodyPart::LeftArm => &mut self.left_arm,
            BodyPart::RightArm => &mut self.right_arm,
            BodyPart::LeftLeg => &mut self.left_leg,
            BodyPart::RightLeg => &mut self.right_leg,
        }
    }

    /// 特定部位へダメージを与える。
    pub fn hit(&mut self, part: BodyPart, amount: f32, kind: DamageKind) {
        if self.dead || amount <= 0.0 {
            return;
        }
        // 骨折・出血の付与。
        match kind {
            DamageKind::Slashing => self.part_mut(part).bleeding += amount * 0.10,
            DamageKind::Piercing => self.part_mut(part).bleeding += amount * 0.16,
            DamageKind::Blunt => {
                if amount > 8.0 && !part.is_vital() {
                    self.part_mut(part).fractured = true;
                }
            }
            DamageKind::Impact => {
                if matches!(part, BodyPart::LeftLeg | BodyPart::RightLeg) && amount > 6.0 {
                    self.part_mut(part).fractured = true;
                }
            }
            DamageKind::Burn | DamageKind::Toxic => {}
        }

        {
            let p = self.part_mut(part);
            // 包帯は一撃分の衝撃を多少吸収する。
            let absorbed = if p.bandaged { amount * 0.85 } else { amount };
            p.condition = (p.condition - absorbed).max(0.0);
            // 大きな損傷で包帯が破れる。
            if amount > 6.0 {
                p.bandaged = false;
            }
        }

        // 痛みの増加。
        self.pain = (self.pain + amount * 0.02).clamp(0.0, 1.0);

        self.check_death(part);
    }

    /// ランダムな部位への被弾（近接・遠隔の流れ弾）。`roll` は [0,1)。
    pub fn hit_random(&mut self, amount: f32, kind: DamageKind, roll: f32) {
        let total: f32 = BodyPart::ALL.iter().map(|p| p.hit_weight()).sum();
        let mut pick = roll.clamp(0.0, 0.9999) * total;
        for part in BodyPart::ALL {
            let w = part.hit_weight();
            if pick < w {
                self.hit(part, amount, kind);
                return;
            }
            pick -= w;
        }
        self.hit(BodyPart::Torso, amount, kind);
    }

    fn check_death(&mut self, last_hit: BodyPart) {
        if self.dead {
            return;
        }
        // 頭または胴の破壊で即死。
        if self.head.is_broken() {
            self.die("頭部の致命傷");
            return;
        }
        if self.torso.is_broken() {
            self.die("胴体の致命傷");
            return;
        }
        let _ = last_hit;
    }

    fn die(&mut self, cause: &str) {
        self.dead = true;
        self.cause_of_death = Some(cause.to_string());
    }

    /// 時間経過による出血・失血ショック・痛みの減衰を適用する。
    /// `dt_seconds` はゲーム内秒。
    pub fn tick(&mut self, dt_seconds: f32) {
        if self.dead {
            return;
        }
        let mut total_bleed = 0.0;
        for part in BodyPart::ALL {
            let max = self.max_of(part);
            let p = self.part_mut(part);
            if p.bleeding > 0.0 {
                // 包帯を巻いていれば出血は止まっていく。
                if p.bandaged {
                    p.bleeding = (p.bleeding - dt_seconds * 0.5).max(0.0);
                } else {
                    let lost = p.bleeding * dt_seconds;
                    p.condition = (p.condition - lost).max(0.0);
                    total_bleed += p.bleeding;
                    // 血は自然にゆっくり凝固する（ただし手当てより遥かに遅い）。
                    p.bleeding = (p.bleeding - dt_seconds * 0.02).max(0.0);
                }
            }
            // 包帯を巻いた部位はごくゆっくり回復する。
            if p.bandaged && p.bleeding <= 0.0 && !p.fractured {
                p.condition = (p.condition + dt_seconds * 0.08).min(max);
            }
        }

        // 失血ショック。出血の総量に比例して進行する。
        self.blood_loss = (self.blood_loss + total_bleed * dt_seconds * 0.01).clamp(0.0, 1.0);
        // 出血が止まっていれば血は少しずつ戻る。
        if total_bleed <= 0.01 {
            self.blood_loss = (self.blood_loss - dt_seconds * 0.003).max(0.0);
        }
        if self.blood_loss >= 1.0 {
            self.die("失血");
        }

        // 頭・胴の破壊も監視（出血で削れて死ぬ場合）。
        if self.head.is_broken() {
            self.die("頭部の致命傷");
        } else if self.torso.is_broken() {
            self.die("胴体の致命傷");
        }

        // 痛みは時間で薄れる。
        self.pain = (self.pain - dt_seconds * 0.01).max(0.0);
    }

    /// 包帯を巻く。出血を止め、以後ゆっくり回復させる。成功したら true。
    pub fn apply_bandage(&mut self, part: BodyPart) -> bool {
        let max = self.max_of(part);
        let p = self.part_mut(part);
        if p.bandaged || p.condition >= max {
            return false;
        }
        p.bandaged = true;
        p.bleeding = 0.0;
        true
    }

    /// 添え木を当てる。骨折を治療する。
    pub fn apply_splint(&mut self, part: BodyPart) -> bool {
        let p = self.part_mut(part);
        if !p.fractured {
            return false;
        }
        p.fractured = false;
        true
    }

    /// ポーションなどで全身を即座に回復する。
    pub fn heal_all(&mut self, amount: f32) {
        if self.dead {
            return;
        }
        for part in BodyPart::ALL {
            let max = self.max_of(part);
            let p = self.part_mut(part);
            p.condition = (p.condition + amount).min(max);
            p.bleeding = 0.0;
        }
        self.blood_loss = (self.blood_loss - amount * 0.02).max(0.0);
        self.pain = (self.pain - amount * 0.02).max(0.0);
    }

    /// リスポーン等での完全回復。
    pub fn revive(&mut self) {
        let toughness = self.toughness;
        *self = Body::scaled(toughness);
    }

    /// 全身の平均的な健全度 0〜1（HUD の簡易ゲージ用）。
    pub fn overall_fraction(&self) -> f32 {
        let mut cur = 0.0;
        let mut max = 0.0;
        for part in BodyPart::ALL {
            cur += self.part(part).condition;
            max += self.max_of(part);
        }
        let base = if max > 0.0 { cur / max } else { 0.0 };
        (base * (1.0 - self.blood_loss)).clamp(0.0, 1.0)
    }

    /// 移動能力 0〜1。脚の損傷・骨折で落ちる。
    pub fn mobility(&self) -> f32 {
        let leg = |p: &PartState, part: BodyPart| {
            let mut m = (p.condition / self.max_of(part)).clamp(0.0, 1.0);
            if p.fractured {
                m *= 0.3;
            }
            m
        };
        let l = leg(&self.left_leg, BodyPart::LeftLeg);
        let r = leg(&self.right_leg, BodyPart::RightLeg);
        // 片脚でも多少は動ける。
        (0.35 + 0.65 * (l + r) * 0.5) * (1.0 - self.pain * 0.4)
    }

    /// 腕の能力 0〜1。攻撃力・採掘速度に効く。
    pub fn arm_capacity(&self) -> f32 {
        let arm = |p: &PartState, part: BodyPart| {
            let mut a = (p.condition / self.max_of(part)).clamp(0.0, 1.0);
            if p.fractured {
                a *= 0.25;
            }
            a
        };
        let l = arm(&self.left_arm, BodyPart::LeftArm);
        let r = arm(&self.right_arm, BodyPart::RightArm);
        (0.4 + 0.6 * (l + r) * 0.5) * (1.0 - self.pain * 0.3)
    }

    /// 手当てが必要な部位の一覧（HUD の医療画面用）。
    pub fn injuries(&self) -> Vec<(BodyPart, String)> {
        let mut out = Vec::new();
        for part in BodyPart::ALL {
            let p = self.part(part);
            let frac = p.condition / self.max_of(part);
            if p.bleeding > 0.05 {
                out.push((part, format!("{}：出血", part.label())));
            }
            if p.fractured {
                out.push((part, format!("{}：骨折", part.label())));
            }
            if frac < 0.5 && p.bleeding <= 0.05 && !p.fractured {
                out.push((part, format!("{}：損傷 {:.0}%", part.label(), (1.0 - frac) * 100.0)));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_body_is_whole_and_alive() {
        let b = Body::healthy();
        assert!(!b.dead);
        assert!((b.overall_fraction() - 1.0).abs() < 1e-4);
        assert!(b.mobility() > 0.95);
        assert!(b.injuries().is_empty());
    }

    #[test]
    fn destroying_the_head_kills_instantly() {
        let mut b = Body::healthy();
        b.hit(BodyPart::Head, 999.0, DamageKind::Blunt);
        assert!(b.dead);
        assert_eq!(b.cause_of_death.as_deref(), Some("頭部の致命傷"));
    }

    #[test]
    fn destroying_the_torso_kills_instantly() {
        let mut b = Body::healthy();
        b.hit(BodyPart::Torso, 999.0, DamageKind::Piercing);
        assert!(b.dead);
    }

    #[test]
    fn losing_a_limb_is_not_fatal_but_disables_it() {
        let mut b = Body::healthy();
        b.hit(BodyPart::RightArm, 999.0, DamageKind::Slashing);
        assert!(!b.dead, "losing an arm should not kill you");
        assert!(b.right_arm.is_broken());
        assert!(b.arm_capacity() < 0.75, "a destroyed arm should reduce arm capacity");

        b.hit(BodyPart::LeftLeg, 999.0, DamageKind::Impact);
        assert!(!b.dead);
        assert!(b.mobility() < 0.8, "a destroyed leg should slow you down");
    }

    #[test]
    fn bleeding_kills_over_time_if_untreated() {
        let mut b = Body::healthy();
        b.hit(BodyPart::LeftArm, 15.0, DamageKind::Piercing);
        assert!(b.left_arm.bleeding > 0.0);
        // 手当てせずに時間を進めると失血で死ぬ。
        for _ in 0..2000 {
            b.tick(1.0);
            if b.dead {
                break;
            }
        }
        assert!(b.dead, "untreated bleeding should eventually be fatal");
    }

    #[test]
    fn a_bandage_stops_bleeding_and_saves_a_life() {
        let mut b = Body::healthy();
        b.hit(BodyPart::LeftArm, 12.0, DamageKind::Slashing);
        assert!(b.apply_bandage(BodyPart::LeftArm));
        // 包帯を巻けば出血は止まり、死なない。
        for _ in 0..3000 {
            b.tick(1.0);
        }
        assert!(!b.dead, "a bandaged wound should not bleed out");
        assert!(b.left_arm.bleeding <= 0.01);
    }

    #[test]
    fn blunt_trauma_fractures_limbs_and_splints_fix_them() {
        let mut b = Body::healthy();
        b.hit(BodyPart::LeftLeg, 12.0, DamageKind::Blunt);
        assert!(b.left_leg.fractured);
        assert!(b.mobility() < 0.75, "a fracture should hurt mobility");
        assert!(b.apply_splint(BodyPart::LeftLeg));
        assert!(!b.left_leg.fractured);
        assert!(!b.apply_splint(BodyPart::LeftLeg), "splinting a healthy leg does nothing");
    }

    #[test]
    fn natural_recovery_is_negligible_without_treatment() {
        let mut b = Body::healthy();
        b.hit(BodyPart::Torso, 20.0, DamageKind::Blunt);
        let before = b.torso.condition;
        for _ in 0..60 {
            b.tick(1.0);
        }
        // 手当て無しの胴はほとんど回復しない（出血なし・包帯なし）。
        assert!((b.torso.condition - before).abs() < 0.5, "wounds should not self-heal quickly");
    }

    #[test]
    fn a_potion_restores_the_whole_body() {
        let mut b = Body::healthy();
        b.hit(BodyPart::Head, 10.0, DamageKind::Blunt);
        b.hit(BodyPart::LeftLeg, 10.0, DamageKind::Slashing);
        b.heal_all(50.0);
        assert!((b.overall_fraction() - 1.0).abs() < 0.05);
        assert!(b.left_leg.bleeding <= 0.0);
    }

    #[test]
    fn random_hits_respect_the_weighting_but_reach_every_part() {
        let mut seen_head = false;
        let mut seen_torso = false;
        for i in 0..1000 {
            let mut b = Body::healthy();
            let roll = (i as f32 * 0.001) % 1.0;
            b.hit_random(5.0, DamageKind::Slashing, roll);
            for part in BodyPart::ALL {
                if b.part(part).condition < part.max_condition() {
                    if part == BodyPart::Head {
                        seen_head = true;
                    }
                    if part == BodyPart::Torso {
                        seen_torso = true;
                    }
                }
            }
        }
        assert!(seen_head && seen_torso, "random targeting never hit head/torso");
    }

    #[test]
    fn tougher_individuals_survive_more_punishment() {
        let mut weak = Body::scaled(0.6);
        let mut strong = Body::scaled(2.0);
        // 同じ打撃で、弱い個体は先に胴が壊れる。
        for _ in 0..10 {
            weak.hit(BodyPart::Torso, 5.0, DamageKind::Blunt);
            strong.hit(BodyPart::Torso, 5.0, DamageKind::Blunt);
        }
        assert!(weak.dead || weak.torso.condition < strong.torso.condition);
        assert!(!strong.dead, "the tough individual should still be standing");
    }

    #[test]
    fn serialization_round_trips() {
        let mut b = Body::healthy();
        b.hit(BodyPart::LeftArm, 8.0, DamageKind::Slashing);
        b.apply_bandage(BodyPart::LeftArm);
        let json = serde_json::to_string(&b).unwrap();
        let back: Body = serde_json::from_str(&json).unwrap();
        assert_eq!(back.left_arm.bandaged, b.left_arm.bandaged);
        assert_eq!(back.dead, b.dead);
    }
}
