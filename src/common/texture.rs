use super::WGPUContext;

pub struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl Texture {
    pub fn create_depth_texture(wgpu: &WGPUContext) -> Self {
        let size = wgpu::Extent3d {
            width: wgpu.config.width,
            height: wgpu.config.height,
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let texture = wgpu.device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = wgpu.device.create_sampler(
            &wgpu::SamplerDescriptor {
                compare: Some(wgpu::CompareFunction::LessEqual),
                ..Default::default()
            }
        );

        Self { texture, view, sampler }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.texture.format()
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}