//! 決定論的ノイズ関数群。
//!
//! ワールド生成はシード値と座標のみから全てを再現できなければならない（仕様 58）。
//! そのため乱数生成器の状態には一切依存せず、整数ハッシュから直接値を作る。
//! 同じ (seed, x, y, z) は常に同じ値を返すため、チャンクを任意の順番・任意のスレッドで
//! 生成しても世界は完全に一致する。

/// 64bit の雪崩ハッシュ（SplitMix64 finalizer）。
#[inline]
pub fn hash_u64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[inline]
pub fn hash2i(x: i32, z: i32, seed: u64) -> u64 {
    hash_u64(
        seed ^ (x as i64 as u64).wrapping_mul(0x8DA6_B343_1FA2_9C37)
            ^ (z as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F),
    )
}

#[inline]
pub fn hash3i(x: i32, y: i32, z: i32, seed: u64) -> u64 {
    hash_u64(
        seed ^ (x as i64 as u64).wrapping_mul(0x8DA6_B343_1FA2_9C37)
            ^ (y as i64 as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
            ^ (z as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F),
    )
}

/// [0,1) の一様乱数。
#[inline]
pub fn rand01_2i(x: i32, z: i32, seed: u64) -> f32 {
    (hash2i(x, z, seed) >> 40) as f32 / 16_777_216.0
}

#[inline]
pub fn rand01_3i(x: i32, y: i32, z: i32, seed: u64) -> f32 {
    (hash3i(x, y, z, seed) >> 40) as f32 / 16_777_216.0
}

/// [-1,1] の格子勾配を用いた 2D 勾配ノイズ（Perlin 相当）。
#[inline]
fn grad2(x: i32, z: i32, seed: u64) -> (f32, f32) {
    let h = hash2i(x, z, seed);
    let angle = (h >> 40) as f32 / 16_777_216.0 * std::f32::consts::TAU;
    (angle.cos(), angle.sin())
}

#[inline]
fn smootherstep(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub fn perlin2(x: f32, z: f32, seed: u64) -> f32 {
    let xi = x.floor();
    let zi = z.floor();
    let xf = x - xi;
    let zf = z - zi;
    let (x0, z0) = (xi as i32, zi as i32);

    let u = smootherstep(xf);
    let v = smootherstep(zf);

    let dot = |cx: i32, cz: i32, dx: f32, dz: f32| {
        let (gx, gz) = grad2(cx, cz, seed);
        gx * dx + gz * dz
    };

    let n00 = dot(x0, z0, xf, zf);
    let n10 = dot(x0 + 1, z0, xf - 1.0, zf);
    let n01 = dot(x0, z0 + 1, xf, zf - 1.0);
    let n11 = dot(x0 + 1, z0 + 1, xf - 1.0, zf - 1.0);

    let a = n00 + u * (n10 - n00);
    let b = n01 + u * (n11 - n01);
    // Perlin 2D の理論最大値 sqrt(2)/2 で正規化して概ね [-1,1] に収める。
    ((a + v * (b - a)) * std::f32::consts::SQRT_2).clamp(-1.0, 1.0)
}

#[inline]
fn grad3(x: i32, y: i32, z: i32, seed: u64) -> (f32, f32, f32) {
    let h = hash3i(x, y, z, seed);
    // 12 方向の標準的な勾配ベクトル集合。
    const G: [(f32, f32, f32); 12] = [
        (1.0, 1.0, 0.0), (-1.0, 1.0, 0.0), (1.0, -1.0, 0.0), (-1.0, -1.0, 0.0),
        (1.0, 0.0, 1.0), (-1.0, 0.0, 1.0), (1.0, 0.0, -1.0), (-1.0, 0.0, -1.0),
        (0.0, 1.0, 1.0), (0.0, -1.0, 1.0), (0.0, 1.0, -1.0), (0.0, -1.0, -1.0),
    ];
    G[(h % 12) as usize]
}

pub fn perlin3(x: f32, y: f32, z: f32, seed: u64) -> f32 {
    let (xi, yi, zi) = (x.floor(), y.floor(), z.floor());
    let (xf, yf, zf) = (x - xi, y - yi, z - zi);
    let (x0, y0, z0) = (xi as i32, yi as i32, zi as i32);
    let (u, v, w) = (smootherstep(xf), smootherstep(yf), smootherstep(zf));

    let dot = |cx: i32, cy: i32, cz: i32, dx: f32, dy: f32, dz: f32| {
        let (gx, gy, gz) = grad3(cx, cy, cz, seed);
        gx * dx + gy * dy + gz * dz
    };

    let n000 = dot(x0, y0, z0, xf, yf, zf);
    let n100 = dot(x0 + 1, y0, z0, xf - 1.0, yf, zf);
    let n010 = dot(x0, y0 + 1, z0, xf, yf - 1.0, zf);
    let n110 = dot(x0 + 1, y0 + 1, z0, xf - 1.0, yf - 1.0, zf);
    let n001 = dot(x0, y0, z0 + 1, xf, yf, zf - 1.0);
    let n101 = dot(x0 + 1, y0, z0 + 1, xf - 1.0, yf, zf - 1.0);
    let n011 = dot(x0, y0 + 1, z0 + 1, xf, yf - 1.0, zf - 1.0);
    let n111 = dot(x0 + 1, y0 + 1, z0 + 1, xf - 1.0, yf - 1.0, zf - 1.0);

    let lerp = |a: f32, b: f32, t: f32| a + t * (b - a);
    let x00 = lerp(n000, n100, u);
    let x10 = lerp(n010, n110, u);
    let x01 = lerp(n001, n101, u);
    let x11 = lerp(n011, n111, u);
    let y0v = lerp(x00, x10, v);
    let y1v = lerp(x01, x11, v);
    (lerp(y0v, y1v, w) * 1.15).clamp(-1.0, 1.0)
}

/// 多重フラクタルノイズ（fBm）。
pub fn fbm2(x: f32, z: f32, seed: u64, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += perlin2(x * freq, z * freq, seed ^ (o as u64).wrapping_mul(0x51_7C_C1_B7)) * amp;
        norm += amp;
        amp *= gain;
        freq *= lacunarity;
    }
    if norm > 0.0 {
        sum / norm
    } else {
        0.0
    }
}

pub fn fbm3(x: f32, y: f32, z: f32, seed: u64, octaves: u32) -> f32 {
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += perlin3(x * freq, y * freq, z * freq, seed ^ (o as u64).wrapping_mul(0x9E37_79B1)) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm.max(1e-6)
}

/// 尾根状ノイズ。山脈の鋭い稜線を作る。
pub fn ridged2(x: f32, z: f32, seed: u64, octaves: u32) -> f32 {
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        let n = 1.0 - perlin2(x * freq, z * freq, seed ^ (o as u64).wrapping_mul(0x2545_F491)).abs();
        sum += n * n * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (sum / norm.max(1e-6)) * 2.0 - 1.0
}

/// ドメインワーピング。座標そのものをノイズで歪ませ、単調な波形を有機的な形に変える。
#[inline]
pub fn domain_warp(x: f32, z: f32, seed: u64, strength: f32) -> (f32, f32) {
    let wx = fbm2(x * 0.5, z * 0.5, seed ^ 0xA53F, 3, 2.0, 0.5);
    let wz = fbm2(x * 0.5 + 41.7, z * 0.5 - 17.3, seed ^ 0x77C1, 3, 2.0, 0.5);
    (x + wx * strength, z + wz * strength)
}

/// ボロノイ（ワーリー）ノイズ。最近セル中心までの距離と、そのセルのハッシュを返す。
/// 洞窟の部屋・鉱床の分布・地域分割に使う。
pub fn voronoi2(x: f32, z: f32, seed: u64) -> (f32, u64) {
    let cx = x.floor() as i32;
    let cz = z.floor() as i32;
    let mut best_d = f32::MAX;
    let mut best_h = 0u64;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (gx, gz) = (cx + dx, cz + dz);
            let h = hash2i(gx, gz, seed);
            let px = gx as f32 + ((h >> 40) as f32 / 16_777_216.0);
            let pz = gz as f32 + (((h >> 16) & 0xFF_FFFF) as f32 / 16_777_216.0);
            let d = (px - x) * (px - x) + (pz - z) * (pz - z);
            if d < best_d {
                best_d = d;
                best_h = h;
            }
        }
    }
    (best_d.sqrt(), best_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_deterministic() {
        for i in 0..64 {
            let x = i as f32 * 0.37;
            let a = perlin2(x, x * 1.7, 12345);
            let b = perlin2(x, x * 1.7, 12345);
            assert_eq!(a, b, "same input must give same output");
        }
    }

    #[test]
    fn noise_stays_in_range() {
        for i in 0..2000 {
            let x = (i as f32) * 0.113;
            let z = (i as f32) * -0.271;
            let p = perlin2(x, z, 99);
            assert!((-1.0..=1.0).contains(&p) && p.is_finite(), "perlin2 out of range: {p}");
            let f = fbm2(x, z, 99, 5, 2.0, 0.5);
            assert!((-1.0..=1.0).contains(&f) && f.is_finite(), "fbm2 out of range: {f}");
            let r = ridged2(x, z, 99, 4);
            assert!((-1.01..=1.01).contains(&r) && r.is_finite(), "ridged2 out of range: {r}");
            let p3 = perlin3(x, z * 0.5, z, 99);
            assert!((-1.0..=1.0).contains(&p3) && p3.is_finite(), "perlin3 out of range: {p3}");
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let a: f32 = (0..100).map(|i| perlin2(i as f32 * 0.3, 5.0, 1)).sum();
        let b: f32 = (0..100).map(|i| perlin2(i as f32 * 0.3, 5.0, 2)).sum();
        assert!((a - b).abs() > 1e-3, "different seeds produced identical fields");
    }

    #[test]
    fn rand01_is_uniform_enough() {
        let mut buckets = [0u32; 10];
        for i in 0..10_000i32 {
            let v = rand01_2i(i, i * 7, 4242);
            assert!((0.0..1.0).contains(&v));
            buckets[(v * 10.0) as usize % 10] += 1;
        }
        // 完全一様である必要はないが、極端な偏りがあればハッシュが壊れている。
        for b in buckets {
            assert!(b > 500 && b < 1800, "bucket distribution is broken: {buckets:?}");
        }
    }
}
