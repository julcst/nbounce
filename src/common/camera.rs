use glam::{Mat4, Quat, Vec2, Vec3};
use wgpu::util::{DeviceExt, BufferInitDescriptor};
use std::f32::consts::PI;

use super::WGPUContext;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::NoUninit)]
pub struct CameraBuffer {
    pub world_to_clip: Mat4,
    pub clip_to_world: Mat4,
}

#[derive(Debug)]
pub struct CameraController {
    world_position: Vec3,
    target: Vec3,
    up: Vec3,
    min_dist: f32,
    fov: f32,
    aspect_ratio: f32,
    near: f32,
    is_dirty: bool,
    data: CameraBuffer,
    buffer: wgpu::Buffer,
}

impl CameraController {
    pub const ALTITUDE_DELTA: f32 = 0.01;

    pub fn new(wgpu: &WGPUContext) -> Self {
        let data = CameraBuffer::default();

        let buffer = wgpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::bytes_of(&data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            world_position: Vec3::new(5.0, 0.0, 0.0),
            target: Vec3::new(0.0, 0.0, 0.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            min_dist: 0.1,
            fov: PI / 3.0,
            aspect_ratio: 1.0,
            near: 0.1,
            is_dirty: true,
            data: CameraBuffer::default(),
            buffer,
        }
    }

    pub fn orbit(&mut self, delta: Vec2) {
        let relative_pos = self.world_position - self.target;
        let direction = relative_pos.normalize();
        let right = direction.cross(self.up).normalize(); 
        let max_up_delta = direction.dot(self.up).acos();
        let max_down_delta = -(PI - max_up_delta);
        let clamped_delta_y = delta.y.clamp(max_down_delta + Self::ALTITUDE_DELTA, max_up_delta - Self::ALTITUDE_DELTA);
        let rotation = Quat::from_axis_angle(self.up, -delta.x)
            * Quat::from_axis_angle(right, clamped_delta_y);
        self.world_position = self.target + rotation.mul_vec3(relative_pos);
        self.invalidate();
    }

    pub fn zoom(&mut self, delta: f32) {
        let direction = self.world_position - self.target;
        let distance = direction.length();
        let direction = direction / distance;
        let distance = (distance - delta).max(self.min_dist);
        self.world_position = self.target + direction * distance;
        self.invalidate();
    }

    pub fn move_in_eye_space(&mut self, delta: Vec3) {
        let world_to_view = self.calc_view_matrix();
        let cam_delta = world_to_view.transform_vector3(delta);
        self.world_position += cam_delta;
        self.target += cam_delta;
        self.invalidate();
    }

    pub fn resize(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio;
        self.invalidate();
    }

    pub fn window_event(&mut self, event: &winit::event::WindowEvent) {
        match event {
            winit::event::WindowEvent::PinchGesture { delta, .. } => {
                // FIXME: Sometimes the pinch gesture is not detected
                self.zoom(*delta as f32 * 10.0);
            },
            winit::event::WindowEvent::Resized(size) => {
                self.resize(size.width as f32 / size.height as f32);
            },
            _ => {}
        }
    }

    pub fn device_event(&mut self, event: &winit::event::DeviceEvent) {
        #[allow(clippy::single_match)]
        match event {
            winit::event::DeviceEvent::MouseWheel { delta } => {
                let delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => Vec2::new(*x, *y),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => Vec2::new(pos.x as f32, pos.y as f32),
                };
                //self.camera.zoom(delta * 0.1);
                self.orbit(delta * 0.01);
            },
            _ => {}
        }
    }

    pub fn invalidate(&mut self) {
        self.is_dirty = true;
    }

    fn calc_view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.world_position, self.target, self.up)
    }

    fn calc_projection_matrix(&self) -> Mat4 {
        Mat4::perspective_infinite_rh(self.fov, self.aspect_ratio, self.near)
    }

    #[allow(dead_code)]
    fn calc_focal_length(&self) -> f32 {
        1.0 / (self.fov / 2.0).tan()
    }

    fn calc_camera_data(&self) -> CameraBuffer {
        let world_to_view = self.calc_view_matrix();
        let view_to_clip = self.calc_projection_matrix();
        let world_to_clip = view_to_clip * world_to_view;
        CameraBuffer {
            world_to_clip,
            clip_to_world: world_to_clip.inverse(),
        }
    }

    pub fn update(&mut self, wgpu: &WGPUContext) -> bool {
        if self.is_dirty {
            self.data = self.calc_camera_data();
            wgpu.queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&self.data));
            self.is_dirty = false;
            true
        } else {
            false
        }
    }

    pub fn buffer_binding(&self) -> wgpu::BindingResource {
        self.buffer.as_entire_binding()
    }
}