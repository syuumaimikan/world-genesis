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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_inv_returns_reciprocal_for_normal_values() {
        assert_eq!(safe_inv(4.0, 99.0), 0.25);
        assert_eq!(safe_inv(-2.0, 99.0), -0.5);
    }

    #[test]
    fn safe_inv_falls_back_near_zero() {
        assert_eq!(safe_inv(0.0, 7.0), 7.0);
        assert_eq!(safe_inv(1e-9, 7.0), 7.0);
        assert_eq!(safe_inv(-1e-9, 7.0), 7.0);
    }

    #[test]
    fn sanitize_f32_clamps_finite_values() {
        assert_eq!(sanitize_f32(5.0, 0.0, 1.0), 1.0);
        assert_eq!(sanitize_f32(-5.0, 0.0, 1.0), 0.0);
        assert_eq!(sanitize_f32(0.5, 0.0, 1.0), 0.5);
    }

    #[test]
    fn sanitize_f32_replaces_non_finite_with_min() {
        assert_eq!(sanitize_f32(f32::NAN, -3.0, 3.0), -3.0);
        assert_eq!(sanitize_f32(f32::INFINITY, -3.0, 3.0), -3.0);
        assert_eq!(sanitize_f32(f32::NEG_INFINITY, -3.0, 3.0), -3.0);
    }

    #[test]
    fn bilinear_interpolate_hits_corners_and_center() {
        assert_eq!(bilinear_interpolate(0.0, 1.0, 2.0, 3.0, 0.0, 0.0), 0.0);
        assert_eq!(bilinear_interpolate(0.0, 1.0, 2.0, 3.0, 1.0, 0.0), 2.0);
        assert_eq!(bilinear_interpolate(0.0, 1.0, 2.0, 3.0, 0.0, 1.0), 1.0);
        assert_eq!(bilinear_interpolate(0.0, 1.0, 2.0, 3.0, 1.0, 1.0), 3.0);
        assert_eq!(bilinear_interpolate(0.0, 1.0, 2.0, 3.0, 0.5, 0.5), 1.5);
    }

    #[test]
    fn curl_noise_of_linear_field_is_constant_and_orthogonal() {
        // f(p) = 2x + 3y  =>  grad = (2, 3)  =>  curl = (3, -2)
        let curl = curl_noise_2d(Vec2::new(4.0, -7.0), |p| 2.0 * p.x + 3.0 * p.y);
        assert!((curl.x - 3.0).abs() < 1e-2, "curl.x = {}", curl.x);
        assert!((curl.y + 2.0).abs() < 1e-2, "curl.y = {}", curl.y);
    }

    #[test]
    fn curl_noise_is_divergence_free_rotation_of_gradient() {
        let sample = |p: Vec2| (p.x * 0.7).sin() * (p.y * 1.3).cos();
        let p = Vec2::new(1.25, -0.5);
        let curl = curl_noise_2d(p, sample);
        const EPS: f32 = 0.001;
        let grad = Vec2::new(
            (sample(p + Vec2::new(EPS, 0.0)) - sample(p - Vec2::new(EPS, 0.0))) / (2.0 * EPS),
            (sample(p + Vec2::new(0.0, EPS)) - sample(p - Vec2::new(0.0, EPS))) / (2.0 * EPS),
        );
        assert!(
            curl.dot(grad).abs() < 1e-3,
            "curl must be perpendicular to gradient"
        );
    }
}
