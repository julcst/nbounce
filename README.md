# GPU-Accelerated Path Tracer in Rust and wgpu

This is a GPU-accelerated path tracer, fully written in Rust using `wgpu`, developed for my bachelor thesis. The primary goal is to implement and compare different approaches to **Neural Radiance Caching** in order to accelerate real-time path tracing.

Thanks to `wgpu`, this implementation is fully cross-platform, supporting Metal on macOS and Vulkan on Windows and Linux. However, due to the advanced GPU features required—some of which are not yet exposed by WebGPU—this will not natively run in the browser via WebAssembly (WASM). Future additions to the WebGPU standard might change this limitation.

## Planned Features
- [X] Software ray tracing using SAH-optimized BVH trees and Müller-Trombore intersection tests
- [ ] Hardware-accelerated ray tracing
- [X] Random Quasi-Monte Carlo sampling with a precomputed Owen-scrambled Sobol sequence and per-pixel random Cranley-Patterson rotations
- [ ] Neural Radiance Caching
- [X] Support for environment lighting and emissive materials
- [ ] Multiple Importance Sampling (MIS)
- [ ] Texture and normal map support
- [X] GLTF parsing (requires precomputed tangents, texture coordinates, and normals)
- [ ] Support for transmissive materials
- [X] Basic Disney BRDF: Burley Diffuse + Trowbridge-Reitz Specular PBR materials
- [X] Importance sampling of the Visible Normal Distribution Function (VNDF)
- [X] HDR output on macOS

## Features Not Planned (Yet)
- [ ] MikkTSpace tangent generation
- [ ] Advanced Disney BSDF: Subsurface scattering, sheen, clearcoat
- [ ] Implicit light sources: Directional, spot, point
- [ ] Animation support
- [ ] Neural supersampling
- [ ] Neural denoising
- [ ] Depth of field
- [ ] Tensor core utilization
- [ ] Volumetrics