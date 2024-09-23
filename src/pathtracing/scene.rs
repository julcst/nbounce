use std::collections::HashMap;
use std::{mem, ops::Range, path::Path};

use glam::{Mat4, Vec3, Vec4};
use itertools::izip;
use wgpu::util::DeviceExt;

use super::bvh::{self, BVHPrimitive, BVHTree};

use crate::common::WGPUContext;

// TODO: Benchmark best layout
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::NoUninit)]
pub struct Vertex {
    pub position: Vec3,
    pub u: f32,
    pub normal: Vec3,
    pub v: f32,
    pub tangent: Vec4,
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
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: mem::offset_of!(Vertex, tangent) as wgpu::BufferAddress,
                    shader_location: 4,
                },
            ],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Primitive {
    local_to_world: Mat4,
    color: Vec4,
    roughness: f32,
    metallic: f32,
    emissive: f32,
    index_range: Range<u32>,
}

#[derive(Debug)]
pub enum MeshError {
    Gltf(gltf::Error),
    MissingPositions,
    MissingNormals,
    MissingIndices,
    MissingTangents,
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
            MeshError::MissingIndices => write!(f, "Missing indices"),
            MeshError::MissingTangents => write!(f, "Tangent generation is not yet supported"),
            MeshError::NotTriangleList => write!(f, "Not a triangle list"),
        }
    }
}

impl std::error::Error for MeshError {}

#[derive(Default)]
pub struct Scene {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    primitives: Vec<Primitive>,
}

impl Scene {
    pub fn parse_gltf(&mut self, path: &Path) -> Result<(), MeshError> {
        let time = std::time::Instant::now();
        let (gltf, buffers, _images) = gltf::import(path)?;
        log::info!("Loaded {:?} in {:?}", path, time.elapsed());
        //log::info!("GLTF: {:#?}", gltf);

        let time = std::time::Instant::now();

        // let mut textures = Vec::new();
        // 
        // for texture in gltf.textures() {
        //     let image = _images.get(texture.source().index()).unwrap();
        //     let format = match image.format {
        //         gltf::image::Format::R8 => wgpu::TextureFormat::R8Unorm,
        //         gltf::image::Format::R8G8 => wgpu::TextureFormat::Rg8Unorm,
        //         gltf::image::Format::R8G8B8A8 => wgpu::TextureFormat::Rgba8Unorm,
        //         gltf::image::Format::R16 => wgpu::TextureFormat::R16Unorm,
        //         gltf::image::Format::R16G16 => wgpu::TextureFormat::Rg16Unorm,
        //         gltf::image::Format::R16G16B16A16 => wgpu::TextureFormat::Rgba16Unorm,
        //         gltf::image::Format::R32G32B32A32FLOAT => wgpu::TextureFormat::Rgba32Float,
        //         _ => unimplemented!(),
        //     };
        //     textures.push(Texture::from_data(&wgpu, format, image.width, image.height, &image.pixels))
        // }

        // Maps primitive index -> index range
        let mut geometry_map = HashMap::new();

        for mesh in gltf.meshes() {
            log::debug!("Processing {:?} primitives in mesh {:?}", mesh.primitives().len(), mesh.name());
            for primitive in mesh.primitives() {
                // if let texture = primitive.material().pbr_metallic_roughness().base_color_texture() {
                //     let texture = Texture::from_gltf(image, &images, &WGPUContext::new());
                // }
                // log::debug!("{:#?}", primitive.material().pbr_metallic_roughness().base_color_texture().unwrap().texture().source().index());
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    return Err(MeshError::NotTriangleList);
                }
                
                log::debug!("{:?}", primitive.bounding_box());
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let positions = reader.read_positions().ok_or(MeshError::MissingPositions)?;
                let normals = reader.read_normals().ok_or(MeshError::MissingNormals)?;

                let texcoords: &mut dyn Iterator<Item = _> = match reader.read_tex_coords(0) {
                    Some(t) => &mut t.into_f32(),
                    None => &mut std::iter::repeat([0.0, 0.0]),
                };

                let start_vertex = self.vertices.len() as u32;
                let start_index = self.indices.len() as u32;

                if let Some(tangents) = reader.read_tangents() {
                    for (position, normal, texcoord, tangent) in izip!(positions, normals, texcoords, tangents) {
                        self.vertices.push(Vertex {
                            position: Vec3::from(position),
                            u: texcoord[0],
                            normal: Vec3::from(normal),
                            v: texcoord[1],
                            tangent: Vec4::from(tangent),
                        });
                    }

                    let indices = reader.read_indices().ok_or(MeshError::MissingIndices)?.into_u32();
                    self.indices.extend(indices.map(|i| i + start_vertex));
                } else {
                    // TODO: Generate tangents using mikktspace
                    return Err(MeshError::MissingTangents);
                }

                geometry_map.insert((mesh.index(), primitive.index()) , start_index..self.indices.len() as u32);
            }
        }

        //log::debug!("Primitives: {:#?}", geometry_map);
        
        for node in gltf.nodes() {
            if let Some(mesh) = node.mesh() {
                let local_to_world = Mat4::from_cols_array_2d(&node.transform().matrix());

                for primitive in mesh.primitives() {
                    let material = primitive.material();
                    let emissive = Vec3::from(material.emissive_factor());
                    let is_emissive = emissive != Vec3::ZERO;
                    let color = if is_emissive {
                        emissive.extend(1.0)
                    } else {
                        Vec4::from_array(material.pbr_metallic_roughness().base_color_factor())
                    };
                    let index_range = geometry_map.get(&(mesh.index(), primitive.index())).unwrap().to_owned();
                    self.primitives.push(Primitive { 
                        index_range,
                        local_to_world,
                        color,
                        roughness: material.pbr_metallic_roughness().roughness_factor(),
                        metallic: material.pbr_metallic_roughness().metallic_factor(),
                        emissive: if is_emissive {1.0} else {0.0},
                    });
                }
            } else {
                log::info!("Skipped non-mesh node {:?}", node.name());
            }
        }

        log::debug!("Scene: {:#?}", self.primitives);

        log::info!("Processed {:?} in {:?}", path, time.elapsed());
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::NoUninit)]
struct Instance {
    world_to_local: Mat4,
    local_to_world: Mat4,
    color: Vec4,
    roughness: f32,
    metallic: f32,
    emissive: f32,
    node: u32,
}

struct InstanceWithBounds {
    instance: Instance,
    world_min: Vec3,
    world_max: Vec3,
}

impl InstanceWithBounds {
    fn approximate_from_instance(instance: Instance, local_min: Vec3, local_max: Vec3) -> Self {
        // Transform all 8 corners of the local bounds to world space and find the new bounds
        let mut world_min = Vec3::splat(f32::INFINITY);
        let mut world_max = Vec3::splat(f32::NEG_INFINITY);
        let local_to_world = instance.local_to_world;
        for i in 0..8u8 {
            let local = Vec3::new(
                if i & 1 == 0 { local_min.x } else { local_max.x },
                if i & 2 == 0 { local_min.y } else { local_max.y },
                if i & 4 == 0 { local_min.z } else { local_max.z },
            );
            let world = local_to_world.transform_point3(local);
            world_min = world_min.min(world);
            world_max = world_max.max(world);
        }
        Self {
            instance,
            world_min,
            world_max,
        }
    }
}

impl BVHPrimitive for InstanceWithBounds {
    fn min(&self) -> Vec3 {
        self.world_min
    }
    fn max(&self) -> Vec3 {
        self.world_max
    }
}

pub struct SceneBuffers {
    primitives: Vec<Primitive>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
}

impl SceneBuffers {
    pub fn from_scene(wgpu: &WGPUContext, scene: &mut Scene) -> Self {
        let mut triangles = bvh::build_triangle_cache(&scene.vertices, &scene.indices);
        let mut instances = Vec::new();

        let mut blas = BVHTree::default();
        for primitive in &scene.primitives {
            let triangle_range = primitive.index_range.start / 3..primitive.index_range.end / 3;
            let node = blas.append(&mut triangles, triangle_range);
            let local_min = blas.nodes()[node as usize].min;
            let local_max = blas.nodes()[node as usize].max;
            instances.push(InstanceWithBounds::approximate_from_instance(Instance {
                world_to_local: primitive.local_to_world.inverse(),
                local_to_world: primitive.local_to_world,
                color: primitive.color,
                roughness: primitive.roughness,
                metallic: primitive.metallic,
                emissive: primitive.emissive,
                node,
            }, local_min, local_max));
        }

        let range = 0..instances.len() as u32;
        let tlas = bvh::build_bvh(&mut instances, range);

        // Apply triangle permutation to indices
        bvh::flatten_triangle_list(&triangles, &mut scene.indices);

        let stripped_instances: Vec<_> = instances.into_iter().map(|i| i.instance).collect();

        let blas_buffer = wgpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BLAS Nodes"),
            contents: bytemuck::cast_slice(blas.nodes()),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let tlas_buffer = wgpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TLAS Nodes"),
            contents: bytemuck::cast_slice(tlas.nodes()),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let instance_buffer = wgpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instances"),
            contents: bytemuck::cast_slice(&stripped_instances),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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
                        buffer: &blas_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &tlas_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &instance_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &vertex_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &index_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        Self {
            primitives: scene.primitives.clone(),
            vertex_buffer,
            index_buffer,
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
        for primitive in &self.primitives {
            render_pass.draw_indexed(primitive.index_range.clone(), 0, 0..1);
        }
    }
}