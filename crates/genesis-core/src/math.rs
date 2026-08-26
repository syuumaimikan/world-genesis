use glam::Vec2;

#[inline]
pub fn safe_inv(val: f32, fallback: f32) -> f32 {
    if val.abs() < 1e-7 {
        fallback
    } else {
        1.0 / val
    }
}

#[inline]
pub fn sanitize_f32(val: f32, min_val: f32, max_val: f32) -> f32 {
    if val.is_nan() || val.is_infinite() {
        min_val
    } else {
        val.clamp(min_val, max_val)
    }
}

#[inline]
pub fn bilinear_interpolate(q11: f32, q12: f32, q21: f32, q22: f32, tx: f32, ty: f32) -> f32 {
    let r1 = (1.0 - tx) * q11 + tx * q21;
    let r2 = (1.0 - tx) * q12 + tx * q22;
    (1.0 - ty) * r1 + ty * r2
}

pub fn curl_noise_2d(p: Vec2, sample_fn: impl Fn(Vec2) -> f32) -> Vec2 {
    const EPS: f32 = 0.001;
    let dx = (sample_fn(p + Vec2::new(EPS, 0.0)) - sample_fn(p - Vec2::new(EPS, 0.0))) / (2.0 * EPS);
    let dy = (sample_fn(p + Vec2::new(0.0, EPS)) - sample_fn(p - Vec2::new(0.0, EPS))) / (2.0 * EPS);
    Vec2::new(dy, -dx)
}
