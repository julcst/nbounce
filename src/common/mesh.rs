use std::{mem, path::Path};

use gltf;
use glam::{self, Vec2, Vec3, Vec4};
use wgpu::util::DeviceExt;

use super::WGPUContext;

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

#[derive(Debug)]
pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

#[derive(Debug)]
pub enum MeshError {
    Gltf(gltf::Error),
    MissingPositions,
    MissingNormals,
    MissingTexCoords,
    MissingIndices,
    NotTriangleList,
}

impl From<gltf::Error> for MeshError {
    fn from(e: gltf::Error) -> Self {
        MeshError::Gltf(e)
    }
}

impl Mesh {
    pub fn new(wgpu: &WGPUContext, path: &Path) -> Result<Self, MeshError> {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        Self::append_gltf_to_vec(path, &mut vertices, &mut indices)?;

        let vertex_buffer = wgpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );

        let index_buffer = wgpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            }
        );

        Ok(Self {
            vertices,
            indices,
            vertex_buffer,
            index_buffer,
        })
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

    fn append_gltf_to_vec(path: &Path, vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) -> Result<(), MeshError> {
        let time = std::time::Instant::now();
        let (gltf, buffers, _images) = gltf::import(path)?;
        log::info!("Loaded {:?} in {:?}", path, time.elapsed());

        let time = std::time::Instant::now();
        for mesh in gltf.meshes() {
            log::info!("Processing {:?} primitives in mesh {:?}", mesh.primitives().len(), mesh.name());
            for primitive in mesh.primitives() {
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    return Err(MeshError::NotTriangleList);
                }
                log::info!("{:?}", primitive.bounding_box());
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let start_index = vertices.len() as u32;
                let positions = reader.read_positions().ok_or(MeshError::MissingPositions)?;
                let normals = reader.read_normals().ok_or(MeshError::MissingNormals)?;
                let texcoords = reader.read_tex_coords(0).ok_or(MeshError::MissingTexCoords)?.into_f32();

                for ((position, normal), texcoord) in
                    positions.zip(normals).zip(texcoords)
                {
                    vertices.push(Vertex {
                        position: Vec3::from(position).extend(1.0),
                        normal: Vec3::from(normal).extend(0.0),
                        texcoord: Vec2::from(texcoord).extend(0.0).extend(0.0),
                    });
                }

                let gltf_indices = reader.read_indices().ok_or(MeshError::MissingIndices)?.into_u32();
                for index in gltf_indices {
                    indices.push(start_index + index);
                }
            }
        }
        log::info!("Processed {:?} in {:?}", path, time.elapsed());
        Ok(())
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.num_indices(), 0, 0..1);
    }
}