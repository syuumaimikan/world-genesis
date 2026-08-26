//! 天球：太陽・月・星・星座・天の川。
//!
//! 星は貼り絵ではない。恒星は天球上の固定座標を持ち、惑星の自転で
//! 夜ごとに東から西へ流れ、公転（＝日付）でゆっくり位置がずれる。
//! 月は満ち欠けし、黄道十二星座は季節で入れ替わる。
//! そして占星術のための「凶星」——災いが近づくと赤く輝く星——を持つ。
//!
//! すべて (シード, tick) の純関数。Bevy に依存しない。

use crate::noise::hash_u64;
use glam::Vec3;

pub const TICKS_PER_DAY: u64 = 86_400;
/// 1 年の日数（ゲーム暦）。
pub const DAYS_PER_YEAR: u64 = 360;

/// 天球上の 1 天体。
#[derive(Debug, Clone, Copy)]
pub struct Star {
    /// 天球（半径 1 の球）上の基準方向。
    pub base_dir: Vec3,
    /// 見かけの明るさ 0〜1。
    pub magnitude: f32,
    /// 色温度（0=赤, 0.5=白, 1=青）。
    pub color_temp: f32,
    /// 天の川の帯に属するか。
    pub in_milky_way: bool,
}

impl Star {
    /// 指定 tick における天球上の方向。
    ///
    /// 惑星の自転で 1 日 1 回転し、公転で 1 年かけて 1 回転ぶんずれる
    /// （恒星時と太陽時の差＝歳差に相当する見かけの回転）。
    pub fn direction_at(&self, seed: u64, tick: u64) -> Vec3 {
        let _ = seed;
        let day_angle = (tick % TICKS_PER_DAY) as f32 / TICKS_PER_DAY as f32 * std::f32::consts::TAU;
        // 公転による年周のずれ。
        let year_angle =
            (tick / TICKS_PER_DAY % DAYS_PER_YEAR) as f32 / DAYS_PER_YEAR as f32 * std::f32::consts::TAU;
        let angle = day_angle + year_angle;
        // 天の北極（Y 軸）まわりの回転。
        let rot = glam::Quat::from_rotation_y(angle);
        rot * self.base_dir
    }
}

/// 星座（恒星のつながり）。
#[derive(Debug, Clone)]
pub struct Constellation {
    pub name: &'static str,
    /// 構成する恒星の添字（`StarField::stars` を指す）。
    pub star_indices: Vec<usize>,
    /// 黄道十二星座か（季節で空を移る）。
    pub zodiac: bool,
}

/// 月の状態。
#[derive(Debug, Clone, Copy)]
pub struct Moon {
    /// 満ち欠け 0〜1（0=新月, 0.5=満月, 1=新月）。
    pub phase: f32,
    /// 天球上の方向。
    pub direction: Vec3,
    /// 照度（満月ほど明るい）。
    pub illumination: f32,
}

/// 恒星の集合と星座。ワールドごとに一度だけ生成する。
pub struct StarField {
    pub seed: u64,
    pub stars: Vec<Star>,
    pub constellations: Vec<Constellation>,
    /// 占星術の「凶星」の添字。
    pub omen_star: usize,
}

const ZODIAC_NAMES: [&str; 12] = [
    "白羊宮", "金牛宮", "双子宮", "巨蟹宮", "獅子宮", "処女宮",
    "天秤宮", "天蠍宮", "人馬宮", "磨羯宮", "宝瓶宮", "双魚宮",
];

const OTHER_CONSTELLATIONS: [&str; 8] = [
    "北の竜", "狩人", "大熊", "小舟", "鍛冶神の鎚", "旅人の杖", "戴冠", "落涙",
];

impl StarField {
    /// シードから恒星と星座を組み上げる。
    pub fn generate(seed: u64, star_count: usize) -> Self {
        let mut stars = Vec::with_capacity(star_count);

        for i in 0..star_count {
            let h = hash_u64(seed ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            // 球面上に一様分布させる。
            let u = ((h >> 40) & 0xFFFF) as f32 / 65535.0;
            let v = ((h >> 20) & 0xFFFF) as f32 / 65535.0;
            let theta = u * std::f32::consts::TAU;
            let z = v * 2.0 - 1.0;
            let r = (1.0 - z * z).max(0.0).sqrt();
            let base_dir = Vec3::new(r * theta.cos(), z, r * theta.sin());

            // 天の川：ある大円（銀河面）の近くに星を集中させる。
            let galactic = base_dir.dot(Vec3::new(0.3, 0.2, 0.93).normalize()).abs();
            let in_milky_way = galactic < 0.12;

            let mag_roll = ((h >> 8) & 0xFF) as f32 / 255.0;
            // 明るい星は少なく、暗い星が多い。
            let magnitude = if in_milky_way {
                0.15 + mag_roll * 0.35
            } else {
                (mag_roll * mag_roll) * 0.9 + 0.1
            };
            let color_temp = ((h >> 16) & 0xFF) as f32 / 255.0;

            stars.push(Star {
                base_dir,
                magnitude,
                color_temp,
                in_milky_way,
            });
        }

        // 星座を作る：近い明るい星をいくつか束ねる。
        let bright: Vec<usize> = {
            let mut idx: Vec<usize> = (0..stars.len()).collect();
            idx.sort_by(|&a, &b| stars[b].magnitude.partial_cmp(&stars[a].magnitude).unwrap());
            idx
        };

        let mut constellations = Vec::new();
        let mut used = vec![false; stars.len()];

        // 黄道十二星座：黄道帯（赤道付近）の星から作る。
        for (zi, name) in ZODIAC_NAMES.iter().enumerate() {
            let mut group = Vec::new();
            // 黄道を 12 分割し、その経度帯の明るい星を集める。
            let lon_lo = zi as f32 / 12.0 * std::f32::consts::TAU;
            let lon_hi = (zi + 1) as f32 / 12.0 * std::f32::consts::TAU;
            for &s in &bright {
                if used[s] {
                    continue;
                }
                let d = stars[s].base_dir;
                // 黄道帯（|y| が小さい）。
                if d.y.abs() > 0.35 {
                    continue;
                }
                let lon = d.z.atan2(d.x).rem_euclid(std::f32::consts::TAU);
                if lon >= lon_lo && lon < lon_hi {
                    group.push(s);
                    used[s] = true;
                    if group.len() >= 5 {
                        break;
                    }
                }
            }
            if group.len() >= 3 {
                constellations.push(Constellation {
                    name,
                    star_indices: group,
                    zodiac: true,
                });
            }
        }

        // その他の星座：残りの明るい星から。
        for name in OTHER_CONSTELLATIONS {
            let seed_star = bright.iter().copied().find(|&s| !used[s]);
            let Some(anchor) = seed_star else { break };
            let mut group = vec![anchor];
            used[anchor] = true;
            let a = stars[anchor].base_dir;
            // アンカーに近い明るい星を集める。
            for &s in &bright {
                if used[s] {
                    continue;
                }
                if stars[s].base_dir.distance(a) < 0.35 {
                    group.push(s);
                    used[s] = true;
                    if group.len() >= 6 {
                        break;
                    }
                }
            }
            if group.len() >= 3 {
                constellations.push(Constellation {
                    name,
                    star_indices: group,
                    zodiac: false,
                });
            }
        }

        // 凶星：最も明るい星を 1 つ選ぶ。
        let omen_star = bright.first().copied().unwrap_or(0);

        Self {
            seed,
            stars,
            constellations,
            omen_star,
        }
    }

    /// 太陽の方向。東（+X）から昇り、南（+Z 側）へ傾いた弧を描いて西（-X）へ沈む。
    ///
    /// 日周運動は 1 本の大円なので、高度と方位は同じ角度から導く。
    /// こうしておけば「日の出は必ず東」という関係が壊れない。
    pub fn sun_direction(&self, tick: u64) -> Vec3 {
        let t = (tick % TICKS_PER_DAY) as f32 / TICKS_PER_DAY as f32;
        // 6 時（t=0.25）に日の出、18 時（t=0.75）に日没。
        let a = (t - 0.25) * std::f32::consts::TAU;
        Vec3::new(a.cos() * 0.72, a.sin(), 0.34).normalize_or_zero()
    }

    /// 月の状態。月は約 30 日周期で満ち欠けし、太陽とずれて空を回る。
    pub fn moon(&self, tick: u64) -> Moon {
        let day = tick / TICKS_PER_DAY;
        // 30 日周期の満ち欠け。
        let phase = (day % 30) as f32 / 30.0;
        // 満月（phase=0.5）で最大。
        let illumination = 1.0 - (phase * 2.0 - 1.0).abs();

        // 月は太陽と半日ずれた位置を、満ち欠けに応じて少し遅れて回る。
        let t = (tick % TICKS_PER_DAY) as f32 / TICKS_PER_DAY as f32;
        let lunar = t + 0.5 + phase * 0.03;
        let a = (lunar - 0.25) * std::f32::consts::TAU;
        let direction = Vec3::new(a.cos() * 0.72, a.sin(), -0.28).normalize_or_zero();

        Moon {
            phase,
            direction,
            illumination,
        }
    }

    /// いま天頂近くにある黄道星座（＝今の「星座」）。
    pub fn current_zodiac(&self, tick: u64) -> Option<&Constellation> {
        let day = tick / TICKS_PER_DAY % DAYS_PER_YEAR;
        // 1 年で 12 星座を一巡する。
        let sign = (day * 12 / DAYS_PER_YEAR) as usize;
        self.constellations
            .iter()
            .filter(|c| c.zodiac)
            .nth(sign.min(11))
    }

    /// 凶星の輝きの色。`disaster_omen` は差し迫った災害の予兆度 0〜1。
    /// 平時は白、災いが近いと赤く燃える（占星術）。
    pub fn omen_color(&self, disaster_omen: f32) -> [f32; 3] {
        let o = disaster_omen.clamp(0.0, 1.0);
        // 白 (1,1,1) → 赤 (1, 0.2, 0.15)
        [1.0, 1.0 - o * 0.8, 1.0 - o * 0.85]
    }

    /// 恒星の現在方向のうち、地平線より上（可視）のものだけを返す。
    /// `(方向, 明るさ, 色温度)` の列。夜ほど、また月が暗いほど星がよく見える。
    pub fn visible_stars(&self, tick: u64, brightness_cutoff: f32) -> Vec<(Vec3, f32, f32)> {
        self.stars
            .iter()
            .filter_map(|s| {
                let d = s.direction_at(self.seed, tick);
                if d.y > 0.02 && s.magnitude >= brightness_cutoff {
                    Some((d, s.magnitude, s.color_temp))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// 空の暗さ 0〜1（0=真昼, 1=真夜中）。星の見え方に使う。
pub fn night_factor(sun_elevation: f32) -> f32 {
    (-sun_elevation * 3.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field() -> StarField {
        StarField::generate(0xC0FFEE, 1500)
    }

    #[test]
    fn generation_is_deterministic() {
        let a = StarField::generate(42, 500);
        let b = StarField::generate(42, 500);
        assert_eq!(a.stars.len(), b.stars.len());
        assert_eq!(a.stars[100].base_dir, b.stars[100].base_dir);
        assert_eq!(a.omen_star, b.omen_star);
    }

    #[test]
    fn stars_lie_on_the_unit_sphere() {
        let f = field();
        for s in &f.stars {
            assert!((s.base_dir.length() - 1.0).abs() < 1e-3, "star is not on the celestial sphere");
            assert!((0.0..=1.0).contains(&s.magnitude));
        }
    }

    #[test]
    fn stars_move_across_the_sky_through_the_night() {
        let f = field();
        let s = &f.stars[10];
        let d0 = s.direction_at(f.seed, 0);
        let d1 = s.direction_at(f.seed, TICKS_PER_DAY / 4);
        assert!(d0.distance(d1) > 0.1, "the star did not move as the planet rotated");
        // 1 日で一周して戻る。
        let d_full = s.direction_at(f.seed, TICKS_PER_DAY);
        // 公転ぶんだけずれるので完全一致はしないが、近い。
        assert!(d0.distance(d_full) < 0.1);
    }

    #[test]
    fn the_star_field_drifts_over_the_year() {
        let f = field();
        let s = &f.stars[10];
        // 同じ時刻（真夜中）でも、半年後は空が違う。
        let winter = s.direction_at(f.seed, 0);
        let summer = s.direction_at(f.seed, TICKS_PER_DAY * 180);
        assert!(winter.distance(summer) > 0.3, "the sky looks identical across seasons");
    }

    #[test]
    fn the_sun_rises_in_the_east_and_sets_in_the_west() {
        let f = field();
        let noon = f.sun_direction(TICKS_PER_DAY / 2);
        let midnight = f.sun_direction(0);
        // 真上ではなく高い位置を通る（現実でも天頂を通るのは赤道の春秋分だけ）。
        assert!(noon.y > 0.8, "the noon sun should be high in the sky, got y={}", noon.y);
        assert!(midnight.y < -0.5, "the midnight sun should be below the horizon");
        // 朝は東（+X 寄り）、夕は西（-X 寄り）。
        let morning = f.sun_direction(TICKS_PER_DAY * 6 / 24);
        let evening = f.sun_direction(TICKS_PER_DAY * 18 / 24);
        assert!(morning.x > evening.x, "the sun should travel east to west");
    }

    #[test]
    fn the_moon_waxes_and_wanes() {
        let f = field();
        let new_moon = f.moon(0);
        let full_moon = f.moon(TICKS_PER_DAY * 15);
        assert!(new_moon.illumination < 0.2, "day 0 should be near a new moon");
        assert!(full_moon.illumination > 0.8, "day 15 should be near a full moon");
    }

    #[test]
    fn there_are_twelve_zodiac_signs_and_they_rotate() {
        let f = field();
        let zodiac_count = f.constellations.iter().filter(|c| c.zodiac).count();
        assert!(zodiac_count >= 8, "expected close to twelve zodiac constellations, got {zodiac_count}");

        // 季節が違えば頭上の星座も違う。
        let spring = f.current_zodiac(TICKS_PER_DAY * 30);
        let autumn = f.current_zodiac(TICKS_PER_DAY * 210);
        if let (Some(a), Some(b)) = (spring, autumn) {
            assert_ne!(a.name, b.name, "the zodiac sign should change with the season");
        }
    }

    #[test]
    fn the_milky_way_is_a_dense_band_not_the_whole_sky() {
        let f = field();
        let band = f.stars.iter().filter(|s| s.in_milky_way).count();
        let total = f.stars.len();
        assert!(band > 0, "there should be a milky way");
        assert!(band < total / 2, "the milky way should be a band, not the whole sky");
    }

    #[test]
    fn the_omen_star_reddens_as_disaster_approaches() {
        let f = field();
        let calm = f.omen_color(0.0);
        let doom = f.omen_color(1.0);
        assert!(calm[1] > 0.9 && calm[2] > 0.9, "in calm times the omen star is white");
        assert!(doom[1] < 0.3 && doom[0] > 0.8, "before disaster the omen star burns red");
    }

    #[test]
    fn stars_are_only_visible_above_the_horizon() {
        let f = field();
        let vis = f.visible_stars(0, 0.0);
        assert!(!vis.is_empty());
        for (dir, _, _) in &vis {
            assert!(dir.y > 0.0, "a star below the horizon was reported as visible");
        }
    }

    #[test]
    fn night_factor_tracks_the_sun() {
        assert_eq!(night_factor(1.0), 0.0, "noon is not night");
        assert!(night_factor(-0.5) > 0.9, "deep night should be dark");
    }
}
