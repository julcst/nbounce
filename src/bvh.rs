use glam::{UVec3, Vec3, Vec4Swizzles};

use crate::scene::Vertex;

pub trait BVHPrimitive {
    fn min(&self) -> &Vec3;
    fn max(&self) -> &Vec3;
    fn count(&self) -> u32 {1}
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::NoUninit)]
pub struct BVHNode {
    min: Vec3,
    /// If this is a leaf node, this is the index of the first triangle index.
    /// If this is an inner node, this is the index of the left child node.
    start: u32,
    max: Vec3,
    count: u32,
}

impl BVHNode {
    fn new_leaf(triangles: &[Triangle], start: u32, count: u32) -> Self {
        debug_assert_ne!(count, 0);

        let mut min: Vec3 = triangles[start as usize].min;
        let mut max: Vec3 = triangles[start as usize].max;

        for i in start + 1..start + count {
            min = min.min(triangles[i as usize].min);
            max = max.max(triangles[i as usize].max);
        }

        Self { min, start, max, count, }
    }

    fn from_bin(bin: &Bin, start: u32) -> Self {
        Self { min: bin.min, start, max: bin.max, count: bin.count, }
    }

    fn is_leaf(&self) -> bool {
        self.count > 0
    }

    fn make_inner(&mut self, left_child: u32) {
        self.count = 0; // Mark as inner node
        self.start = left_child;
    }

    fn cost(&self) -> f32 {
        debug_assert!(self.is_leaf());
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
}

pub struct Triangle {
    center: Vec3,
    min: Vec3,
    max: Vec3,
    indices: [u32; 3],
}

impl BVHPrimitive for Triangle {
    fn min(&self) -> &Vec3 {&self.min}
    fn max(&self) -> &Vec3 {&self.max}
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

const MAX_DEPTH: u32 = 32;
const N_BINS: usize = 16;

pub fn build_triangle_cache(vertices: &[Vertex], indices: &[u32]) -> Vec<Triangle> {
    let timer = std::time::Instant::now();
    let mut triangles = Vec::with_capacity(indices.len() as usize / 3);
    for triangle in indices.chunks_exact(3) {
        let v0 = vertices[triangle[0] as usize].position.xyz();
        let v1 = vertices[triangle[1] as usize].position.xyz();
        let v2 = vertices[triangle[2] as usize].position.xyz();
        let center = (v0 + v1 + v2) / 3.0;
        let min = v0.min(v1).min(v2);
        let max = v0.max(v1).max(v2);
        triangles.push(Triangle {center, min, max, indices: triangle.try_into().unwrap()});
    }
    log::info!("Built triangle cache in {:?}", timer.elapsed());
    triangles
}

pub fn build_bvh(triangles: &mut[Triangle], start: u32, count: u32) -> Vec<BVHNode> {
    let timer = std::time::Instant::now();
    let mut nodes = Vec::new();
    let mut stack = Vec::new();

    // TODO: Fix
    let parent = BVHNode::new_leaf(triangles, start, count);
    nodes.push(parent);
    stack.push((0u32, 0u32));

    // TODO: Make parallel (maybe using rayon?)
    while let Some((depth, node_index)) = stack.pop() {
        if depth >= MAX_DEPTH {
            continue;
        }
        let node = &nodes[node_index as usize];
        if let Some((left, right)) = split_node(triangles, node) {
            let left_index = nodes.len() as u32;
            let right_index = left_index + 1;
            nodes[node_index as usize].make_inner(left_index);
            nodes.push(left);
            nodes.push(right);
            stack.push((depth + 1, left_index));
            stack.push((depth + 1, right_index));
        }
    }

    log::info!("Built BVH in {:?}", timer.elapsed());

    nodes
}

pub fn flatten_triangle_list(triangles: &[Triangle], indices: &mut[u32]) {
    for (i, index) in triangles.iter().flat_map(|t| t.indices).enumerate() {
        indices[i] = index;
    }
}

fn split_node(triangles: &mut [Triangle], parent: &BVHNode) -> Option<(BVHNode, BVHNode)> {
    match parent.count {
        0 | 1 => None, // No need to split, single triangle
        2 => { // Just two triangles -> split manually
            let left = BVHNode::new_leaf(triangles, parent.start, 1);
            let right = BVHNode::new_leaf(triangles, parent.start + 1, 1);
            if left.cost() + right.cost() < parent.cost() {
                Some((left, right))
            } else {
                None
            }
        }
        // Ranges from 3 to 11 are small enough that brute forcing is faster than binning for N_BINS = 16
        3..=11 => { // Use Surface Area Heuristic to find best split by brute force
            let s = find_best_split(triangles, parent)?;
            split(triangles, parent, s)
        }
        _ => { // Use Surface Area Heuristic to find best split by binning
            let s = approximate_best_split(triangles, parent)?;
            split(triangles, parent, s)
        }
    }
}

fn find_best_split(triangles: &[Triangle], parent: &BVHNode) -> Option<Split> {
    let mut best_cost = parent.cost();
    let mut result = None;

    for axis in 0..3 {
        for i in parent.start..parent.start + parent.count {
            let mid = triangles[i as usize].center[axis];
            let mut left = Bin::default();
            let mut right = Bin::default();
            for j in parent.start..parent.start + parent.count {
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

fn approximate_best_split(triangles: &[Triangle], parent: &BVHNode) -> Option<Split> {
    // Build N_BINS bins per axis
    let mut bins = [Bin::default(); N_BINS * 3];
    let step = (parent.max - parent.min) / N_BINS as f32;

    for i in parent.start..parent.start + parent.count {
        let triangle = &triangles[i as usize];

        let bin_indices = Vec3::floor((triangle.center - parent.min) / step).as_uvec3().min(UVec3::splat(N_BINS as u32 - 1));

        bins[bin_indices.x as usize].include(triangle);
        bins[N_BINS + bin_indices.y as usize].include(triangle);
        bins[N_BINS * 2 + bin_indices.z as usize].include(triangle);
    }

    let mut best_cost = parent.cost();
    let mut result = None;

    for axis in 0..3 {
        let mut left = Bin::default();
        for i in 0..N_BINS - 1 {
            left.include_bin(&bins[axis * N_BINS + i]);
            let mut right = Bin::default();
            for j in (i + 1)..N_BINS {
                right.include_bin(&bins[axis * N_BINS + j])
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

#[allow(dead_code)]
fn longest_split(parent: &BVHNode) -> Split {
    let extent = parent.max - parent.min;
    let mut axis = if extent.x > extent.y {0} else {1};
    if extent[axis] < extent.z {axis = 2;}
    let mid = (parent.min[axis] + parent.max[axis]) * 0.5;
    Split { axis, mid }
}

fn split(triangles: &mut [Triangle], parent: &BVHNode, split: Split) -> Option<(BVHNode, BVHNode)> {
    let mut left = Bin::default();
    let mut right = Bin::default();

    for i in parent.start..parent.start + parent.count {
        let triangle = &triangles[i as usize];
        let center = triangle.center[split.axis];
        if center < split.mid {
            left.include(triangle);
            triangles.swap(i as usize, parent.start as usize + left.count as usize - 1);
        } else {
            right.include(triangle);
        }
    }

    if left.count == 0 || right.count == 0 {
        log::debug!("Failed to split node");
        return None;
    }

    let left = BVHNode::from_bin(&left, parent.start);
    let right = BVHNode::from_bin(&right, parent.start + left.count);
    Some((left, right))
}