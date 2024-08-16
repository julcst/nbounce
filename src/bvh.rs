use glam::{UVec3, Vec3, Vec4Swizzles};
use wgpu::util::DeviceExt;

use crate::common::{Mesh, Vertex};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::NoUninit)]
pub struct BVHNode {
    min: Vec3,
    /// If this is a leaf node, this is the index of the first triangle index.
    /// If this is an inner node, this is the index of the left child node.
    index: u32,
    max: Vec3,
    count: u32,
}

impl BVHNode {
    fn is_leaf(&self) -> bool {
        self.count > 0
    }

    fn make_inner(&mut self, left_child: u32) {
        self.count = 0; // Mark as inner node
        self.index = left_child;
    }

    fn include(&mut self, triangle: &Triangle) {
        self.min = self.min.min(triangle.min);
        self.max = self.max.max(triangle.max);
    }

    fn cost(&self) -> f32 {
        let extent = self.max - self.min;
        if extent.is_finite() {
            let area = extent.x * extent.y + extent.x * extent.z + extent.y * extent.z;
            self.count as f32 * area
        } else {
            f32::INFINITY
        }
    }
}

pub struct BVHTree {
    nodes: Vec<BVHNode>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

struct Triangle {
    center: Vec3,
    min: Vec3,
    max: Vec3,
    indices: [u32; 3],
}

#[derive(Clone, Copy)]
struct Bin {
    min: Vec3,
    max: Vec3,
    count: u32,
}

impl Default for Bin {
    fn default() -> Self {
        Self {
            min: Vec3::INFINITY,
            max: Vec3::NEG_INFINITY,
            count: 0,
        }
    }
}

impl Bin {
    fn include(&mut self, triangle: &Triangle) {
        self.min = self.min.min(triangle.min);
        self.max = self.max.max(triangle.max);
        self.count += 1;
    }

    fn include_bin(&mut self, other: &Self) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
        self.count += other.count;
    }

    fn cost(&self) -> f32 {
        let extent = self.max - self.min;
        if extent.is_finite() {
            let area = extent.x * extent.y + extent.x * extent.z + extent.y * extent.z;
            self.count as f32 * area
        } else {
            f32::INFINITY
        }
    }
}

pub struct Split {
    axis: usize,
    mid: f32,
}

impl BVHTree {
    const MAX_DEPTH: u32 = 32;

    pub fn build_bvh(mesh: &Mesh) -> Self {
        let timer = std::time::Instant::now();
        let mut nodes = Vec::new();
        let vertices = mesh.vertices().to_vec();
        let mut triangles = Vec::with_capacity(mesh.num_indices() as usize / 3);

        for indices in mesh.indices().chunks(3) {
            let v0 = vertices[indices[0] as usize].position.xyz();
            let v1 = vertices[indices[1] as usize].position.xyz();
            let v2 = vertices[indices[2] as usize].position.xyz();
            let center = (v0 + v1 + v2) / 3.0;
            let min = v0.min(v1).min(v2);
            let max = v0.max(v1).max(v2);
            triangles.push(Triangle {center, min, max, indices: indices.try_into().unwrap()});
        }
        log::info!("Built triangle cache in {:?}", timer.elapsed());

        let mut stack = Vec::new();

        let parent = Self::build_leaf(&triangles, 0, triangles.len() as u32);
        nodes.push(parent);
        stack.push((0u32, 0u32));

        // TODO: Make parallel (maybe using rayon?)
        while let Some((depth, node_index)) = stack.pop() {
            if depth >= Self::MAX_DEPTH {
                continue;
            }
            let node = &nodes[node_index as usize];
            if let Some((left, right)) = Self::split_node(&mut triangles, node) {
                let left_index = nodes.len() as u32;
                let right_index = left_index + 1;
                nodes[node_index as usize].make_inner(left_index);
                nodes.push(left);
                nodes.push(right);
                stack.push((depth + 1, left_index));
                stack.push((depth + 1, right_index));
            }
        }

        let indices = triangles.iter().flat_map(|t| t.indices).collect();

        log::info!("Built BVH in {:?}", timer.elapsed());

        Self { nodes, vertices, indices, }
    }

    fn build_leaf(triangles: &[Triangle], index: u32, count: u32) -> BVHNode {
        debug_assert_ne!(count, 0);

        let mut min: Vec3 = triangles[index as usize].min;
        let mut max: Vec3 = triangles[index as usize].max;

        for i in index + 1..index + count {
            min = min.min(triangles[i as usize].min);
            max = max.max(triangles[i as usize].max);
        }

        BVHNode { min, index, max, count, }
    }

    fn split_node(triangles: &mut [Triangle], parent: &BVHNode) -> Option<(BVHNode, BVHNode)> {
        match parent.count {
            0 | 1 => None, // No need to split, single triangle
            2 => { // Just two triangles -> split manually
                let left = Self::build_leaf(triangles, parent.index, 1);
                let right = Self::build_leaf(triangles, parent.index + 1, 1);
                if left.cost() + right.cost() < parent.cost() {
                    Some((left, right))
                } else {
                    None
                }
            }
            // Ranges from 3 to 11 are small enough that brute forcing is faster than binning for N_BINS = 16
            3..=11 => { // Use Surface Area Heuristic to find best split by brute force
                let split = Self::find_best_split(triangles, parent)?;
                Self::split(triangles, parent, split)
            }
            _ => { // Use Surface Area Heuristic to find best split by binning
                let split = Self::approximate_best_split(triangles, parent)?;
                Self::split(triangles, parent, split)
            }
        }
    }

    fn find_best_split(triangles: &[Triangle], parent: &BVHNode) -> Option<Split> {
        let mut best_cost = parent.cost();
        let mut result = None;

        for axis in 0..3 {
            for i in parent.index..parent.index + parent.count {
                let mid = triangles[i as usize].center[axis];
                let mut left = Bin::default();
                let mut right = Bin::default();
                for j in parent.index..parent.index + parent.count {
                    let triangle = &triangles[j as usize];
                    if triangle.center[axis] < mid {
                        left.include(triangle);
                    } else {
                        right.include(triangle);
                    }
                }
                let cost = left.cost() + right.cost();
                if cost < best_cost {
                    best_cost = cost;
                    result = Some(Split { axis, mid, });
                }
            }
        }

        result
    }

    const N_BINS: usize = 16;

    fn approximate_best_split(triangles: &[Triangle], parent: &BVHNode) -> Option<Split> {
        // Build N_BINS bins per axis
        let mut bins = [Bin::default(); Self::N_BINS * 3];
        let step = (parent.max - parent.min) / Self::N_BINS as f32;

        for i in parent.index..parent.index + parent.count {
            let triangle = &triangles[i as usize];

            let bin_indices = Vec3::floor((triangle.center - parent.min) / step).as_uvec3().min(UVec3::splat(Self::N_BINS as u32 - 1));

            bins[bin_indices.x as usize].include(triangle);
            bins[Self::N_BINS + bin_indices.y as usize].include(triangle);
            bins[Self::N_BINS * 2 + bin_indices.z as usize].include(triangle);
        }

        let mut best_cost = parent.cost();
        let mut result = None;

        for axis in 0..3 {
            let mut left = Bin::default();
            for i in 0..Self::N_BINS - 1 {
                left.include_bin(&bins[axis * Self::N_BINS + i]);
                let mut right = Bin::default();
                for j in (i + 1)..Self::N_BINS {
                    right.include_bin(&bins[axis * Self::N_BINS + j])
                }
                let cost = left.cost() + right.cost();
                if cost < best_cost {
                    best_cost = cost;
                    let mid = parent.min[axis] + step[axis] * (i + 1) as f32;
                    result = Some(Split { axis, mid });
                }
            }
        }

        result
    }

    fn longest_split(parent: &BVHNode) -> Split {
        let extent = parent.max - parent.min;
        let mut axis = if extent.x > extent.y {0} else {1};
        if extent[axis] < extent.z {axis = 2;}
        let mid = (parent.min[axis] + parent.max[axis]) * 0.5;
        Split { axis, mid }
    }

    fn split(triangles: &mut [Triangle], parent: &BVHNode, split: Split) -> Option<(BVHNode, BVHNode)> {
        let mut left_count = 0u32;
        for i in parent.index..parent.index + parent.count {
            let center = triangles[i as usize].center[split.axis];
            if center < split.mid {
                triangles.swap(i as usize, parent.index as usize + left_count as usize);
                left_count += 1;
            }
        }

        if left_count == 0 || left_count == parent.count {
            log::debug!("Failed to split node");
            return None;
        }

        let left = Self::build_leaf(triangles, parent.index, left_count);
        let right = Self::build_leaf(triangles, parent.index + left_count, parent.count - left_count);
        Some((left, right))
    }
}

pub struct BVHBindGroup {
    nodes: wgpu::Buffer,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    group: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
}

impl BVHBindGroup {

    pub fn from_mesh(wgpu: &crate::common::WGPUContext, mesh: &Mesh) -> Self {
        let bvh = BVHTree::build_bvh(mesh);
        Self::from_bvh(wgpu, &bvh)
    }

    pub fn from_bvh(wgpu: &crate::common::WGPUContext, bvh: &BVHTree) -> Self {
        let nodes = wgpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BVH Nodes"),
            contents: bytemuck::cast_slice(&bvh.nodes),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let vertices = wgpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BVH Vertices"),
            contents: bytemuck::cast_slice(&bvh.vertices),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let indices = wgpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BVH Indices"),
            contents: bytemuck::cast_slice(&bvh.indices),
            usage: wgpu::BufferUsages::STORAGE,
        });

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
                        buffer: &vertices,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &indices,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        Self {
            nodes,
            vertices,
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
}