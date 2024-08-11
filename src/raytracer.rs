use std::collections::HashMap;

use glam::{uvec2, Vec3Swizzles};

use crate::common::{Texture, WGPUContext};

pub struct Raytracer {
    pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
    output: Texture,
}

impl Raytracer {
    const COMPUTE_SIZE: u32 = 16;

    pub fn new(wgpu: &WGPUContext) -> Self {
        let module = wgpu.device.create_shader_module(wgpu::include_wgsl!("raytracer.wgsl"));

        let output = Self::create_output_texture(wgpu);

        let bind_group_layout = wgpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bind_group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Raytracer Output Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(output.view()),
                },
            ]
        });

        let layout = wgpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Raytracer Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = wgpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Raytracer Compute"),
            layout: Some(&layout),
            module: &module,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &HashMap::from([
                    (String::from("COMPUTE_SIZE"), Self::COMPUTE_SIZE as f64)
                ]),
                zero_initialize_workgroup_memory: false,
                vertex_pulling_transform: false,
            },
            cache: None,
        });

        Self { pipeline, bind_group, output }
    }

    fn create_output_texture(wgpu: &WGPUContext) -> Texture {
        let dim = uvec2(wgpu.config.width, wgpu.config.height).as_vec2() * 0.2;
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

        self.bind_group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
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

    pub fn dispatch(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Raytracer Compute Pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        let n_workgroups = self.output.size().xy() / Self::COMPUTE_SIZE;
        cpass.dispatch_workgroups(n_workgroups.x, n_workgroups.y, 1);
    }
}