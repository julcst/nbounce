use std::collections::HashMap;
use std::time::Instant;

use glam::{uvec2, Vec3Swizzles, Vec4};
use itertools::iproduct;
use sobol_burley::sample_4d;
use wgpu::util::DeviceExt;
use wgpu::{PushConstantRange, ShaderModuleDescriptor};

use crate::common::{CameraController, Texture, WGPUContext};
use crate::scene::SceneBuffers;

pub struct Pathtracer {
    pipeline: wgpu::ComputePipeline,
    global_layout: wgpu::BindGroupLayout,
    global_group: wgpu::BindGroup,
    output: Texture,
    lds_buffer: wgpu::Buffer,
    pub globals: Globals,
    pub resolution_factor: f32,
    pub max_sample_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::NoUninit)]
pub struct Globals {
    pub sample: u32,
    weight: f32,
    pub bounces: u32,
    pub throughput: f32,
}

impl Default for Globals {
    fn default() -> Self {
        Self { 
            sample: 0,
            weight: 0.0,
            bounces: 8,
            throughput: 0.01,
        }
    }
}

impl Pathtracer {
    const COMPUTE_SIZE: u32 = 8;
    const LDS_PER_BOUNCE: u32 = 2;

    pub fn new(wgpu: &WGPUContext, scene: &SceneBuffers, camera: &CameraController) -> Self {
        let resolution_factor = 0.3;
        let output = Self::create_output_texture(wgpu, resolution_factor);

        let globals = Globals::default();
        let max_sample_count = 1024;
        let dims = globals.bounces * Self::LDS_PER_BOUNCE + 1;
        let n = max_sample_count;

        // TODO: maybe dynamically generate LDS per frame
        let timer = Instant::now();
        let lds: Vec<_> = iproduct!(0..n, 0..dims).map(|(sample_index, dimension_set)| {
            Vec4::from(sample_4d(sample_index, dimension_set, 0))
        }).collect();
        log::info!("Generated Sobol-Burley-Sequence in {:?} using {} KiB", timer.elapsed(), n * dims * 32 / 1024);

        let lds_buffer = wgpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Pathtracer LDS"),
            contents: bytemuck::cast_slice(&lds),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let global_layout = wgpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ]
        });

        let global_group = Self::create_global_group(wgpu, &global_layout, &output, camera, &lds_buffer);

        let layout = wgpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Raytracer Pipeline Layout"),
            bind_group_layouts: &[&global_layout, scene.layout()],
            push_constant_ranges: &[PushConstantRange {
                stages: wgpu::ShaderStages::COMPUTE,
                range: 0..std::mem::size_of::<Globals>() as u32,
            }],
        });

        // TODO: Maybe make unchecked in debug mode for performance
        // TODO: Refactor into macro
        let module = wgpu.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Pathtracing Shader"),
            source: wgpu::ShaderSource::Wgsl((include_str!("pathtracing.wgsl").to_owned() + include_str!("swraytracing.wgsl")).into()),
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
            global_layout,
            global_group,
            lds_buffer,
            output,
            globals,
            resolution_factor,
            max_sample_count,
        }
    }

    fn create_global_group(wgpu: &WGPUContext, global_layout: &wgpu::BindGroupLayout, output: &Texture, camera: &CameraController, lds_buffer: &wgpu::Buffer) -> wgpu::BindGroup {
        wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Raytracer Output Bind Group"),
            layout: global_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(output.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: camera.buffer_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: lds_buffer.as_entire_binding(),
                },
            ]
        })
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

    pub fn update(&mut self, wgpu: &WGPUContext, camera: &CameraController) {
        self.output = Self::create_output_texture(wgpu, self.resolution_factor);

        self.global_group = Self::create_global_group(wgpu, &self.global_layout, &self.output, camera, &self.lds_buffer);

        self.invalidate();
    }

    pub fn sample_count(&self) -> u32 {
        self.globals.sample
    }

    pub fn invalidate(&mut self) {
        self.globals.sample = 0;
    }

    pub fn dispatch(&mut self, encoder: &mut wgpu::CommandEncoder, scene: &SceneBuffers) {
        if self.globals.sample >= self.max_sample_count { return; }
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Raytracer Compute Pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.global_group, &[]);
        cpass.set_bind_group(1, scene.bind_group(), &[]);
        self.globals.sample += 1;
        self.globals.weight = 1.0 / self.globals.sample as f32;
        cpass.set_push_constants(0, bytemuck::cast_slice(&[self.globals]));
        let n_workgroups = self.output.size().xy() / Self::COMPUTE_SIZE;
        cpass.dispatch_workgroups(n_workgroups.x, n_workgroups.y, 1);
    }
}