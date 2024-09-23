use std::path::PathBuf;

use crate::common::{Texture, WGPUContext};

pub struct EnvMap {
    texture: Texture,
}

// TODO: Get skyboxes from git repo

impl EnvMap {
    pub fn load(wgpu: &WGPUContext, path: &PathBuf) -> Result<Self, std::io::Error> {
        let bytes = std::fs::read(path)?;
        let texture = Texture::create_cubemap(wgpu, bytes.as_slice());

        Ok(Self { texture })
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.texture.view()
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        self.texture.sampler()
    }
}