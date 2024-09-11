use crate::common::{CameraController, WGPUContext};
use crate::scene::{SceneBuffers, Vertex};

pub struct MeshRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_group: wgpu::BindGroup,
}

impl MeshRenderer {
    pub fn new(wgpu: &WGPUContext, camera: &CameraController) -> Self {
        let shader = wgpu.device.create_shader_module(wgpu::include_wgsl!("mesh.wgsl"));

        // Note: It is common to have 4 bind groups:
        // 0 - per frame, engine globals
        // 1 - per pass
        // 2 - per material
        // 3 - per object

        let uniform_layout = wgpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.buffer_binding(),
            }],
        });

        let pipeline_layout = wgpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mesh Pipeline Layout"),
            bind_group_layouts: &[
                &uniform_layout,
            ],
            push_constant_ranges: &[],
        });

        let pipeline = wgpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mesh Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu.config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            uniform_group,
        }
    }

    pub fn render<'r>(&'r self, render_pass: &mut wgpu::RenderPass<'r>, scene: &SceneBuffers) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.uniform_group, &[]);
        scene.draw(render_pass);
    }
}