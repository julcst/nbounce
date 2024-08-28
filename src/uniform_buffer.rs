use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::common::WGPUContext;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::NoUninit)]
pub struct Uniforms {
    pub flags: u32,
    pub frame: u32,
    pub weight: f32,
    pub max_bounces: u32,
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            flags: 0,
            frame: 0,
            weight: 0.1,
            max_bounces: 8,
        }
    }
}

pub struct UniformBuffer {
    data: Uniforms,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
}

impl UniformBuffer {
    pub fn new(wgpu: &WGPUContext) -> Self {
        let data = Uniforms::default();

        let buffer = wgpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[data]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let layout = wgpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            data,
            buffer,
            bind_group,
            layout,
        }
    }

    pub fn update(&mut self, wgpu: &WGPUContext) {
        self.data.frame += 1;

        wgpu.queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.data]));
    }
}