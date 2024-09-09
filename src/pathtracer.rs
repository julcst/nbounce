use std::collections::HashMap;

use glam::{uvec2, Vec3Swizzles};
use wgpu::{PushConstantRange, ShaderModuleDescriptor};

use crate::common::{CameraController, Texture, WGPUContext};
use crate::scene::SceneBuffers;

pub struct Pathtracer {
    pipeline: wgpu::ComputePipeline,
    output_group: wgpu::BindGroup,
    output: Texture,
    pub uniforms: Uniforms,
    sample_count: f32,
    pub resolution_factor: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::NoUninit)]
pub struct Uniforms {
    frame: u32,
    weight: f32,
    pub bounces: u32,
    pub throughput: f32,
}

impl Default for Uniforms {
    fn default() -> Self {
        Self { 
            frame: 0,
            weight: 0.0,
            bounces: 8,
            throughput: 0.01,
        }
    }
}

impl Pathtracer {
    const COMPUTE_SIZE: u32 = 8;

    pub fn new(wgpu: &WGPUContext, scene: &SceneBuffers, camera: &CameraController) -> Self {
        // TODO: Refactor into macro
        let module = wgpu.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Pathtracing Shader"),
            source: wgpu::ShaderSource::Wgsl((include_str!("pathtracing.wgsl").to_owned() + include_str!("swraytracing.wgsl")).into()),
        });

        let resolution_factor = 0.3;
        let output = Self::create_output_texture(wgpu, resolution_factor);

        let output_layout = wgpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Raytracer Output Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture { 
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ]
        });

        let output_group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Raytracer Output Bind Group"),
            layout: &output_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(output.view()),
                },
            ]
        });

        let layout = wgpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Raytracer Pipeline Layout"),
            bind_group_layouts: &[&output_layout, camera.layout(), scene.layout()],
            push_constant_ranges: &[PushConstantRange {
                stages: wgpu::ShaderStages::COMPUTE,
                range: 0..std::mem::size_of::<Uniforms>() as u32,
            }],
        });

        let pipeline = wgpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Raytracer Compute"),
            layout: Some(&layout),
            module: &module,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &HashMap::from([
                    // (String::from("COMPUTE_SIZE"), Self::COMPUTE_SIZE as f64)
                ]),
                zero_initialize_workgroup_memory: false,
                vertex_pulling_transform: false,
            },
            cache: None,
        });

        Self { 
            pipeline,
            output_group,
            output,
            uniforms: Uniforms::default(),
            sample_count: 0.0,
            resolution_factor,
        }
    }

    fn create_output_texture(wgpu: &WGPUContext, resolution_factor: f32) -> Texture {
        let dim = uvec2(wgpu.config.width, wgpu.config.height).as_vec2() * resolution_factor;
        let dim = dim.as_uvec2() / Self::COMPUTE_SIZE * Self::COMPUTE_SIZE;

        let size = wgpu::Extent3d {
            width: dim.x,
            height: dim.y,
            depth_or_array_layers: 1,
        };
        Texture::create_texture(wgpu, size, wgpu::TextureFormat::Rgba32Float)
    }

    pub fn output_texture(&self) -> &Texture {
        &self.output
    }

    pub fn resize(&mut self, wgpu: &WGPUContext) {
        self.output = Self::create_output_texture(wgpu, self.resolution_factor);

        self.output_group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blit Bind Group"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(self.output.view()),
                },
            ]
        });

        self.invalidate();
    }

    pub fn sample_count(&self) -> u32 {
        self.sample_count as u32
    }

    pub fn invalidate(&mut self) {
        self.sample_count = 0.0;
    }

    pub fn dispatch(&mut self, encoder: &mut wgpu::CommandEncoder, scene: &SceneBuffers, camera: &CameraController) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Raytracer Compute Pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.output_group, &[]);
        cpass.set_bind_group(1, camera.bind_group(), &[]);
        cpass.set_bind_group(2, scene.bind_group(), &[]);
        self.sample_count += 1.0;
        self.uniforms.frame += 1;
        self.uniforms.weight = 1.0 / self.sample_count;
        cpass.set_push_constants(0, bytemuck::cast_slice(&[self.uniforms]));
        let n_workgroups = self.output.size().xy() / Self::COMPUTE_SIZE;
        cpass.dispatch_workgroups(n_workgroups.x, n_workgroups.y, 1);
    }
}