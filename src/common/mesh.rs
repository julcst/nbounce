use std::{mem, path::Path};

use gltf;
use glam::{self, Vec2, Vec3, Vec4};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::NoUninit)]
pub struct Vertex {
    position: Vec4,
    normal: Vec4,
    texcoord: Vec4,
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: mem::offset_of!(Vertex, position) as wgpu::BufferAddress,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: mem::offset_of!(Vertex, normal) as wgpu::BufferAddress,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: mem::offset_of!(Vertex, texcoord) as wgpu::BufferAddress,
                    shader_location: 2,
                },
            ],
        }
    }
}

#[derive(Debug, Default)]
pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

#[derive(Debug)]
pub enum MeshError {
    Gltf(gltf::Error),
    MissingPositions,
    MissingNormals,
    MissingTexCoords,
    MissingIndices,
}

impl From<gltf::Error> for MeshError {
    fn from(e: gltf::Error) -> Self {
        MeshError::Gltf(e)
    }
}

impl Mesh {
    pub fn new(path: &Path) -> Result<Self, MeshError> {
        let mut mesh = Self::default();
        mesh.attach_gltf(path)?;
        Ok(mesh)
    }

    pub fn vertices_as_u8(&self) -> &[u8] {
        bytemuck::cast_slice(&self.vertices)
    }

    pub fn indices_as_u8(&self) -> &[u8] {
        bytemuck::cast_slice(&self.indices)
    }

    pub fn num_indices(&self) -> u32 {
        self.indices.len() as u32
    }

    pub fn attach_gltf(&mut self, path: &Path) -> Result<(), MeshError> {
        let (gltf, buffers, _images) = gltf::import(path)?;

        log::info!("{:#?}", gltf);

        for mesh in gltf.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let start_index = self.vertices.len() as u32;
                let positions = reader.read_positions().ok_or(MeshError::MissingPositions)?;
                let normals = reader.read_normals().ok_or(MeshError::MissingNormals)?;
                let texcoords = reader.read_tex_coords(0).ok_or(MeshError::MissingTexCoords)?.into_f32();

                for ((position, normal), texcoord) in
                    positions.zip(normals).zip(texcoords)
                {
                    self.vertices.push(Vertex {
                        position: Vec3::from(position).extend(1.0),
                        normal: Vec3::from(normal).extend(0.0),
                        texcoord: Vec2::from(texcoord).extend(0.0).extend(0.0),
                    });
                }

                let indices = reader.read_indices().ok_or(MeshError::MissingIndices)?.into_u32();
                for index in indices {
                    self.indices.push(start_index + index);
                }
            }
        }
        Ok(())
    }
}