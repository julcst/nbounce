use std::path::Path;
use std::sync::Arc;

use winit::window::Window;

use crate::scene::{Scene, SceneBuffers};
use crate::common::{App, CameraController, ImGuiContext, PerformanceMetrics, Texture, WGPUContext};

use crate::blit_renderer::BlitRenderer;
use crate::mesh_renderer::MeshRenderer;
use crate::raytracer::Raytracer;

#[allow(dead_code)]
pub struct MainApp {
    wgpu: WGPUContext,
    imgui: ImGuiContext,
    window: Arc<Window>,
    metrics: PerformanceMetrics<420>,

    depth_texture: Texture,
    scene: SceneBuffers,
    fullscreen_renderer: BlitRenderer,
    mesh_renderer: MeshRenderer,
    raytracer: Raytracer,
    camera: CameraController,
}

impl App for MainApp {
    async fn new(window: Arc<Window>) -> Self {
        let wgpu = WGPUContext::new(Arc::clone(&window)).await;
        let imgui = ImGuiContext::new(Arc::clone(&window), &wgpu);
        let metrics = PerformanceMetrics::default();

        let mut scene_data = Scene::default();
        scene_data.parse_gltf(Path::new("assets/testscene.glb")).unwrap();
        let timer = std::time::Instant::now();
        scene_data.gen_tangents().unwrap();
        log::info!("Generated tangents in {:?}", timer.elapsed());
        let scene = SceneBuffers::from_scene(&wgpu, &mut scene_data);

        let camera = CameraController::new(&wgpu);

        let mesh_renderer = MeshRenderer::new(&wgpu, &camera);
        let depth_texture = Texture::create_depth(&wgpu);
        let raytracer = Raytracer::new(&wgpu, &scene, &camera);
        let fullscreen_renderer = BlitRenderer::new(&wgpu, raytracer.output_texture());

        Self {
            wgpu,
            imgui,
            window,
            metrics,
            depth_texture,
            scene,
            fullscreen_renderer,
            mesh_renderer,
            camera,
            raytracer,
        }
    }

    fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == self.wgpu.config.width && new_size.height == self.wgpu.config.height {
            log::info!("Skipping unnecessary resize");
            return;
        }
        self.wgpu.resize(new_size);
        self.depth_texture = Texture::create_depth(&self.wgpu);
        self.raytracer.resize(&self.wgpu);
        self.fullscreen_renderer.set_texture(&self.wgpu, self.raytracer.output_texture());
    }

    fn update(&mut self) {
        self.metrics.next_frame();

        self.imgui.update(self.metrics.curr_frame_time());
        let ui = self.imgui.frame(&self.window);

        ui.window("Performance Metrics")
            .title_bar(false)
            .size([1.0, 1.0], imgui::Condition::FirstUseEver)
            .position([0.0, 0.0], imgui::Condition::FirstUseEver)
            .always_auto_resize(true)
            .movable(false)
            .no_inputs()
            .build(|| {
                ui.text(format!("{:.2?} ({:.2?}) {:.2?} ({:.2?}) {}x{}",
                    self.metrics.avg_frame_time(),
                    self.metrics.curr_frame_time(),
                    self.metrics.avg_frame_rate(),
                    self.metrics.curr_frame_rate(),
                    self.window.inner_size().width,
                    self.window.inner_size().height));
            });

        self.camera.update(&self.wgpu);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // TODO: Call prepare_render here

        let frame = self.wgpu.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.wgpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        self.raytracer.dispatch(&mut encoder, &self.scene, &self.camera);

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.depth_texture.view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.fullscreen_renderer.render(&mut rpass);
            //self.mesh_renderer.render(&mut rpass, &self.scene, &self.camera);
            self.imgui.render(&self.wgpu, &mut rpass);
        }
    
        self.wgpu.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
    
    fn window_event(&mut self, event: &winit::event::WindowEvent) {
        self.imgui.handle_input(&self.window, event);
        self.camera.window_event(event);
    }

    fn device_event(&mut self, event: &winit::event::DeviceEvent) {
        self.camera.device_event(event);
    }
}