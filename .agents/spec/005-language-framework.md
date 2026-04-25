# 005 -- Language and Framework Selection

**Date:** 2026-03-15
**Status:** Proposed -- Rust + ash (Vulkan) recommended, wgpu as fallback
**Updated:** 2026-03-15 (Iteration 003)

## Context

MARL requires:
1. A host language for simulation control, cell registry, HGT engine, I/O, and renderer orchestration
2. A GPU compute API for the field update pass (50M voxels x 8 species, 3D stencil), light attenuation pass (column prefix sum), and cell update pass (sparse agent evaluation)
3. A shader language for the compute kernels themselves

The system targets a single desktop with an RTX 4060 (8GB VRAM). It is research software, not a product -- developer velocity and correctness matter more than cross-platform reach. The user explicitly stated Rust is on the table.

## Options Considered

### Option A: Rust + wgpu (WebGPU abstraction)

**What it is:** wgpu is a pure-Rust implementation of the WebGPU API that maps to Vulkan, Metal, DX12, and OpenGL backends. Shaders written in WGSL (WebGPU Shading Language). Cross-platform by design.

**Advantages:**
- Safe Rust API -- no raw pointers, lifetime-tracked GPU resources
- Cross-platform (Windows, Linux, macOS, web via WebAssembly)
- Active community, good documentation (Learn Wgpu tutorial), ~14K GitHub stars
- Cargo ecosystem -- unified build system, dependency management
- WGSL shaders are readable and well-specified
- Could potentially run in browser for demos/outreach

**Disadvantages (CRITICAL for MARL):**
- **Buffer size cap: 1 GB maximum.** wgpu caps individual buffer allocations at 1 GB due to driver overflow bugs (GitHub issue #2337). MARL's field buffer at float16 is 800 MB per species in SoA layout, or 800 MB total if interleaved. A single double-buffered field (input + output) is 1.6 GB. This requires splitting across multiple buffers.
- **Storage buffer binding limit: 128 MiB default** (can be increased by requesting device limits, but max is still capped at 1 GB per binding). With 8 species in SoA layout, each species plane is 100 MB -- this fits in a single binding. But the full field as a single binding does NOT fit. Must use per-species bindings (8 bindings for field_in + 8 for field_out = 16 storage buffers per shader stage, which exceeds the default limit of 8).
- **Max storage buffers per shader stage: 8 (default).** Can be increased with device feature requests, but the WebGPU spec allows up to 10 in the `maxStorageBuffersPerShaderStage` limit. Native-only features (binding arrays) can work around this but sacrifice web portability.
- **Shared memory limit: 16,384 bytes per workgroup (default).** Our 2.5D tiling needs ~16 KB for 8 species -- exactly at the limit. Any increase in species count or tile size would exceed it. Vulkan devices typically support 32-48 KB.
- **Max workgroup invocations: 256 (default).** Our planned 32x8 tile = 256 threads, exactly at the limit. No room for alternative tile sizes.
- **No subgroup operations in stable WGSL.** Subgroup shuffle/ballot are critical for optimized stencil codes. Available only as experimental extensions.
- **CPU overhead.** wgpu's validation and state tracking add measurable CPU overhead per dispatch call. For a tight simulation loop dispatching 4+ compute passes per tick at 100+ ticks/sec, this overhead is noticeable.

**Verdict:** wgpu's buffer and binding limits are dangerously close to MARL's requirements. It would work, but with zero headroom. Any increase in species count, grid resolution, or precision (float32 fallback) would require workarounds that add complexity. The 16 KB shared memory cap is particularly concerning for the 2.5D tiling strategy.

### Option B: Rust + ash (raw Vulkan bindings)

**What it is:** ash is a low-level, mostly-generated Rust binding to the Vulkan API. It provides the full Vulkan API surface with minimal abstraction. Shaders written in GLSL or HLSL, compiled to SPIR-V.

**Advantages:**
- **Full Vulkan access.** No artificial buffer size limits. The RTX 4060 supports buffers up to available VRAM (8 GB). Single allocations of 2+ GB are routine.
- **Full device limits.** Shared memory up to 49,152 bytes (48 KB) on Ada Lovelace. Storage buffers limited only by descriptor count (up to millions). Workgroup size up to 1024 invocations.
- **Subgroup operations.** Vulkan 1.1+ mandates subgroup support. Subgroup shuffle, ballot, and arithmetic are available for optimized reductions and stencil codes.
- **Vulkan Compute specialization constants.** Can parameterize species count, tile size, R_max at pipeline creation without shader recompilation.
- **Zero abstraction overhead.** Dispatch calls are direct Vulkan commands. No validation layer in release builds.
- **SPIR-V ecosystem.** GLSL compute shaders compiled with glslc/glslangValidator are mature, well-documented, and widely used in HPC GPU computing.
- **Timeline semaphores, async compute queues.** Advanced synchronization for overlapping field update with cell update on separate queues.

**Disadvantages:**
- **Unsafe Rust.** ash is inherently unsafe -- raw Vulkan commands can cause UB if used incorrectly. Requires careful wrapper design.
- **Boilerplate.** Vulkan requires 500+ lines of setup code (instance, device, queues, command pools, descriptor sets, pipeline layouts) before any compute work can begin.
- **Vulkan-only.** No Metal, no DX12, no web. Limits the project to Windows/Linux with Vulkan-capable GPUs.
- **No built-in resource lifetime tracking.** Must manually manage buffer lifetimes, descriptor set updates, and synchronization.
- **Steeper learning curve.** Vulkan's explicitness is powerful but demands deep GPU programming knowledge.

**Mitigation for disadvantages:**
- Unsafe wrapper: write a thin safe abstraction layer (~1000 lines) that wraps buffer allocation, pipeline creation, and dispatch. This is a one-time cost.
- Boilerplate: use a helper library like `gpu-allocator` (Rust crate for Vulkan memory allocation) and `vulkano` patterns for descriptor management.
- Vulkan-only: acceptable for research software targeting a specific desktop. Cross-platform is a nice-to-have, not a requirement.

**Verdict:** ash provides the full hardware capability with no artificial constraints. The boilerplate cost is real but amortized -- it is written once. For a research project pushing hardware limits, the access to full Vulkan features is worth the upfront investment.

### Option C: C++ + Vulkan (direct)

**What it is:** The traditional choice for GPU compute. C++ with the Vulkan C API or Vulkan-Hpp (C++ bindings).

**Advantages:**
- Maximum ecosystem maturity. Vast library of Vulkan tutorials, examples, and production code.
- Performance equivalent to Rust (within 5-10% on most benchmarks).
- Full Vulkan access (same as Option B).
- Existing 3D stencil implementations to reference (CUDA literature often translates directly).

**Disadvantages:**
- Memory safety bugs. Buffer overruns, use-after-free, dangling pointers are the #1 source of bugs in GPU compute codebases. No compile-time guarantees.
- Build system complexity. CMake + vcpkg/conan + manual SPIR-V compilation pipeline. No equivalent to Cargo's unified ergonomics.
- Slower development velocity for a solo/small-team research project.
- No RAII for Vulkan handles without manual wrappers.

**Verdict:** C++ is the established choice but offers no advantage over Rust for this project. Rust's memory safety and Cargo ecosystem provide meaningful developer velocity improvements. The performance difference is negligible for a bandwidth-bound GPU workload (the compute kernels are SPIR-V regardless of host language).

### Option D: Rust + vulkano (safe Vulkan wrapper)

**What it is:** vulkano is a Rust crate providing safe, high-level Vulkan bindings with compile-time shader validation.

**Advantages:**
- Safe Rust API without the boilerplate of raw ash.
- Compile-time SPIR-V validation (shaders checked at build time).
- Full Vulkan device limits (not constrained like wgpu).
- Active maintenance.

**Disadvantages:**
- Smaller community than wgpu or ash.
- Abstraction sometimes lags behind latest Vulkan features.
- Less documentation than wgpu or raw Vulkan.
- Still requires understanding Vulkan concepts (descriptors, pipelines, command buffers).

**Verdict:** A reasonable middle ground. Less boilerplate than ash, more capable than wgpu. Worth considering as a fallback if ash proves too much boilerplate.

### Option E: Rust + rust-gpu (Rust shaders compiled to SPIR-V)

**What it is:** rust-gpu compiles Rust code directly to SPIR-V, allowing shaders to be written in Rust instead of GLSL/WGSL. Can be used with wgpu or ash as the host API.

**Advantages:**
- Single language for host and device code. Share types, constants, validation logic.
- Rust's type system in shaders -- prevents many classes of shader bugs.
- Active development (2025: "Rust running on every GPU" milestone).

**Disadvantages:**
- Requires a specific nightly Rust compiler version. Fragile toolchain.
- Not production-ready (stated explicitly by the project).
- Limited shader feature support -- many GLSL/WGSL features not yet available.
- Debugging is harder (SPIR-V generated by non-standard compiler).

**Verdict:** Promising for the future but too immature for a research project that needs to produce results. Revisit if/when rust-gpu reaches stable.

## Analysis: The Buffer Size Problem

The decisive technical factor is buffer sizing. Let me quantify:

```
MARL's VRAM requirements (float16 field):

  Field input buffer:   50M voxels x 8 species x 2 bytes = 800 MB
  Field output buffer:  800 MB (double-buffered)
  Source delta buffer:   50M x 8 x 4 bytes = 1,600 MB (float32 for atomics)
  Light field:           50M x 2 bytes = 100 MB
  Cell registry:         100K cells x 346 bytes = 35 MB
  ---------------------------------------------------------
  TOTAL:                ~3,335 MB (~3.3 GB of 8 GB VRAM)

How this maps to buffer bindings:

  SoA layout: each species is a separate 100 MB plane.
  Field input:  8 bindings (one per species plane) x 100 MB = 800 MB total
  Field output: 8 bindings x 100 MB = 800 MB total
  Delta buffer: 8 bindings x 200 MB = 1,600 MB total
  Light:        1 binding x 100 MB
  Cells:        1 binding x 35 MB
  ---------------------------------------------------------
  Total bindings per shader stage: up to 26 bindings

  wgpu default:    8 storage buffers per stage (can request up to ~10)
  Vulkan minimum: 16,384 storage buffers per stage (effectively unlimited)
```

**With wgpu:** We need 26 storage buffer bindings per shader stage. wgpu's max (even with feature requests) is ~10-12 for storage buffers. This requires either:
- Multiple dispatch passes (split species processing across passes) -- doubles dispatch overhead
- Binding arrays (native-only, not WebGPU-compatible) -- sacrifices web portability
- AoS layout (all species interleaved in one buffer) -- sacrifices memory coalescing

**With ash/Vulkan:** 26 bindings is trivial. No workarounds needed.

## Decision

**Proposed: Rust + ash (Option B)**

Rationale:
1. **Buffer and binding limits are the deciding factor.** wgpu's artificial caps require compromises that degrade performance or add complexity. ash provides full hardware access.
2. **Rust over C++.** Memory safety, Cargo, and ecosystem quality provide meaningful developer velocity for a research project. Performance is equivalent.
3. **The project is Vulkan-only by practical necessity.** The target is a single desktop with an NVIDIA GPU. Cross-platform and web deployment are aspirational, not requirements.
4. **Boilerplate is a one-time cost.** The Vulkan setup code (~500-1000 lines) is written once and wrapped in a safe abstraction. All subsequent development uses the wrapper.
5. **Full subgroup operations, specialization constants, and async compute** enable performance optimizations that wgpu cannot access.

**Fallback: Rust + vulkano (Option D)** if ash boilerplate proves unmanageable for a small team.

**Migration path to wgpu:** If wgpu's limits are raised in the future (buffer size cap removed, storage buffer count increased), the compute shaders (GLSL -> WGSL) and host logic (ash -> wgpu) can be ported. The simulation logic is API-agnostic.

## Consequences

1. **Shader language: GLSL (compiled to SPIR-V).** GLSL is the most documented and widely used Vulkan shader language. Compute shader support is mature. Use `glslangValidator` or `shaderc` for compilation.
2. **Build system: Cargo + build.rs for SPIR-V compilation.** The `shaderc` Rust crate can compile GLSL to SPIR-V at build time.
3. **Memory management: `gpu-allocator` crate** for Vulkan memory allocation (handles memory types, sub-allocation, defragmentation).
4. **Platform support: Windows + Linux.** macOS via MoltenVK is theoretically possible but not a target.
5. **Safe wrapper design:** Create a `marl-gpu` module wrapping ash calls with safe Rust interfaces: `GpuField`, `GpuCellRegistry`, `ComputePass`, `DispatchChain`. This wrapper is MARL-specific, not a general-purpose Vulkan abstraction.

## Open Questions

1. **Should we use HLSL instead of GLSL?** HLSL has better tooling (DXC compiler, RenderDoc integration) and is the shader language for DX12. Vulkan supports HLSL via SPIR-V. However, GLSL is more natural for Vulkan and has more compute shader examples.
2. **Should we evaluate vulkano more seriously?** If the boilerplate cost of ash proves too high, vulkano provides a middle ground with safe abstractions and compile-time shader validation.
3. **Timeline for implementation:** The spec is nearly complete enough to begin implementation. The field update pass (3D stencil diffusion) is the first module to implement -- it has no dependencies on open design questions.

## References

- ash crate: https://github.com/ash-rs/ash
- wgpu crate: https://github.com/gfx-rs/wgpu
- wgpu buffer size limitation: https://github.com/gfx-rs/wgpu/issues/2337
- wgpu limits documentation: https://wgpu.rs/doc/wgpu/struct.Limits.html
- gpu-allocator crate: https://github.com/Traverse-Research/gpu-allocator
- Vulkan compute shader tutorial: https://docs.vulkan.org/tutorial/latest/11_Compute_Shader.html
- Rust vs C++ performance: https://blog.jetbrains.com/rust/2025/12/16/rust-vs-cpp-comparison-for-2026/
- rust-gpu project: https://github.com/Rust-GPU/rust-gpu
