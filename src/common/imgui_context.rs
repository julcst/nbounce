use std::sync::Arc;

use winit::{event::WindowEvent, window::Window};

use super::WGPUContext;

pub struct ImGuiContext {
    pub renderer: imgui_wgpu::Renderer,
    pub ctx: imgui::Context,
    pub platform: imgui_winit_support::WinitPlatform,
}

impl ImGuiContext {
    pub fn new(window: Arc<Window>, wgpu: &WGPUContext) -> Self {
        let hidpi_factor = window.scale_factor();
        let mut ctx = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut ctx);
        platform.attach_window(
            ctx.io_mut(),
            &window,
            imgui_winit_support::HiDpiMode::Default,
        );
        ctx.set_ini_filename(None);

        let font_size = (9.0 * hidpi_factor) as f32;
        ctx.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        ctx.fonts().add_font(&[imgui::FontSource::DefaultFontData {
            config: Some(imgui::FontConfig {
                oversample_h: 1,
                pixel_snap_h: true,
                size_pixels: font_size,
                ..Default::default()
            }),
        }]);

        // Set up dear imgui wgpu renderer
        let renderer_config = imgui_wgpu::RendererConfig {
            texture_format: wgpu.config.format,
            depth_format: Some(wgpu::TextureFormat::Depth32Float),
            font_atlas_format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            ..Default::default()
        };

        let renderer = imgui_wgpu::Renderer::new(&mut ctx, &wgpu.device, &wgpu.queue, renderer_config);

        Self {
            renderer,
            ctx,
            platform,
        }
    }

    pub fn update(&mut self, delta: std::time::Duration) {
        self.ctx.io_mut().update_delta_time(delta);
    }

    pub fn frame(&mut self, window: &Window) -> &mut imgui::Ui {
        self.platform
            .prepare_frame(self.ctx.io_mut(), window)
            .expect("Failed to prepare ImGui frame");
        self.ctx.frame()
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) {
        self.platform.handle_window_event(self.ctx.io_mut(), window, event);
    }

    pub fn prepare_render(&mut self, ui: &imgui::Ui, window: &Window) {
        self.platform.prepare_render(ui, window)
    }

    pub fn render<'r>(&'r mut self, wgpu: &WGPUContext, render_pass: &mut wgpu::RenderPass<'r>) {
        self.renderer
            .render(self.ctx.render(), &wgpu.queue, &wgpu.device, render_pass)
            .expect("ImGui Rendering failed")
    }
}