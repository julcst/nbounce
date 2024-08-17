use crate::common::{CameraController, WGPUContext};
use crate::scene::{SceneBindGroup, Vertex};

pub struct MeshRenderer {
    pipeline: wgpu::RenderPipeline,
}

impl MeshRenderer {
    pub fn new(wgpu: &WGPUContext, camera: &CameraController) -> Self {
        let shader = wgpu.device.create_shader_module(wgpu::include_wgsl!("mesh.wgsl"));

        let pipeline_layout = wgpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mesh Pipeline Layout"),
            bind_group_layouts: &[
                camera.layout(),
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
        }
    }

    pub fn render<'r>(&'r self, render_pass: &mut wgpu::RenderPass<'r>, scene: &SceneBindGroup, camera: &CameraController) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera.bind_group(), &[]);
        scene.draw(render_pass);
    }
}