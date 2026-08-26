//! 病気システム。
//!
//! 病はランダムに発生するのではなく、病原体（`Pathogen`）が宿主から宿主へ
//! 感染して広がる。感染確率は距離・宿主の免疫・病原体の感染力から決まる。
//! 種によって罹りやすい病が違う——狼や犬は狂犬病に、鳥は鳥インフルに、
//! 人間は風邪や疫病に。人獣共通感染症（狂犬病など）は種を越えて伝播する。
//!
//! Bevy に依存しない純関数として書いてあり、感染判定も進行も単体テスト可能。

use serde::{Deserialize, Serialize};

/// 感染しうる宿主の種別区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostClass {
    Human,
    Canine,
    Bird,
    Livestock,
    WildMammal,
    Reptile,
    Fish,
    Insect,
}

impl HostClass {
    /// 動物種のキーから宿主区分を判定する。
    pub fn of_species(key: &str) -> Self {
        match key {
            "wolf" | "fox" | "jackal" | "lynx" => HostClass::Canine,
            "cow" | "sheep" | "horse" | "pig" | "goat" | "mooshroom" => HostClass::Livestock,
            "eagle" | "vulture" | "owl" | "parrot" | "ostrich" | "heron" | "chicken" => HostClass::Bird,
            "cod" | "salmon" | "squid" => HostClass::Fish,
            "crocodile" | "python" | "salamander" | "turtle" | "frog" => HostClass::Reptile,
            "scorpion" | "crab" | "bee" => HostClass::Insect,
            _ => HostClass::WildMammal,
        }
    }
}

/// 感染経路。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transmission {
    /// 空気感染（近くにいるだけで移る）。
    Airborne,
    /// 接触・咬傷（近接時のみ）。
    Contact,
    /// 水系（同じ水場）。
    Waterborne,
    /// 媒介昆虫（蚊など）。
    Vector,
}

/// 病原体の定義。
#[derive(Debug, Clone)]
pub struct Pathogen {
    pub id: &'static str,
    pub name: &'static str,
    pub transmission: Transmission,
    /// 基礎感染力（1接触あたりの確率係数）。
    pub contagion: f32,
    /// 感染範囲（ブロック）。
    pub range: f32,
    /// 症状が出るまでの潜伏期間（ゲーム内時間・時）。
    pub incubation_hours: f32,
    /// 発症後、致死判定に至るまでの進行速度（毎時）。
    pub virulence: f32,
    /// 致死率（進行が最大に達したときの死亡確率係数）。
    pub lethality: f32,
    /// 感染しうる宿主区分。
    pub hosts: &'static [HostClass],
    /// 回復後に免疫を得るか。
    pub confers_immunity: bool,
    /// 症状による移動・行動の低下（0〜1）。
    pub debilitation: f32,
}

/// 世界に存在する病の一覧。プラグインで追加も可能（`extra_pathogens`）。
pub const PATHOGENS: &[Pathogen] = &[
    Pathogen {
        id: "common_cold",
        name: "風邪",
        transmission: Transmission::Airborne,
        contagion: 0.35,
        range: 3.0,
        incubation_hours: 12.0,
        virulence: 0.02,
        lethality: 0.02,
        hosts: &[HostClass::Human],
        confers_immunity: false,
        debilitation: 0.2,
    },
    Pathogen {
        id: "influenza",
        name: "流行り病（インフルエンザ）",
        transmission: Transmission::Airborne,
        contagion: 0.5,
        range: 4.0,
        incubation_hours: 24.0,
        virulence: 0.05,
        lethality: 0.12,
        hosts: &[HostClass::Human],
        confers_immunity: true,
        debilitation: 0.4,
    },
    Pathogen {
        id: "plague",
        name: "疫病",
        transmission: Transmission::Vector,
        contagion: 0.6,
        range: 5.0,
        incubation_hours: 36.0,
        virulence: 0.06,
        lethality: 0.55,
        hosts: &[HostClass::Human, HostClass::WildMammal],
        confers_immunity: true,
        debilitation: 0.6,
    },
    Pathogen {
        id: "rabies",
        name: "狂犬病",
        transmission: Transmission::Contact,
        contagion: 0.8,
        range: 1.6,
        incubation_hours: 72.0,
        virulence: 0.04,
        lethality: 0.95,
        // 人獣共通感染症：犬・野生哺乳類から人へも移る。
        hosts: &[HostClass::Canine, HostClass::WildMammal, HostClass::Human, HostClass::Livestock],
        confers_immunity: false,
        debilitation: 0.5,
    },
    Pathogen {
        id: "avian_flu",
        name: "鳥インフルエンザ",
        transmission: Transmission::Contact,
        contagion: 0.55,
        range: 3.0,
        incubation_hours: 20.0,
        virulence: 0.07,
        lethality: 0.5,
        // 主に鳥だが、まれに家畜・人へ。
        hosts: &[HostClass::Bird, HostClass::Livestock, HostClass::Human],
        confers_immunity: true,
        debilitation: 0.4,
    },
    Pathogen {
        id: "murrain",
        name: "獣疫",
        transmission: Transmission::Contact,
        contagion: 0.5,
        range: 3.5,
        incubation_hours: 30.0,
        virulence: 0.05,
        lethality: 0.4,
        hosts: &[HostClass::Livestock, HostClass::WildMammal],
        confers_immunity: true,
        debilitation: 0.5,
    },
    Pathogen {
        id: "dysentery",
        name: "赤痢",
        transmission: Transmission::Waterborne,
        contagion: 0.45,
        range: 2.0,
        incubation_hours: 18.0,
        virulence: 0.04,
        lethality: 0.2,
        hosts: &[HostClass::Human, HostClass::Livestock],
        confers_immunity: false,
        debilitation: 0.35,
    },
];

pub fn pathogen(id: &str) -> Option<&'static Pathogen> {
    PATHOGENS.iter().find(|p| p.id == id)
}

/// 病原体がこの宿主区分に感染しうるか。
pub fn can_infect(p: &Pathogen, host: HostClass) -> bool {
    p.hosts.contains(&host)
}

/// 感染の段階。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// 潜伏中（無症状だが感染源にはなる）。
    Incubating,
    /// 発症（症状あり、進行する）。
    Symptomatic,
    /// 回復（免疫の有無は病原体次第）。
    Recovered,
}

/// 宿主の感染状態。
#[derive(Debug, Clone)]
pub struct Infection {
    pub pathogen_id: &'static str,
    pub stage: Stage,
    /// 潜伏または発症からの経過（時）。
    pub elapsed_hours: f32,
    /// 病状の進行度 0〜1（1 で致死判定）。
    pub severity: f32,
}

/// 宿主 1 体の免疫・感染管理。
#[derive(Debug, Clone, Default)]
pub struct ImmuneSystem {
    /// 免疫力（0.0〜1.0）。高いほど感染しにくく、回復も速い。
    pub immunity: f32,
    /// 過去に罹って免疫を得た病。
    pub immune_to: Vec<&'static str>,
    /// 現在の感染（複数の病に同時に罹りうる）。
    pub infections: Vec<Infection>,
}

impl ImmuneSystem {
    pub fn new(immunity: f32) -> Self {
        Self {
            immunity: immunity.clamp(0.0, 1.0),
            immune_to: Vec::new(),
            infections: Vec::new(),
        }
    }

    pub fn is_immune(&self, id: &str) -> bool {
        self.immune_to.contains(&id)
    }

    pub fn is_infected_with(&self, id: &str) -> bool {
        self.infections.iter().any(|i| i.pathogen_id == id)
    }

    /// 感染源になれるか（潜伏中または発症中）。
    pub fn is_contagious_with(&self, id: &str) -> bool {
        self.infections
            .iter()
            .any(|i| i.pathogen_id == id && !matches!(i.stage, Stage::Recovered))
    }

    /// 発症中で最も重い病の衰弱度（行動低下に使う）。
    pub fn debilitation(&self) -> f32 {
        self.infections
            .iter()
            .filter(|i| i.stage == Stage::Symptomatic)
            .map(|i| {
                pathogen(i.pathogen_id)
                    .map(|p| p.debilitation * i.severity)
                    .unwrap_or(0.0)
            })
            .fold(0.0, f32::max)
    }

    /// 感染を試みる。既に免疫・感染済みなら何もしない。
    /// `exposure` は接触の強さ（距離が近い・咬まれた等で高い）。`roll` は [0,1)。
    pub fn try_infect(&mut self, p: &Pathogen, host: HostClass, exposure: f32, roll: f32) -> bool {
        if !can_infect(p, host) || self.is_immune(p.id) || self.is_infected_with(p.id) {
            return false;
        }
        // 感染確率 = 感染力 × 曝露 × (1 - 免疫)。
        let chance = (p.contagion * exposure * (1.0 - self.immunity)).clamp(0.0, 0.98);
        if roll < chance {
            self.infections.push(Infection {
                pathogen_id: p.id,
                stage: Stage::Incubating,
                elapsed_hours: 0.0,
                severity: 0.0,
            });
            true
        } else {
            false
        }
    }

    /// 時間を進める。潜伏→発症→回復/死亡を管理する。
    /// 戻り値: この tick で死に至ったか（`death_roll` は [0,1)）。
    pub fn tick(&mut self, dt_hours: f32, death_roll: f32) -> bool {
        let mut newly_immune: Vec<&'static str> = Vec::new();
        let mut died = false;

        for inf in self.infections.iter_mut() {
            let Some(p) = pathogen(inf.pathogen_id) else { continue };

            // 発症に使える時間。潜伏が明けたら、その残りをそのまま病状の進行へ回す。
            // こうしないと、時間を 100 倍速で飛ばしたときに病がまったく進まない。
            let mut active_hours = dt_hours;
            if inf.stage == Stage::Incubating {
                inf.elapsed_hours += dt_hours;
                if inf.elapsed_hours >= p.incubation_hours {
                    active_hours = inf.elapsed_hours - p.incubation_hours;
                    inf.stage = Stage::Symptomatic;
                    inf.elapsed_hours = active_hours;
                } else {
                    continue;
                }
            } else {
                inf.elapsed_hours += dt_hours;
            }

            if inf.stage != Stage::Symptomatic || active_hours <= 0.0 {
                continue;
            }

            // 進行と、免疫による回復の綱引き。
            let progress = p.virulence * active_hours;
            let recovery = self.immunity * 0.03 * active_hours;
            inf.severity = (inf.severity + progress - recovery).clamp(0.0, 1.0);

            if inf.severity >= 1.0 {
                // 致死判定。
                if death_roll < p.lethality {
                    died = true;
                } else {
                    // 峠を越えて回復へ。
                    inf.severity = 0.6;
                    inf.stage = Stage::Recovered;
                    if p.confers_immunity {
                        newly_immune.push(p.id);
                    }
                }
            } else if inf.severity <= 0.0 && inf.elapsed_hours > 1.0 {
                // 免疫が病原体を抑え込んだ。
                inf.stage = Stage::Recovered;
                if p.confers_immunity {
                    newly_immune.push(p.id);
                }
            }
        }

        for id in newly_immune {
            if !self.immune_to.contains(&id) {
                self.immune_to.push(id);
            }
        }
        // 回復した感染は片付ける。
        self.infections.retain(|i| i.stage != Stage::Recovered);
        died
    }

    /// 医薬（薬草・ポーション）で治療する。重症度を下げ、免疫を一時的に底上げする。
    pub fn treat(&mut self, potency: f32) {
        for inf in self.infections.iter_mut() {
            inf.severity = (inf.severity - potency).max(0.0);
        }
        self.immunity = (self.immunity + potency * 0.2).min(1.0);
    }

    /// 現在の症状の説明（HUD 用）。
    pub fn status_lines(&self) -> Vec<String> {
        self.infections
            .iter()
            .filter_map(|i| {
                let p = pathogen(i.pathogen_id)?;
                let stage = match i.stage {
                    Stage::Incubating => "潜伏".to_string(),
                    Stage::Symptomatic => format!("発症 重症度{:.0}%", i.severity * 100.0),
                    Stage::Recovered => "回復".to_string(),
                };
                Some(format!("{}（{}）", p.name, stage))
            })
            .collect()
    }
}

/// 2 個体間の曝露強度。感染経路と距離、同じ水場かどうかで決まる。
pub fn exposure_between(
    p: &Pathogen,
    distance: f32,
    same_water_source: bool,
    was_bitten: bool,
) -> f32 {
    if distance > p.range && !same_water_source {
        return 0.0;
    }
    match p.transmission {
        Transmission::Airborne => (1.0 - distance / p.range).clamp(0.0, 1.0),
        Transmission::Contact => {
            if was_bitten {
                1.0
            } else {
                (1.0 - distance / p.range).clamp(0.0, 1.0) * 0.6
            }
        }
        Transmission::Waterborne => {
            if same_water_source {
                0.8
            } else {
                0.0
            }
        }
        Transmission::Vector => (1.0 - distance / p.range).clamp(0.0, 1.0) * 0.7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_map_to_the_right_host_class() {
        assert_eq!(HostClass::of_species("wolf"), HostClass::Canine);
        assert_eq!(HostClass::of_species("chicken"), HostClass::Bird);
        assert_eq!(HostClass::of_species("cow"), HostClass::Livestock);
        assert_eq!(HostClass::of_species("deer"), HostClass::WildMammal);
        assert_eq!(HostClass::of_species("salmon"), HostClass::Fish);
    }

    #[test]
    fn diseases_only_infect_their_valid_hosts() {
        let rabies = pathogen("rabies").unwrap();
        let bird_flu = pathogen("avian_flu").unwrap();
        assert!(can_infect(rabies, HostClass::Canine));
        assert!(can_infect(rabies, HostClass::Human), "rabies is zoonotic");
        assert!(!can_infect(rabies, HostClass::Fish));
        assert!(can_infect(bird_flu, HostClass::Bird));
        assert!(!can_infect(bird_flu, HostClass::Canine));
    }

    #[test]
    fn a_wolf_can_catch_rabies_a_fish_cannot() {
        let rabies = pathogen("rabies").unwrap();
        let mut wolf = ImmuneSystem::new(0.1);
        // 咬まれた（曝露最大）。決定論のため roll=0。
        assert!(wolf.try_infect(rabies, HostClass::Canine, 1.0, 0.0));

        let mut fish = ImmuneSystem::new(0.1);
        assert!(!fish.try_infect(rabies, HostClass::Fish, 1.0, 0.0), "a fish cannot get rabies");
    }

    #[test]
    fn high_immunity_resists_infection() {
        let flu = pathogen("influenza").unwrap();
        let mut hardy = ImmuneSystem::new(0.95);
        // 高い免疫では、そこそこの曝露では感染しにくい。
        let mut infected = 0;
        for i in 0..100 {
            let mut sys = hardy.clone();
            if sys.try_infect(flu, HostClass::Human, 0.5, i as f32 / 100.0) {
                infected += 1;
            }
        }
        assert!(infected < 20, "a highly immune host got infected too often: {infected}/100");
        let _ = &mut hardy;
    }

    #[test]
    fn infection_progresses_through_incubation_to_symptoms() {
        let mut sys = ImmuneSystem::new(0.1);
        let cold = pathogen("common_cold").unwrap();
        sys.try_infect(cold, HostClass::Human, 1.0, 0.0);
        assert_eq!(sys.infections[0].stage, Stage::Incubating);
        // 潜伏期間ぶん進めると発症する。
        sys.tick(cold.incubation_hours + 1.0, 0.99);
        assert_eq!(sys.infections[0].stage, Stage::Symptomatic);
    }

    #[test]
    fn a_deadly_disease_can_kill() {
        let mut sys = ImmuneSystem::new(0.0);
        let rabies = pathogen("rabies").unwrap();
        sys.try_infect(rabies, HostClass::Human, 1.0, 0.0);
        let mut died = false;
        // 致死率 0.95 なので death_roll=0 なら必ず死ぬ（重症度が最大に達したとき）。
        for _ in 0..2000 {
            if sys.tick(1.0, 0.0) {
                died = true;
                break;
            }
        }
        assert!(died, "rabies with no immunity and unlucky rolls should be fatal");
    }

    #[test]
    fn recovering_from_flu_grants_immunity() {
        let mut sys = ImmuneSystem::new(0.8); // 高い免疫で回復に寄せる
        let flu = pathogen("influenza").unwrap();
        sys.try_infect(flu, HostClass::Human, 1.0, 0.0);
        // 死なない引き（death_roll=1.0）で長く進めれば、いずれ回復して免疫を得る。
        for _ in 0..5000 {
            sys.tick(1.0, 1.0);
            if sys.is_immune("influenza") {
                break;
            }
        }
        assert!(sys.is_immune("influenza"), "flu should confer immunity after recovery");
        // 免疫があれば再感染しない。
        assert!(!sys.try_infect(flu, HostClass::Human, 1.0, 0.0));
    }

    #[test]
    fn the_common_cold_does_not_grant_immunity() {
        let cold = pathogen("common_cold").unwrap();
        assert!(!cold.confers_immunity);
    }

    #[test]
    fn treatment_reduces_severity() {
        let mut sys = ImmuneSystem::new(0.2);
        let plague = pathogen("plague").unwrap();
        sys.try_infect(plague, HostClass::Human, 1.0, 0.0);
        // 潜伏を抜けた直後：発症しているが、まだ峠は越えていない。
        sys.tick(plague.incubation_hours + 5.0, 0.99);
        assert_eq!(sys.infections.len(), 1, "the infection should still be running");
        let before = sys.infections[0].severity;
        assert!(before > 0.0, "the plague should have started to progress");
        sys.treat(0.5);
        assert!(sys.infections[0].severity < before);
    }

    #[test]
    fn exposure_respects_transmission_route() {
        let airborne = pathogen("influenza").unwrap();
        let contact = pathogen("rabies").unwrap();
        let water = pathogen("dysentery").unwrap();

        // 空気感染は距離が近いほど強い。
        assert!(exposure_between(airborne, 0.5, false, false) > exposure_between(airborne, 3.5, false, false));
        // 接触感染は咬まれると最大、離れると 0。
        assert_eq!(exposure_between(contact, 1.0, false, true), 1.0);
        assert_eq!(exposure_between(contact, 10.0, false, false), 0.0);
        // 水系は同じ水場でのみ。
        assert!(exposure_between(water, 50.0, true, false) > 0.0);
        assert_eq!(exposure_between(water, 0.5, false, false), 0.0);
    }

    #[test]
    fn a_host_can_be_a_source_only_while_infectious() {
        let mut sys = ImmuneSystem::new(0.2);
        let cold = pathogen("common_cold").unwrap();
        assert!(!sys.is_contagious_with("common_cold"));
        sys.try_infect(cold, HostClass::Human, 1.0, 0.0);
        // 潜伏中でも感染源になる。
        assert!(sys.is_contagious_with("common_cold"));
    }
}
