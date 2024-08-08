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

    pub fn create_fullscreen(wgpu: &WGPUContext, format: wgpu::TextureFormat) -> Self {
        let size = wgpu::Extent3d {
            width: wgpu.config.width,
            height: wgpu.config.height,
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some("Fullscreen Texture"),
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
            &wgpu::SamplerDescriptor::default(),
        );

        Self { texture, view, sampler }
    }

    pub fn from_gltf(wgpu: &WGPUContext, texture: &gltf::Texture, images: &Vec<gltf::image::Data>) -> Self {
        let image = &images[0];

        let format = match image.format {
            gltf::image::Format::R8 => wgpu::TextureFormat::R8Unorm,
            gltf::image::Format::R8G8 => wgpu::TextureFormat::Rg8Unorm,
            gltf::image::Format::R8G8B8A8 => wgpu::TextureFormat::Rgba8Unorm,
            gltf::image::Format::R16 => wgpu::TextureFormat::R16Unorm,
            gltf::image::Format::R16G16 => wgpu::TextureFormat::Rg16Unorm,
            gltf::image::Format::R16G16B16A16 => wgpu::TextureFormat::Rgba16Unorm,
            gltf::image::Format::R32G32B32A32FLOAT => wgpu::TextureFormat::Rgba32Float,
            _ => unimplemented!(),
        };

        let texture = wgpu.device.create_texture_with_data(
            &wgpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("Texture"),
                size: wgpu::Extent3d {
                    width: image.width,
                    height: image.height,
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
            image.pixels.as_slice(),
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
}