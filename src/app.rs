use std::path::PathBuf;
use std::sync::Arc;

use winit::window::Window;

use crate::common::util::search_files;
use crate::common::{App, CameraController, ImGuiContext, PerformanceMetrics, Texture, WGPUContext};

use crate::pathtracing::envmap::EnvMap;
use crate::pathtracing::scene::{Scene, SceneBuffers};
use crate::pathtracing::blit_renderer::BlitRenderer;
use crate::pathtracing::mesh_renderer::MeshRenderer;
use crate::pathtracing::pathtracer::Pathtracer;

#[allow(dead_code)]
pub struct MainApp {
    wgpu: WGPUContext,
    imgui: ImGuiContext,
    window: Arc<Window>,
    metrics: PerformanceMetrics<420>,

    depth_texture: Texture,
    scene: SceneBuffers,
    envmap: EnvMap,
    fullscreen_renderer: BlitRenderer,
    mesh_renderer: MeshRenderer,
    pathtracer: Pathtracer,
    camera: CameraController,

    scenes: Vec<PathBuf>,
    scene_index: usize,
    envmaps: Vec<PathBuf>,
    envmap_index: usize,
    err_msg: String,
}

// TODO: Cleanup
impl App for MainApp {
    async fn new(window: Arc<Window>) -> Self {
        let wgpu = WGPUContext::new(Arc::clone(&window)).await;
        let imgui = ImGuiContext::new(Arc::clone(&window), &wgpu);
        let metrics = PerformanceMetrics::default();

        let scenes = search_files("assets", "glb").expect("Failed to search for scenes");
        let scene_index = 0;
        let envmaps = search_files("assets", "dds").expect("Failed to search for environment maps");
        let envmap_index = 0;

        let mut scene_data = Scene::default();
        scene_data.parse_gltf(&scenes[scene_index]).unwrap();
        let scene = SceneBuffers::from_scene(&wgpu, &mut scene_data);

        let camera = CameraController::new(&wgpu);

        let envmap = EnvMap::load(&wgpu, &envmaps[envmap_index]).expect("Failed to load environment map");

        let mesh_renderer = MeshRenderer::new(&wgpu, &camera);
        let depth_texture = Texture::create_depth(&wgpu);
        let pathtracer = Pathtracer::new(&wgpu, &scene, &camera, &envmap);
        let fullscreen_renderer = BlitRenderer::new(&wgpu, pathtracer.output_texture());

        Self {
            wgpu,
            imgui,
            window,
            metrics,
            depth_texture,
            scene,
            envmap,
            fullscreen_renderer,
            mesh_renderer,
            camera,
            pathtracer,
            scenes,
            scene_index,
            envmaps,
            envmap_index,
            err_msg: String::from("No Error"),
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
        self.pathtracer.resize(&self.wgpu);
        self.pathtracer.update(&self.wgpu, &self.camera, &self.envmap);
        self.fullscreen_renderer.set_texture(&self.wgpu, self.pathtracer.output_texture());
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

        ui.window("Settings")
            .size([1.0, 1.0], imgui::Condition::FirstUseEver)
            .always_auto_resize(true)
            .build(|| {
                ui.text(format!("Sample {}/{}", self.pathtracer.sample_count(), self.pathtracer.max_sample_count));
                if ui.slider("Res", 0.1, 1.0, &mut self.pathtracer.resolution_factor) {
                    self.pathtracer.resize(&self.wgpu);
                    self.pathtracer.update(&self.wgpu, &self.camera, &self.envmap);
                    self.fullscreen_renderer.set_texture(&self.wgpu, self.pathtracer.output_texture());
                }
                let mut updated = false;
                updated |= ui.slider("Bounces", 0, 32, &mut self.pathtracer.globals.bounces);
                let mut contribution_filtering = 1.0 / self.pathtracer.globals.contribution_factor;
                if ui.slider("Contribution Filtering", 0.0, 1.0, &mut contribution_filtering) {
                    self.pathtracer.globals.contribution_factor = 1.0 / contribution_filtering;
                    updated = true;
                }
                if updated { self.pathtracer.invalidate(); }
                if ui.combo("Scene", &mut self.scene_index, &self.scenes, |x| x.to_string_lossy()) {
                    let mut scene_data = Scene::default();
                    match scene_data.parse_gltf(&self.scenes[self.scene_index]) {
                        Ok(_) => {
                            self.scene = SceneBuffers::from_scene(&self.wgpu, &mut scene_data);
                            self.pathtracer.invalidate();
                        },
                        Err(e) => {
                            self.err_msg = e.to_string();
                            ui.open_popup("Error");
                        }
                    }
                }
                if ui.combo("Environment", &mut self.envmap_index, &self.envmaps, |x| x.to_string_lossy()) {
                    match EnvMap::load(&self.wgpu, &self.envmaps[self.envmap_index]) {
                        Ok(envmap) => {
                            self.envmap = envmap;
                            self.pathtracer.update(&self.wgpu, &self.camera, &self.envmap);
                        },
                        Err(e) => {
                            self.err_msg = e.to_string();
                            ui.open_popup("Error");
                        }
                    }
                }
                ui.modal_popup_config("Error").build(|| {
                    ui.text(self.err_msg.clone());
                    if ui.button("Close") {
                        ui.close_current_popup();
                    }
                });
        });

        if self.camera.update(&self.wgpu) {
            self.pathtracer.invalidate();
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // TODO: Call prepare_render here

        let frame = self.wgpu.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.wgpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        self.pathtracer.dispatch(&mut encoder, &self.scene);

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