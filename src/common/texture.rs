use wgpu::util::DeviceExt;

use super::WGPUContext;

#[derive(Debug)]
pub struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl Texture {
    pub fn create_depth(wgpu: &WGPUContext) -> Self {
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

    pub fn create_texture(wgpu: &WGPUContext, size: wgpu::Extent3d, format: wgpu::TextureFormat) -> Self {
        let desc = wgpu::TextureDescriptor {
            label: Some("Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        };

        let texture = wgpu.device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = wgpu.device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }
        );

        Self { texture, view, sampler }
    }

    pub fn create_fullscreen(wgpu: &WGPUContext, format: wgpu::TextureFormat) -> Self {
        let size = wgpu::Extent3d {
            width: wgpu.config.width,
            height: wgpu.config.height,
            depth_or_array_layers: 1,
        };

        Self::create_texture(wgpu, size, format)
    }

    pub fn create_cubemap(wgpu: &WGPUContext, bytes: &[u8]) -> Self {
        let mut image = ddsfile::Dds::read(bytes).expect("Could not read DDS file");
        let format = match image.get_dxgi_format() {
            Some(ddsfile::DxgiFormat::BC6H_UF16) => wgpu::TextureFormat::Bc6hRgbUfloat,
            _ => unimplemented!("Unsupported cubemap format"),
        };
        // Force the array size to 6, because the DDS loader doesn't set it correctly
        image.header10.as_mut().unwrap().array_size = 6;
        log::debug!("Cubemap Info: {:#?}", image);

        let size = wgpu::Extent3d {
            width: image.get_width(),
            height: image.get_height(),
            depth_or_array_layers: 6,
        };

        let mut data = Vec::new();
        for i in 0..6 {
            data.extend_from_slice(image.get_data(i).expect("Could not load cubemap layer"));
        }

        let texture = wgpu.device.create_texture_with_data(
            &wgpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("Cubemap Texture"),
                size,
                mip_level_count: image.get_num_mipmap_levels(),
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[format],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &data,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let sampler = wgpu.device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }
        );

        Self { texture, view, sampler }
    }

    pub fn from_data(wgpu: &WGPUContext, format: wgpu::TextureFormat, width: u32, height: u32, data: &[u8]) -> Self {
        let texture = wgpu.device.create_texture_with_data(
            &wgpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[format],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            data,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = wgpu.device.create_sampler(
            &wgpu::SamplerDescriptor::default(),
        );

        Self { texture, view, sampler }
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.texture.format()
    }

    pub fn size(&self) -> glam::UVec3 {
        let size = self.texture.size();
        glam::uvec3(size.width, size.height, size.depth_or_array_layers)
    }
}