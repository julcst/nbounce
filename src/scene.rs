use std::{mem, ops::Range, path::Path};

use glam::{self, Vec2, Vec3, Vec4};
use wgpu::util::DeviceExt;

use crate::bvh::BVHTree;

use crate::common::WGPUContext;

// TODO: Benchmark best layout
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::NoUninit)]
pub struct Vertex {
    pub position: Vec3,
    pub u: f32,
    pub normal: Vec3,
    pub v: f32,
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: mem::offset_of!(Vertex, position) as wgpu::BufferAddress,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: mem::offset_of!(Vertex, u) as wgpu::BufferAddress,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: mem::offset_of!(Vertex, normal) as wgpu::BufferAddress,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: mem::offset_of!(Vertex, v) as wgpu::BufferAddress,
                    shader_location: 3,
                },
            ],
        }
    }
}

pub struct Instance {
    indices: Range<u32>,
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

impl std::fmt::Display for MeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MeshError::Gltf(e) => write!(f, "Gltf error: {}", e),
            MeshError::MissingPositions => write!(f, "Missing positions"),
            MeshError::MissingNormals => write!(f, "Missing normals"),
            MeshError::MissingTexCoords => write!(f, "Missing texcoords"),
            MeshError::MissingIndices => write!(f, "Missing indices"),
            MeshError::NotTriangleList => write!(f, "Not a triangle list"),
        }
    }
}

impl std::error::Error for MeshError {}

#[derive(Default)]
pub struct Scene {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    instances: Vec<Instance>,
}

impl Scene {
    pub fn parse_gltf(&mut self, path: &Path) -> Result<(), MeshError> {
        let time = std::time::Instant::now();
        let (gltf, buffers, _images) = gltf::import(path)?;
        log::info!("Loaded {:?} in {:?}", path, time.elapsed());

        let time = std::time::Instant::now();

        for mesh in gltf.meshes() {
            log::info!("Processing {:?} primitives in mesh {:?}", mesh.primitives().len(), mesh.name());
            for primitive in mesh.primitives() {
                // if let texture = primitive.material().pbr_metallic_roughness().base_color_texture() {
                //     let texture = Texture::from_gltf(image, &images, &WGPUContext::new());
                // }
                // log::info!("{:#?}", primitive.material().pbr_metallic_roughness().base_color_texture().unwrap().texture().source().index());
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    return Err(MeshError::NotTriangleList);
                }
                
                log::info!("{:?}", primitive.bounding_box());
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let start_index = self.vertices.len() as u32;
                let positions = reader.read_positions().ok_or(MeshError::MissingPositions)?;
                let normals = reader.read_normals().ok_or(MeshError::MissingNormals)?;
                let texcoords = reader.read_tex_coords(0).ok_or(MeshError::MissingTexCoords)?.into_f32();

                for ((position, normal), texcoord) in positions.zip(normals).zip(texcoords) {
                    self.vertices.push(Vertex {
                        position: Vec3::from(position),
                        u: texcoord[0],
                        normal: Vec3::from(normal),
                        v: texcoord[1],
                    });
                }

                let gltf_indices = reader.read_indices().ok_or(MeshError::MissingIndices)?.into_u32();
                self.indices.extend(gltf_indices);

                self.instances.push(Instance { indices: start_index..self.indices.len() as u32 })
            }
        }
        log::info!("Processed {:?} in {:?}", path, time.elapsed());
        Ok(())
    }
}


pub struct SceneBindGroup {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    indices: Range<u32>,
    group: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
}

impl SceneBindGroup {
    pub fn from_scene(wgpu: &WGPUContext, scene: &mut Scene) -> Self {
        let bvh = BVHTree::build_bvh(&scene.vertices, &mut scene.indices);
        let indices = 0..scene.indices.len() as u32;

        let nodes = wgpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BVH Nodes"),
            contents: bytemuck::cast_slice(bvh.nodes()),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let vertex_buffer = wgpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&scene.vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            }
        );

        let index_buffer = wgpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&scene.indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
            }
        );

        let layout = wgpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BVH Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BVH Bind Group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &nodes,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &vertex_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &index_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        Self {
            vertex_buffer,
            index_buffer,
            indices,
            group,
            layout,
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.group
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(self.indices.clone(), 0, 0..1);
    }
}