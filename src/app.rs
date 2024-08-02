use std::sync::Arc;

use winit::window::Window;

use crate::common::{App, ImGuiContext, PerformanceMetrics, WGPUContext};

use crate::triangle_pipeline::TrianglePipeline;

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
    metrics: PerformanceMetrics<120>,
    pipeline: TrianglePipeline,
}

impl App for MainApp {
    async fn new(window: Arc<Window>) -> Self {
        let wgpu = WGPUContext::new(Arc::clone(&window)).await;
        let imgui = ImGuiContext::new(Arc::clone(&window), &wgpu);
        let pipeline = TrianglePipeline::new(&wgpu);

        Self {
            wgpu,
            imgui,
            window,
            metrics: PerformanceMetrics::default(),
            pipeline,
        }
    }

    fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.wgpu.resize(new_size);
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
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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
            self.pipeline.render(&mut rpass);
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