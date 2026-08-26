use glam::{Mat4, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMode {
    Perspective,
    Orthographic,
}

#[derive(Debug, Clone)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    pub pitch_deg: f32, // 仰角 (-89° to +89°)
    pub yaw_deg: f32,   // 方位角 (0° to 360°)
    pub fov_deg: f32,
    pub aspect_ratio: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::new(64.0, 0.0, 64.0),
            distance: 120.0,
            pitch_deg: 45.0,
            yaw_deg: 45.0,
            fov_deg: 60.0,
            aspect_ratio: 16.0 / 9.0,
        }
    }
}

impl OrbitCamera {
    pub fn get_eye_position(&self) -> Vec3 {
        let pitch_rad = self.pitch_deg.to_radians();
        let yaw_rad = self.yaw_deg.to_radians();

        let x = self.distance * pitch_rad.cos() * yaw_rad.sin();
        let y = self.distance * pitch_rad.sin();
        let z = self.distance * pitch_rad.cos() * yaw_rad.cos();

        self.target + Vec3::new(x, y, z)
    }

    pub fn build_view_matrix(&self) -> Mat4 {
        let eye = self.get_eye_position();
        Mat4::look_at_rh(eye, self.target, Vec3::Y)
    }

    pub fn build_projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_deg.to_radians(), self.aspect_ratio, 0.1, 2000.0)
    }
}
