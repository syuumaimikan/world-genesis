//! プレイヤー・村人・野生動物の実体と、その意思決定。
//!
//! NPC の行動は if 文の羅列ではなく、効用（utility）で選ばれる。
//! 「空腹」「疲労」「社交欲」「恐怖」「勤労意欲」といった内部状態と、
//! 時刻・天候・周囲の脅威から各行動のスコアを計算し、最も高いものを実行する。
//! そのため同じ村人でも、日によって、状況によって違う行動を取る。
//!
//! 判断そのものは Bevy に依存しない純関数として書いてあるので、
//! ゲームを起動しなくても単体テストで検証できる。

use crate::blocky::LimbAnimator;
use crate::physics::BodyShape;
use crate::species::{Diet, SpeciesDef, SPECIES};
use bevy::prelude::*;

// ======================================================================
// 時間
// ======================================================================

/// ゲーム内時刻。現実時間とシミュレーション tick を仲介する。
#[derive(Resource, Debug, Clone)]
pub struct WorldTime {
    /// シミュレーション経過 tick（1 tick = 1 ゲーム内秒）。
    pub tick: u64,
    /// 時間倍率。0 で停止。
    pub speed: f32,
    pub paused: bool,
    /// 端数の持ち越し（低速時に時間が進まなくなるのを防ぐ）。
    carry: f64,
}

/// 現実の 20 分 = ゲーム内 1 日。
pub const REAL_SECONDS_PER_GAME_DAY: f64 = 20.0 * 60.0;
pub const GAME_SECONDS_PER_DAY: f64 = 86_400.0;
/// 現実 1 秒あたりに進むゲーム内秒数（等倍時）。
pub const TICKS_PER_REAL_SECOND: f64 = GAME_SECONDS_PER_DAY / REAL_SECONDS_PER_GAME_DAY;

impl Default for WorldTime {
    fn default() -> Self {
        // 朝 7 時から始める。
        Self {
            tick: 7 * 3600,
            speed: 1.0,
            paused: false,
            carry: 0.0,
        }
    }
}

impl WorldTime {
    /// 現実の経過秒を与えて時間を進める。
    pub fn advance(&mut self, real_delta: f32) -> u64 {
        if self.paused || self.speed <= 0.0 {
            return 0;
        }
        self.carry += real_delta as f64 * TICKS_PER_REAL_SECOND * self.speed as f64;
        let whole = self.carry.floor();
        // 極端な倍率でも 1 フレームで進みすぎないよう上限を設ける。
        let stepped = (whole as u64).min(GAME_SECONDS_PER_DAY as u64 * 4);
        self.carry -= whole;
        self.tick = self.tick.saturating_add(stepped);
        stepped
    }

    /// 0.0（真夜中）〜1.0 の一日の進み。
    pub fn day_fraction(&self) -> f32 {
        (self.tick % 86_400) as f32 / 86_400.0
    }

    pub fn hour(&self) -> u32 {
        ((self.tick % 86_400) / 3600) as u32
    }

    pub fn minute(&self) -> u32 {
        ((self.tick % 3600) / 60) as u32
    }

    pub fn day_number(&self) -> u64 {
        self.tick / 86_400
    }

    /// 夜か（就寝・夜行性判定に使う）。
    pub fn is_night(&self) -> bool {
        let h = self.hour();
        !(6..20).contains(&h)
    }

    /// 太陽高度の代用値。-1（真夜中）〜1（正午）。
    pub fn sun_elevation(&self) -> f32 {
        // 6 時に日の出、18 時に日没となる正弦。
        let t = self.day_fraction();
        ((t - 0.25) * std::f32::consts::TAU).sin()
    }
}

// ======================================================================
// 共通コンポーネント
// ======================================================================

/// 物理で動く実体。
#[derive(Component, Debug)]
pub struct Actor {
    pub velocity: Vec3,
    pub shape: BodyShape,
    pub grounded: bool,
    pub in_liquid: bool,
    /// 移動目標（None なら停止）。
    pub move_target: Option<Vec3>,
    /// 移動速度（ブロック/秒）。
    pub speed: f32,
    /// 直近の水平移動量（歩行アニメ用）。
    pub last_speed: f32,
    /// 進行が塞がれた回数。詰まりからの脱出に使う。
    pub stuck_frames: u32,
}

impl Actor {
    pub fn new(shape: BodyShape, speed: f32) -> Self {
        Self {
            velocity: Vec3::ZERO,
            shape,
            grounded: false,
            in_liquid: false,
            move_target: None,
            speed,
            last_speed: 0.0,
            stuck_frames: 0,
        }
    }
}

#[derive(Component, Debug)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            0.0
        } else {
            (self.current / self.max).clamp(0.0, 1.0)
        }
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    pub fn damage(&mut self, amount: f32) {
        self.current = (self.current - amount.max(0.0)).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount.max(0.0)).min(self.max);
    }
}

// ======================================================================
// プレイヤー
// ======================================================================

#[derive(Component)]
pub struct Player {
    pub hunger: f32,
    pub body_temp: f32,
    pub stamina: f32,
    pub money: f64,
    pub profession: String,
    pub reputation: f32,
    /// 生後日数。世界の時間とともに歳を取る。
    pub age_days: f32,
    pub selected_slot: usize,
    pub flying: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            hunger: 100.0,
            body_temp: 36.5,
            stamina: 100.0,
            money: 120.0,
            profession: "放浪者".to_string(),
            reputation: 0.0,
            age_days: 18.0 * 360.0,
            selected_slot: 0,
            flying: false,
        }
    }
}

/// プレイヤーカメラ。視点モードは F5 で巡回する。
#[derive(Component)]
pub struct PlayerCamera {
    pub yaw: f32,
    pub pitch: f32,
    /// 三人称・二人称でプレイヤーから離れる距離。
    pub distance: f32,
    pub perspective: crate::game::Perspective,
}

impl Default for PlayerCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.25,
            distance: 5.0,
            perspective: crate::game::Perspective::First,
        }
    }
}

// ======================================================================
// NPC
// ======================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    Male,
    Female,
}

impl Gender {
    pub fn label(self) -> &'static str {
        match self {
            Gender::Male => "男性",
            Gender::Female => "女性",
        }
    }
}

/// 性格。行動の効用計算に重みとして効く。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Personality {
    Brave,
    Cautious,
    Diligent,
    Sociable,
    Gruff,
}

impl Personality {
    pub fn label(self) -> &'static str {
        match self {
            Personality::Brave => "勇敢",
            Personality::Cautious => "慎重",
            Personality::Diligent => "勤勉",
            Personality::Sociable => "社交的",
            Personality::Gruff => "気難しい",
        }
    }

    pub fn from_hash(h: u64) -> Self {
        match h % 5 {
            0 => Personality::Brave,
            1 => Personality::Cautious,
            2 => Personality::Diligent,
            3 => Personality::Sociable,
            _ => Personality::Gruff,
        }
    }

    /// 危険に立ち向かう度合い 0.0〜1.0。
    pub fn courage(self) -> f32 {
        match self {
            Personality::Brave => 0.95,
            Personality::Gruff => 0.6,
            Personality::Diligent => 0.45,
            Personality::Sociable => 0.35,
            Personality::Cautious => 0.1,
        }
    }

    pub fn sociability(self) -> f32 {
        match self {
            Personality::Sociable => 1.0,
            Personality::Brave => 0.6,
            Personality::Diligent => 0.4,
            Personality::Cautious => 0.4,
            Personality::Gruff => 0.15,
        }
    }

    pub fn work_ethic(self) -> f32 {
        match self {
            Personality::Diligent => 1.0,
            Personality::Gruff => 0.7,
            Personality::Cautious => 0.6,
            Personality::Brave => 0.55,
            Personality::Sociable => 0.45,
        }
    }
}

/// NPC が今取っている行動。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Activity {
    Sleep,
    Eat,
    Work,
    Socialize,
    FetchWater,
    GoHome,
    Flee,
    Fight,
    Wander,
}

impl Activity {
    pub fn label(self) -> &'static str {
        match self {
            Activity::Sleep => "就寝中",
            Activity::Eat => "食事中",
            Activity::Work => "仕事中",
            Activity::Socialize => "談笑中",
            Activity::FetchWater => "水汲み",
            Activity::GoHome => "帰宅中",
            Activity::Flee => "逃走中",
            Activity::Fight => "戦闘中",
            Activity::Wander => "散策中",
        }
    }
}

#[derive(Component)]
pub struct Npc {
    pub name: String,
    pub gender: Gender,
    pub personality: Personality,
    pub profession: String,
    /// 年齢（年）。
    pub age: u32,
    pub village_id: u64,
    pub village_name: String,
    /// 自宅（玄関前）の座標。
    pub home: Vec3,
    /// 職場の座標。
    pub workplace: Vec3,
    /// 井戸・水場。
    pub water: Vec3,
    /// 広場（社交の場）。
    pub plaza: Vec3,

    // --- 内部状態 ---
    pub hunger: f32,
    pub fatigue: f32,
    pub social: f32,
    pub fear: f32,
    pub activity: Activity,
    /// 行動の再評価までの残り時間（秒）。毎フレーム決め直さない。
    pub decision_cooldown: f32,
    /// 攻撃のクールダウン。
    pub attack_cooldown: f32,
    /// 記憶。会話で語られる。
    pub memories: Vec<String>,
}

impl Npc {
    /// 武器を取って戦えるか（衛兵・鍛冶屋・勇敢な大人）。
    pub fn can_fight(&self) -> bool {
        if self.age < 15 || self.age > 65 {
            return false;
        }
        matches!(self.profession.as_str(), "衛兵" | "鍛冶屋" | "鉱夫" | "牧夫")
            || self.personality.courage() >= 0.6
    }

    pub fn remember(&mut self, event: impl Into<String>) {
        self.memories.push(event.into());
        // 記憶は無限には持たない。古いものから忘れる。
        if self.memories.len() > 12 {
            self.memories.remove(0);
        }
    }
}

/// 村人の生成に必要な情報（`village::VillagePlan` から作る）。
pub struct NpcSeed {
    pub id: u64,
    pub profession: &'static str,
    pub home: Vec3,
    pub workplace: Vec3,
    pub village_id: u64,
    pub village_name: String,
}

const GIVEN_NAMES_M: [&str; 16] = [
    "ガルク", "ロルフ", "ハラルド", "エドウィン", "トーマス", "ベルン", "カイル", "オズワルド",
    "ヨナス", "アルベル", "ドミニク", "ラース", "セヴァン", "ミロス", "アントン", "フィン",
];
const GIVEN_NAMES_F: [&str; 16] = [
    "エレナ", "ミア", "ヘルガ", "アニカ", "ルーシー", "イングリッド", "マリカ", "ソフィア",
    "テア", "ノラ", "ベアタ", "リナ", "ヨハンナ", "クララ", "エルサ", "ダリア",
];
const FAMILY_NAMES: [&str; 16] = [
    "石割", "麦畑", "北風", "灰塚", "川辺", "鉄槌", "白樺", "夜明け",
    "石橋", "羊飼い", "遠見", "泥土", "薪割り", "峠", "潮見", "陽だまり",
];

/// 決定論的に名前を作る。同じ ID なら常に同じ人物。
pub fn generate_name(id: u64, gender: Gender) -> String {
    let h = crate::noise::hash_u64(id ^ 0x4E41_4D45);
    let given = match gender {
        Gender::Male => GIVEN_NAMES_M[(h % GIVEN_NAMES_M.len() as u64) as usize],
        Gender::Female => GIVEN_NAMES_F[(h % GIVEN_NAMES_F.len() as u64) as usize],
    };
    let family = FAMILY_NAMES[((h >> 20) % FAMILY_NAMES.len() as u64) as usize];
    format!("{family}の{given}")
}

// ======================================================================
// NPC の意思決定（純関数）
// ======================================================================

/// 効用計算の入力。
#[derive(Debug, Clone, Copy)]
pub struct NpcContext {
    pub hour: u32,
    pub hunger: f32,
    pub fatigue: f32,
    pub social: f32,
    /// 最も近い脅威までの距離（脅威が無ければ f32::INFINITY）。
    pub threat_distance: f32,
    pub courage: f32,
    pub sociability: f32,
    pub work_ethic: f32,
    pub can_fight: bool,
    /// 体力の割合。
    pub health_fraction: f32,
}

/// 各行動の効用を計算し、最も高いものを返す。
///
/// 「夜になったら寝る」ではなく「疲労と時刻から睡眠の効用が上がる」。
/// そのため疲れていなければ夜更かしするし、飢えていれば夜中でも食べに行く。
pub fn choose_activity(ctx: &NpcContext) -> Activity {
    // (行動, 効用) を積み上げ、最後に最大のものを選ぶ。
    // 何もすることが無ければ散策する。
    let mut scores: Vec<(Activity, f32)> = vec![(Activity::Wander, 0.15)];
    macro_rules! consider {
        ($activity:expr, $score:expr) => {
            scores.push(($activity, $score));
        };
    }

    // --- 生存が最優先 ---
    if ctx.threat_distance < 18.0 {
        let proximity = 1.0 - (ctx.threat_distance / 18.0).clamp(0.0, 1.0);
        // 追い詰められるほど強く反応する。
        let urgency = 1.2 + proximity * 1.6;
        if ctx.can_fight && ctx.courage > 0.5 && ctx.health_fraction > 0.35 {
            consider!(Activity::Fight, urgency * ctx.courage);
        }
        // 逃走は「勇気で打ち消される」のではなく「勇気で割り引かれる」。
        // 中庸な性格の村人でも、目の前の捕食者を無視して眠り込んだりはしない。
        // 傷ついているほど勇気は当てにならず、逃走の効用が跳ね上がる。
        consider!(Activity::Flee, urgency * (1.55 - ctx.courage * ctx.health_fraction));
    }

    // --- 空腹 ---
    // 満腹（100）で 0、空腹（0）で 1.4。食事時はさらに上乗せ。
    let hunger_need = (1.0 - ctx.hunger / 100.0).clamp(0.0, 1.0);
    let meal_time = matches!(ctx.hour, 7 | 8 | 12 | 13 | 18 | 19);
    consider!(Activity::Eat, hunger_need * 1.4 + if meal_time { 0.35 } else { 0.0 });

    // --- 睡眠 ---
    let night = !(6..21).contains(&ctx.hour);
    let fatigue_need = (ctx.fatigue / 100.0).clamp(0.0, 1.0);
    consider!(
        Activity::Sleep,
        fatigue_need * 1.1 + if night { 0.65 } else { -0.35 }
    );

    // --- 労働 ---
    let work_hours = (8..18).contains(&ctx.hour);
    if work_hours {
        // 空腹・疲労が強いと仕事の効用は落ちる。
        let fitness = (1.0 - hunger_need * 0.7) * (1.0 - fatigue_need * 0.6);
        consider!(Activity::Work, 0.85 * ctx.work_ethic * fitness.max(0.0));
    }

    // --- 社交 ---
    let social_hours = (17..22).contains(&ctx.hour);
    let social_need = (ctx.social / 100.0).clamp(0.0, 1.0);
    consider!(
        Activity::Socialize,
        social_need * ctx.sociability * if social_hours { 1.0 } else { 0.35 }
    );

    // --- 水汲み ---
    // 家事として朝と夕方に発生する。
    if matches!(ctx.hour, 6 | 7 | 16 | 17) {
        consider!(Activity::FetchWater, 0.45);
    }

    // --- 帰宅 ---
    // 夜は、他に強い動機が無ければ家へ戻る。
    if night {
        consider!(Activity::GoHome, 0.55);
    }

    // 同点のときは列挙順（危険 → 生理 → 労働 → 社交）で先に積んだ方が勝つ。
    let mut best = Activity::Wander;
    let mut best_score = f32::NEG_INFINITY;
    for (activity, score) in scores {
        if score > best_score {
            best_score = score;
            best = activity;
        }
    }
    best
}

/// 行動から目的地を決める。
pub fn activity_target(npc: &Npc, activity: Activity, threat: Option<Vec3>, self_pos: Vec3) -> Option<Vec3> {
    match activity {
        Activity::Sleep | Activity::GoHome | Activity::Eat => Some(npc.home),
        Activity::Work => Some(npc.workplace),
        Activity::Socialize => Some(npc.plaza),
        Activity::FetchWater => Some(npc.water),
        Activity::Fight => threat,
        Activity::Flee => {
            // 脅威の反対側、家の方向へ寄せて逃げる。
            let away = threat
                .map(|t| (self_pos - t).normalize_or_zero())
                .unwrap_or(Vec3::Z);
            let home_dir = (npc.home - self_pos).normalize_or_zero();
            let dir = (away * 2.0 + home_dir).normalize_or_zero();
            Some(self_pos + dir * 14.0)
        }
        Activity::Wander => None,
    }
}

// ======================================================================
// 野生動物
// ======================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaunaState {
    Graze,
    Wander,
    Flee,
    Hunt,
    Drink,
    Rest,
}

impl FaunaState {
    pub fn label(self) -> &'static str {
        match self {
            FaunaState::Graze => "採食",
            FaunaState::Wander => "移動",
            FaunaState::Flee => "逃走",
            FaunaState::Hunt => "狩り",
            FaunaState::Drink => "水飲み",
            FaunaState::Rest => "休息",
        }
    }
}

#[derive(Component)]
pub struct Wildlife {
    /// `species::SPECIES` の添字。
    pub species: usize,
    pub hunger: f32,
    pub energy: f32,
    pub state: FaunaState,
    pub decision_cooldown: f32,
    pub attack_cooldown: f32,
    /// 群れの中心（生まれた場所）。ここから離れすぎない。
    pub home_anchor: Vec3,
}

impl Wildlife {
    pub fn def(&self) -> &'static SpeciesDef {
        &SPECIES[self.species.min(SPECIES.len() - 1)]
    }
}

/// 動物の意思決定に必要な状況。
#[derive(Debug, Clone, Copy)]
pub struct FaunaContext {
    pub hunger: f32,
    pub energy: f32,
    /// 最も近い脅威（捕食者・人間）までの距離。
    pub threat_distance: f32,
    /// 最も近い獲物までの距離（捕食者のみ意味を持つ）。
    pub prey_distance: f32,
    pub is_night: bool,
    pub nocturnal: bool,
    pub flee_distance: f32,
    pub is_predator: bool,
    pub health_fraction: f32,
}

pub fn choose_fauna_state(ctx: &FaunaContext) -> FaunaState {
    // 1. 逃げる — 恐れる相手が間合いに入ったら他の全てに優先する。
    if ctx.flee_distance > 0.0 && ctx.threat_distance < ctx.flee_distance {
        return FaunaState::Flee;
    }
    // 傷ついた個体は、逃げ足の距離に関係なく退く。
    if ctx.health_fraction < 0.35 && ctx.threat_distance < 24.0 {
        return FaunaState::Flee;
    }

    // 2. 狩る — 捕食者が空腹で、獲物が射程にいるとき。
    if ctx.is_predator && ctx.hunger > 45.0 && ctx.prey_distance < 34.0 {
        return FaunaState::Hunt;
    }

    // 3. 休む — 活動時間帯でなく、脅威も無く、体力が減っているとき。
    let resting_hours = ctx.is_night != ctx.nocturnal;
    if resting_hours && ctx.energy < 55.0 {
        return FaunaState::Rest;
    }

    // 4. 食べる — 空腹なら採食。
    if ctx.hunger > 40.0 {
        return FaunaState::Graze;
    }

    // 5. 水を飲む — 満腹だが喉が渇いている（エネルギーが十分でないとき）。
    if ctx.energy < 70.0 && !resting_hours {
        return FaunaState::Drink;
    }

    FaunaState::Wander
}

/// 動物の体格から当たり判定を作る。
pub fn shape_for_species(sp: &SpeciesDef) -> BodyShape {
    BodyShape {
        half_width: (sp.length * 0.30).clamp(0.12, 1.4),
        height: sp.height.clamp(0.25, 4.0),
        // 体高の半分までの段差は自力で越えられる。
        step_height: (sp.height * 0.55).clamp(0.3, 1.2),
    }
}

/// 捕食者にとっての獲物か。
pub fn is_prey_of(predator: &SpeciesDef, other: &SpeciesDef) -> bool {
    if !predator.is_predator() {
        return false;
    }
    if predator.key == other.key {
        return false;
    }
    // 自分より大きすぎる相手は狙わない。
    if other.max_health > predator.max_health * 1.4 {
        return false;
    }
    matches!(other.diet, Diet::Herbivore | Diet::Filter) || other.max_health < predator.max_health * 0.6
}

// ======================================================================
// 共通の見た目コンポーネント
// ======================================================================

/// 頭上に名前を出す対象。
#[derive(Component)]
pub struct Nameplate {
    pub text: String,
    pub color: Color,
}

/// 死亡して消滅するまでの猶予。
#[derive(Component)]
pub struct Dying {
    pub timer: f32,
}

/// 飛び道具。
#[derive(Component)]
pub struct Projectile {
    pub velocity: Vec3,
    pub lifetime: f32,
    pub damage: f32,
    /// 撃った本人（自分に当たらないように）。
    pub owner: Option<Entity>,
}

/// 歩行速度をアニメーターへ流し込む。
pub fn sync_limb_animation(mut query: Query<(&Actor, &mut LimbAnimator)>) {
    for (actor, mut anim) in query.iter_mut() {
        anim.move_speed = actor.last_speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> NpcContext {
        NpcContext {
            hour: 10,
            hunger: 100.0,
            fatigue: 0.0,
            social: 0.0,
            threat_distance: f32::INFINITY,
            courage: 0.5,
            sociability: 0.5,
            work_ethic: 0.8,
            can_fight: false,
            health_fraction: 1.0,
        }
    }

    // ---------- 時間 ----------

    #[test]
    fn a_full_game_day_takes_twenty_real_minutes() {
        let mut t = WorldTime::default();
        t.tick = 0;
        // 20 分ぶんを 1/60 秒刻みで進める。
        for _ in 0..(20 * 60 * 60) {
            t.advance(1.0 / 60.0);
        }
        let days = t.tick as f64 / GAME_SECONDS_PER_DAY;
        assert!((days - 1.0).abs() < 0.01, "one real 20-minute session covered {days} game days");
    }

    #[test]
    fn slow_speeds_still_advance_time() {
        let mut t = WorldTime::default();
        t.speed = 0.05;
        let start = t.tick;
        // 端数を切り捨て続けると時間が止まってしまう。持ち越しで防いでいる。
        for _ in 0..600 {
            t.advance(1.0 / 60.0);
        }
        assert!(t.tick > start, "time stood still at low speed");
    }

    #[test]
    fn pausing_freezes_the_clock() {
        let mut t = WorldTime::default();
        t.paused = true;
        let start = t.tick;
        for _ in 0..100 {
            t.advance(1.0);
        }
        assert_eq!(t.tick, start);
    }

    #[test]
    fn day_night_maps_to_sensible_hours() {
        let mut t = WorldTime::default();
        t.tick = 12 * 3600;
        assert_eq!(t.hour(), 12);
        assert!(!t.is_night());
        assert!(t.sun_elevation() > 0.9, "noon sun should be overhead");

        t.tick = 2 * 3600;
        assert!(t.is_night());
        assert!(t.sun_elevation() < 0.0, "the sun should be below the horizon at 02:00");

        t.tick = 86_400 * 3 + 22 * 3600;
        assert_eq!(t.day_number(), 3);
        assert_eq!(t.hour(), 22);
        assert!(t.is_night());
    }

    #[test]
    fn extreme_speed_cannot_blow_up_the_clock() {
        let mut t = WorldTime::default();
        t.speed = 100_000.0;
        let before = t.tick;
        t.advance(1.0);
        let jumped = t.tick - before;
        assert!(jumped <= GAME_SECONDS_PER_DAY as u64 * 4, "clock jumped {jumped} ticks in one frame");
        assert!(jumped > 0);
    }

    // ---------- NPC の意思決定 ----------

    #[test]
    fn a_rested_fed_villager_works_during_the_day() {
        let c = ctx();
        assert_eq!(choose_activity(&c), Activity::Work);
    }

    #[test]
    fn hunger_overrides_work() {
        let mut c = ctx();
        c.hunger = 5.0;
        assert_eq!(choose_activity(&c), Activity::Eat);
    }

    #[test]
    fn a_tired_villager_sleeps_at_night() {
        let mut c = ctx();
        c.hour = 23;
        c.fatigue = 85.0;
        assert_eq!(choose_activity(&c), Activity::Sleep);
    }

    #[test]
    fn a_wide_awake_villager_does_not_sleep_at_noon() {
        let mut c = ctx();
        c.hour = 12;
        c.fatigue = 20.0;
        assert_ne!(choose_activity(&c), Activity::Sleep);
    }

    #[test]
    fn the_brave_fight_and_the_cautious_flee() {
        let mut c = ctx();
        c.threat_distance = 6.0;

        c.courage = 0.95;
        c.can_fight = true;
        assert_eq!(choose_activity(&c), Activity::Fight, "a brave guard should stand and fight");

        c.courage = 0.1;
        c.can_fight = false;
        assert_eq!(choose_activity(&c), Activity::Flee, "a frightened villager should run");
    }

    #[test]
    fn even_the_brave_flee_when_nearly_dead() {
        let mut c = ctx();
        c.threat_distance = 4.0;
        c.courage = 0.95;
        c.can_fight = true;
        c.health_fraction = 0.15;
        assert_eq!(choose_activity(&c), Activity::Flee);
    }

    #[test]
    fn danger_beats_hunger_and_sleep() {
        let mut c = ctx();
        c.hour = 2;
        c.hunger = 0.0;
        c.fatigue = 100.0;
        c.threat_distance = 3.0;
        let a = choose_activity(&c);
        assert!(matches!(a, Activity::Flee | Activity::Fight), "the villager ignored a predator: {a:?}");
    }

    #[test]
    fn a_lonely_sociable_villager_goes_to_the_square_in_the_evening() {
        let mut c = ctx();
        c.hour = 19;
        c.social = 100.0;
        c.sociability = 1.0;
        c.work_ethic = 0.2;
        assert_eq!(choose_activity(&c), Activity::Socialize);
    }

    #[test]
    fn personality_actually_changes_behaviour() {
        // 同じ状況でも性格が違えば結論が変わることを確かめる。
        let mut diligent = ctx();
        diligent.hour = 19;
        diligent.social = 70.0;
        diligent.work_ethic = Personality::Diligent.work_ethic();
        diligent.sociability = Personality::Diligent.sociability();

        let mut sociable = diligent;
        sociable.work_ethic = Personality::Sociable.work_ethic();
        sociable.sociability = Personality::Sociable.sociability();

        assert_ne!(
            choose_activity(&diligent),
            choose_activity(&sociable),
            "diligent and sociable villagers behaved identically"
        );
    }

    #[test]
    fn every_activity_is_reachable_from_some_situation() {
        // どの行動も「絶対に選ばれない死に枝」になっていないこと。
        let mut seen = std::collections::HashSet::new();
        for hour in 0..24u32 {
            for hunger in [0.0f32, 50.0, 100.0] {
                for fatigue in [0.0f32, 50.0, 100.0] {
                    for social in [0.0f32, 100.0] {
                        for threat in [f32::INFINITY, 5.0] {
                            for courage in [0.1f32, 0.95] {
                                let c = NpcContext {
                                    hour,
                                    hunger,
                                    fatigue,
                                    social,
                                    threat_distance: threat,
                                    courage,
                                    sociability: 1.0,
                                    work_ethic: 1.0,
                                    can_fight: courage > 0.5,
                                    health_fraction: 1.0,
                                };
                                seen.insert(choose_activity(&c));
                            }
                        }
                    }
                }
            }
        }
        for expected in [
            Activity::Sleep, Activity::Eat, Activity::Work,
            Activity::Socialize, Activity::Flee, Activity::Fight,
        ] {
            assert!(seen.contains(&expected), "{expected:?} is unreachable");
        }
    }

    #[test]
    fn flee_target_moves_away_from_the_threat() {
        let npc = test_npc();
        let self_pos = Vec3::new(0.0, 64.0, 0.0);
        let threat = Vec3::new(5.0, 64.0, 0.0);
        let target = activity_target(&npc, Activity::Flee, Some(threat), self_pos).unwrap();
        // 逃走先は脅威より遠くなければならない。
        assert!(
            target.distance(threat) > self_pos.distance(threat),
            "the flee target is closer to the predator"
        );
    }

    fn test_npc() -> Npc {
        Npc {
            name: "テスト".into(),
            gender: Gender::Male,
            personality: Personality::Brave,
            profession: "農民".into(),
            age: 30,
            village_id: 1,
            village_name: "テスト村".into(),
            home: Vec3::new(-20.0, 64.0, 0.0),
            workplace: Vec3::new(10.0, 64.0, 10.0),
            water: Vec3::new(0.0, 64.0, 8.0),
            plaza: Vec3::new(0.0, 64.0, 0.0),
            hunger: 100.0,
            fatigue: 0.0,
            social: 0.0,
            fear: 0.0,
            activity: Activity::Wander,
            decision_cooldown: 0.0,
            attack_cooldown: 0.0,
            memories: Vec::new(),
        }
    }

    #[test]
    fn activity_targets_point_at_the_right_places() {
        let npc = test_npc();
        let p = Vec3::ZERO;
        assert_eq!(activity_target(&npc, Activity::Sleep, None, p), Some(npc.home));
        assert_eq!(activity_target(&npc, Activity::Work, None, p), Some(npc.workplace));
        assert_eq!(activity_target(&npc, Activity::FetchWater, None, p), Some(npc.water));
        assert_eq!(activity_target(&npc, Activity::Socialize, None, p), Some(npc.plaza));
        assert_eq!(activity_target(&npc, Activity::Wander, None, p), None);
    }

    #[test]
    fn memories_are_bounded() {
        let mut npc = test_npc();
        for i in 0..50 {
            npc.remember(format!("出来事 {i}"));
        }
        assert!(npc.memories.len() <= 12, "memory grew without bound");
        assert_eq!(npc.memories.last().unwrap(), "出来事 49", "the newest memory was lost");
    }

    #[test]
    fn guards_fight_but_children_never_do() {
        let mut guard = test_npc();
        guard.profession = "衛兵".into();
        assert!(guard.can_fight());

        let mut child = test_npc();
        child.age = 9;
        child.profession = "衛兵".into();
        assert!(!child.can_fight(), "a child must never be sent into combat");

        let mut timid = test_npc();
        timid.personality = Personality::Cautious;
        timid.profession = "パン職人".into();
        assert!(!timid.can_fight());
    }

    #[test]
    fn names_are_deterministic_and_gendered() {
        assert_eq!(generate_name(7, Gender::Male), generate_name(7, Gender::Male));
        let male = generate_name(7, Gender::Male);
        let female = generate_name(7, Gender::Female);
        assert_ne!(male, female);
        assert!(GIVEN_NAMES_M.iter().any(|n| male.contains(n)));
        assert!(GIVEN_NAMES_F.iter().any(|n| female.contains(n)));
    }

    // ---------- 動物の意思決定 ----------

    fn fauna_ctx() -> FaunaContext {
        FaunaContext {
            hunger: 20.0,
            energy: 100.0,
            threat_distance: f32::INFINITY,
            prey_distance: f32::INFINITY,
            is_night: false,
            nocturnal: false,
            flee_distance: 12.0,
            is_predator: false,
            health_fraction: 1.0,
        }
    }

    #[test]
    fn prey_flees_when_a_predator_closes_in() {
        let mut c = fauna_ctx();
        c.threat_distance = 6.0;
        assert_eq!(choose_fauna_state(&c), FaunaState::Flee);
        // 遠ければ逃げない。
        c.threat_distance = 40.0;
        assert_ne!(choose_fauna_state(&c), FaunaState::Flee);
    }

    #[test]
    fn a_hungry_predator_hunts() {
        let mut c = fauna_ctx();
        c.is_predator = true;
        c.flee_distance = 0.0;
        c.hunger = 80.0;
        c.prey_distance = 20.0;
        assert_eq!(choose_fauna_state(&c), FaunaState::Hunt);
    }

    #[test]
    fn a_full_predator_leaves_prey_alone() {
        let mut c = fauna_ctx();
        c.is_predator = true;
        c.flee_distance = 0.0;
        c.hunger = 10.0;
        c.prey_distance = 5.0;
        assert_ne!(choose_fauna_state(&c), FaunaState::Hunt, "a sated predator kept hunting");
    }

    #[test]
    fn nocturnal_and_diurnal_animals_rest_at_opposite_times() {
        let mut day_animal = fauna_ctx();
        day_animal.energy = 30.0;
        day_animal.is_night = true;
        day_animal.nocturnal = false;
        assert_eq!(choose_fauna_state(&day_animal), FaunaState::Rest);

        let mut night_animal = day_animal;
        night_animal.nocturnal = true;
        assert_ne!(choose_fauna_state(&night_animal), FaunaState::Rest, "a nocturnal animal slept at night");
    }

    #[test]
    fn a_wounded_animal_retreats_even_if_brave_ranged() {
        let mut c = fauna_ctx();
        c.flee_distance = 0.0; // 人を恐れない獣
        c.health_fraction = 0.2;
        c.threat_distance = 10.0;
        assert_eq!(choose_fauna_state(&c), FaunaState::Flee);
    }

    #[test]
    fn hungry_herbivores_graze() {
        let mut c = fauna_ctx();
        c.hunger = 70.0;
        assert_eq!(choose_fauna_state(&c), FaunaState::Graze);
    }

    #[test]
    fn predation_targets_are_plausible() {
        let wolf = crate::species::species_by_key("wolf").unwrap();
        let deer = crate::species::species_by_key("deer").unwrap();
        let elephant = crate::species::species_by_key("elephant").unwrap();
        let other_wolf = crate::species::species_by_key("wolf").unwrap();
        let cow = crate::species::species_by_key("cow").unwrap();

        assert!(is_prey_of(wolf, deer), "wolves should hunt deer");
        assert!(is_prey_of(wolf, cow));
        assert!(!is_prey_of(wolf, elephant), "a wolf should not attack an elephant");
        assert!(!is_prey_of(wolf, other_wolf), "wolves should not hunt their own kind");
        assert!(!is_prey_of(deer, wolf), "a deer is not a predator");
    }

    #[test]
    fn every_species_gets_a_usable_collision_shape() {
        for sp in SPECIES {
            let s = shape_for_species(sp);
            assert!(s.half_width > 0.0 && s.half_width.is_finite(), "{}", sp.key);
            assert!(s.height > 0.0 && s.height.is_finite(), "{}", sp.key);
            assert!(s.step_height > 0.0, "{}", sp.key);
        }
    }

    #[test]
    fn health_clamps_at_both_ends() {
        let mut h = Health::new(50.0);
        h.damage(80.0);
        assert_eq!(h.current, 0.0);
        assert!(h.is_dead());
        assert_eq!(h.fraction(), 0.0);
        h.heal(999.0);
        assert_eq!(h.current, 50.0);
        assert_eq!(h.fraction(), 1.0);
        // 負のダメージで回復してしまわないこと。
        h.damage(-10.0);
        assert_eq!(h.current, 50.0);
    }
}
