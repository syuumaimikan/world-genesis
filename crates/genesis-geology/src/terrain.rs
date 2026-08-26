use genesis_core::math::sanitize_f32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceMaterial {
    BasalticRock,
    GraniticRock,
    SedimentarySand,
    VolcanicAsh,
    HumusSoil,
    GlacialIce,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RockLayer {
    pub rock_type: SurfaceMaterial,
    pub thickness: f32,
    pub hardness: f32,
    pub permeability: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeightField {
    pub width: usize,
    pub height: usize,
    pub elevation: Vec<f32>,
    pub water_depth: Vec<f32>,
    pub sediment_depth: Vec<f32>,
    pub bedrock_hardness: Vec<f32>,
}

impl HeightField {
    pub fn new(width: usize, height: usize, initial_elevation: f32) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            elevation: vec![initial_elevation; size],
            water_depth: vec![0.0; size],
            sediment_depth: vec![0.0; size],
            bedrock_hardness: vec![1.0; size],
        }
    }

    #[inline]
    pub fn index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    #[inline]
    pub fn get_elevation(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height {
            self.elevation[self.index(x, y)]
        } else {
            -150.0 // 範囲外は深海
        }
    }

    #[inline]
    pub fn set_elevation(&mut self, x: usize, y: usize, val: f32) {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            self.elevation[idx] = sanitize_f32(val, -3000.0, 3000.0);
        }
    }

    /// バイリニア補間（陸地から深海まで滑らかに足元高さを判定）
    pub fn sample_height_bilinear(&self, world_x: f32, world_z: f32, scale: f32) -> f32 {
        let gx = world_x / scale;
        let gz = world_z / scale;

        if gx < 0.0 || gz < 0.0 || gx >= (self.width - 1) as f32 || gz >= (self.height - 1) as f32 {
            return -120.0; // 世界の外洋
        }

        let x0 = gx.floor() as usize;
        let z0 = gz.floor() as usize;
        let tx = gx - x0 as f32;
        let tz = gz - z0 as f32;

        let h00 = self.get_elevation(x0, z0);
        let h10 = self.get_elevation(x0 + 1, z0);
        let h01 = self.get_elevation(x0, z0 + 1);
        let h11 = self.get_elevation(x0 + 1, z0 + 1);

        let h0 = h00 * (1.0 - tx) + h10 * tx;
        let h1 = h01 * (1.0 - tx) + h11 * tx;
        h0 * (1.0 - tz) + h1 * tz
    }

    /// 途切れのない世界（主大陸・沿岸入り江・群島・なだらかな深海海底）の生成
    pub fn generate_continents(&mut self, seed: u64) {
        let w = self.width;
        let h = self.height;
        let s = (seed & 0xFFFF) as f32;

        for y in 0..h {
            for x in 0..w {
                let fx = x as f32 / w as f32;
                let fy = y as f32 / h as f32;

                // 1. 大陸スケール・山脈スケール・丘陵スケールの波形合成
                let continent_wave = ((fx * 3.1415 * 2.0 + s * 0.01).sin() * (fy * 3.1415 * 2.0 + s * 0.02).cos()) * 160.0;
                let mountain_ridges = ((fx * 3.1415 * 6.0 - s * 0.03).cos() * (fy * 3.1415 * 6.0 + s * 0.01).sin()).abs() * 120.0;
                let hills_detail = ((fx * 3.1415 * 12.0).sin() * (fy * 3.1415 * 12.0).cos()) * 30.0;

                // 2. 世界の縁に向かって滑らかに深海へ沈降させる楕円大陸マスク (Continental Margin)
                let dx = (fx - 0.5) * 2.0; // -1.0 to 1.0
                let dy = (fy - 0.5) * 2.0;
                let dist_from_center = (dx * dx + dy * dy).sqrt(); // 0.0 (中心) 〜 1.41 (隅)

                // 境界付近（dist > 0.7）で自然に深海 -120m へ遷移
                let shelf_falloff = if dist_from_center > 0.65 {
                    ((dist_from_center - 0.65) / 0.35).min(1.0)
                } else {
                    0.0
                };

                let raw_land = continent_wave + mountain_ridges + hills_detail + 40.0;
                let final_elev = raw_land * (1.0 - shelf_falloff) + (-140.0 * shelf_falloff);

                let idx = self.index(x, y);
                self.elevation[idx] = sanitize_f32(final_elev, -250.0, 900.0);
            }
        }
    }

    pub fn calculate_normal(&self, x: usize, y: usize) -> glam::Vec3 {
        let left = if x > 0 { self.get_elevation(x - 1, y) } else { self.get_elevation(x, y) };
        let right = if x + 1 < self.width { self.get_elevation(x + 1, y) } else { self.get_elevation(x, y) };
        let down = if y > 0 { self.get_elevation(x, y - 1) } else { self.get_elevation(x, y) };
        let up = if y + 1 < self.height { self.get_elevation(x, y + 1) } else { self.get_elevation(x, y) };

        let dx = (right - left) * 0.25;
        let dy = (up - down) * 0.25;

        glam::Vec3::new(-dx, 1.0, -dy).normalize()
    }
}
