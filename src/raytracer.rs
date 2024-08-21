use std::collections::HashMap;

use glam::{uvec2, Vec3Swizzles};

use crate::common::{CameraController, Texture, WGPUContext};
use crate::scene::SceneBuffers;

pub struct Raytracer {
    pipeline: wgpu::ComputePipeline,
    output_group: wgpu::BindGroup,
    output: Texture,
}

impl Raytracer {
    const RESOLUTION_FACTOR: f32 = 0.5;
    const COMPUTE_SIZE: u32 = 8;

    pub fn new(wgpu: &WGPUContext, scene: &SceneBuffers, camera: &CameraController) -> Self {
        let module = wgpu.device.create_shader_module(wgpu::include_wgsl!("raytracer.wgsl"));

        let output = Self::create_output_texture(wgpu);

        let output_layout = wgpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Raytracer Output Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture { 
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba16Float,
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
            push_constant_ranges: &[],
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

        Self { pipeline, output_group, output }
    }

    fn create_output_texture(wgpu: &WGPUContext) -> Texture {
        let dim = uvec2(wgpu.config.width, wgpu.config.height).as_vec2() * Self::RESOLUTION_FACTOR;
        let dim = dim.as_uvec2() / Self::COMPUTE_SIZE * Self::COMPUTE_SIZE;

        let size = wgpu::Extent3d {
            width: dim.x,
            height: dim.y,
            depth_or_array_layers: 1,
        };
        Texture::create_texture(wgpu, size, wgpu::TextureFormat::Rgba16Float)
    }

    pub fn output_texture(&self) -> &Texture {
        &self.output
    }

    pub fn resize(&mut self, wgpu: &WGPUContext) {
        self.output = Self::create_output_texture(wgpu);

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
    }

    pub fn dispatch(&self, encoder: &mut wgpu::CommandEncoder, scene: &SceneBuffers, camera: &CameraController) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Raytracer Compute Pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.output_group, &[]);
        cpass.set_bind_group(1, camera.bind_group(), &[]);
        cpass.set_bind_group(2, scene.bind_group(), &[]);
        let n_workgroups = self.output.size().xy() / Self::COMPUTE_SIZE;
        cpass.dispatch_workgroups(n_workgroups.x, n_workgroups.y, 1);
    }
}