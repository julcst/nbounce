use std::{sync::Arc, time::Instant};
use winit::window::Window;

use crate::backend::{App, ImGuiContext, WGPUContext};

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.1,
    g: 0.2,
    b: 0.3,
    a: 1.0,
};

pub struct MainApp {
    wgpu: WGPUContext,
    imgui: ImGuiContext,
    window: Arc<Window>,
    last_frame: Instant,
}

impl App for MainApp {
    async fn new(window: Arc<Window>) -> Self {
        let wgpu = WGPUContext::new(Arc::clone(&window)).await;
        let imgui = ImGuiContext::new(Arc::clone(&window), &wgpu);

        Self {
            wgpu,
            imgui,
            window,
            last_frame: Instant::now(),
        }
    }

    fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.wgpu.resize(new_size);
    }

    fn update(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame);
        self.last_frame = now;

        self.imgui.update(delta);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        
        let ui = self.imgui.frame(&self.window);
        {
            let window = ui.window("Hello world");
            window
                .size([300.0, 100.0], imgui::Condition::FirstUseEver)
                .build(|| {
                    ui.text("Hello world!");
                    ui.text("This...is...imgui-rs on WGPU!");
                    ui.separator();
                    if ui.button("Click me!") {
                        self.window.set_title("Test");
                    }
                });
        }

        // TODO: Call prepare_render here

        let frame = self.wgpu.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.wgpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            self.imgui.render(&self.wgpu, &mut rpass);
        }
    
        self.wgpu.queue.submit(Some(encoder.finish()));
        frame.present();

        Ok(())
    }
    
    fn handle_input(&mut self, event: &winit::event::WindowEvent) {
        self.imgui.handle_input(&self.window, event);
    }
}