use glam::{Mat4, Quat, Vec2, Vec3};
use std::f32::consts::PI;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::NoUninit)]
pub struct CameraData {
    pub world_to_view: Mat4,
    pub view_to_world: Mat4,
    pub view_to_clip: Mat4,
    pub world_to_clip: Mat4,
}

#[derive(Debug)]
pub struct CameraController {
    world_position: Vec3,
    target: Vec3,
    up: Vec3,
    min_dist: f32,
    max_dist: f32,
    fov: f32,
    aspect_ratio: f32,
    near: f32,
    data: CameraData,
    is_dirty: bool,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            world_position: Vec3::new(5.0, 0.0, 0.0),
            target: Vec3::new(0.0, 0.0, 0.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            min_dist: 0.1,
            max_dist: 100.0,
            fov: PI / 3.0,
            aspect_ratio: 1.0,
            near: 0.1,
            is_dirty: true,
            data: CameraData::default(),
        }
    }
}

impl CameraController {
    pub const ALTITUDE_DELTA: f32 = 0.01;

    pub fn orbit(&mut self, delta: Vec2) {
        let relative_pos = self.world_position - self.target;
        let direction = relative_pos.normalize();
        let right = direction.cross(self.up); 
        let max_up_delta = direction.dot(self.up).acos();
        let max_down_delta = -(PI - max_up_delta);
        let rotation = Quat::from_axis_angle(self.up, -delta.x)
            * Quat::from_axis_angle(right, delta.y.clamp(max_down_delta + Self::ALTITUDE_DELTA, max_up_delta - Self::ALTITUDE_DELTA));
        self.world_position = self.target + rotation * relative_pos;
        self.invalidate();
    }

    pub fn zoom(&mut self, delta: f32) {
        let direction = self.world_position - self.target;
        let distance = direction.length();
        let direction = direction / distance;
        let distance = (distance - delta).clamp(self.min_dist, self.max_dist);
        self.world_position = self.target + direction * distance;
        self.invalidate();
    }

    pub fn move_in_eye_space(&mut self, delta: Vec3) {
        let cam_delta = self.data.world_to_view.transform_vector3(delta);
        self.world_position += cam_delta;
        self.target += cam_delta;
        self.invalidate();
    }

    pub fn resize(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio;
        self.invalidate();
    }

    pub fn invalidate(&mut self) {
        self.is_dirty = true;
    }

    fn calc_view_matrix(&self) -> Mat4 {
        Mat4::look_at_lh(self.world_position, self.target, self.up)
    }

    fn calc_projection_matrix(&self) -> Mat4 {
        Mat4::perspective_infinite_lh(self.fov, self.aspect_ratio, self.near)
    }

    fn calc_focal_length(&self) -> f32 {
        1.0 / (self.fov / 2.0).tan()
    }

    fn calc_camera_data(&self) -> CameraData {
        let world_to_view = self.calc_view_matrix();
        let view_to_clip = self.calc_projection_matrix();
        CameraData {
            world_to_view,
            view_to_world: world_to_view.inverse(),
            view_to_clip,
            world_to_clip: view_to_clip * world_to_view,
        }
    }

    pub fn update(&mut self) -> bool {
        if self.is_dirty {
            self.data = self.calc_camera_data();
            self.is_dirty = false;
            true
        } else {
            false
        }
    }

    pub fn data_as_u8(&self) -> &[u8] {
        bytemuck::bytes_of(&self.data)
    }
}