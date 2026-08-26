use genesis_geology::terrain::HeightField;
use genesis_life::plants::FloraCell;
use glam::{Vec3, Vec4};

#[derive(Debug, Clone, Copy, Default)]
pub struct Vertex3D {
    pub position: Vec3,
    pub normal: Vec3,
    pub color: Vec4,
}

#[derive(Debug, Clone, Default)]
pub struct TerrainMesh {
    pub vertices: Vec<Vertex3D>,
    pub indices: Vec<u32>,
}

pub struct TerrainMeshGenerator;

impl TerrainMeshGenerator {
    pub fn build_terrain_mesh(
        heightfield: &HeightField,
        flora: &[FloraCell],
        sea_level: f32,
        grid_scale: f32,
        height_scale: f32,
    ) -> TerrainMesh {
        let w = heightfield.width;
        let h = heightfield.height;
        let mut vertices = Vec::with_capacity(w * h);

        for y in 0..h {
            for x in 0..w {
                let idx = heightfield.index(x, y);
                let elev = heightfield.elevation[idx];
                let norm = heightfield.calculate_normal(x, y);

                let pos = Vec3::new(
                    x as f32 * grid_scale,
                    elev * height_scale,
                    y as f32 * grid_scale,
                );

                // 水深・標高・植生カラーリング
                let color = if elev < sea_level - 60.0 {
                    Vec4::new(0.04, 0.12, 0.28, 1.0) // 深海海底 (暗青色砂泥)
                } else if elev < sea_level - 10.0 {
                    Vec4::new(0.12, 0.35, 0.45, 1.0) // 浅瀬海底
                } else if elev < sea_level + 4.0 {
                    Vec4::new(0.88, 0.82, 0.58, 1.0) // 黄金の砂浜海岸
                } else if elev > 220.0 {
                    Vec4::new(0.96, 0.98, 1.0, 1.0)  // 山頂冠雪
                } else if elev > 130.0 {
                    Vec4::new(0.50, 0.46, 0.42, 1.0)  // 山岳岩肌
                } else {
                    let bio = flora.get(idx).map(|f| f.biomass_density).unwrap_or(4.0);
                    if bio > 10.0 {
                        Vec4::new(0.14, 0.48, 0.18, 1.0) // 深い森
                    } else if bio > 3.0 {
                        Vec4::new(0.32, 0.68, 0.26, 1.0) // 豊かな草原
                    } else {
                        Vec4::new(0.65, 0.62, 0.38, 1.0) // 平原・サバナ
                    }
                };

                vertices.push(Vertex3D { position: pos, normal: norm, color });
            }
        }

        let mut indices = Vec::with_capacity((w - 1) * (h - 1) * 6);
        for y in 0..h - 1 {
            for x in 0..w - 1 {
                let tl = (y * w + x) as u32;
                let tr = tl + 1;
                let bl = ((y + 1) * w + x) as u32;
                let br = bl + 1;

                indices.push(tl);
                indices.push(bl);
                indices.push(tr);

                indices.push(tr);
                indices.push(bl);
                indices.push(br);
            }
        }

        TerrainMesh { vertices, indices }
    }

    /// 水平線の彼方（2,000m）まで広がる大洋プレーン
    pub fn build_boundless_ocean_mesh(sea_level_y: f32) -> TerrainMesh {
        let size = 2000.0;
        let norm = Vec3::Y;
        let water_color = Vec4::new(0.08, 0.38, 0.72, 0.80);

        let vertices = vec![
            Vertex3D { position: Vec3::new(-size, sea_level_y, -size), normal: norm, color: water_color },
            Vertex3D { position: Vec3::new(size, sea_level_y, -size), normal: norm, color: water_color },
            Vertex3D { position: Vec3::new(-size, sea_level_y, size), normal: norm, color: water_color },
            Vertex3D { position: Vec3::new(size, sea_level_y, size), normal: norm, color: water_color },
        ];

        let indices = vec![0, 2, 1, 1, 2, 3];
        TerrainMesh { vertices, indices }
    }
}
