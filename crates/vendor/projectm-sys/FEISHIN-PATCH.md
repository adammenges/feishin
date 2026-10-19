# Native framebuffer support

This contains the Rust source distribution of `projectm-sys` 1.2.2 from crates.io,
with its bundled C++ engine upgraded to the stable projectM 4.1.8 release.
`scripts/vendor-native-projectm.py` verifies the release archive's SHA256
f72135e8b8e5fd807f908537255b41ac7ba4830309265e830cb756535487239e
and reproduces the local C++ changes:

- Backport the `projectm_opengl_render_frame_fbo` API from projectM 4.2.
- Pass the framebuffer to `ProjectM::RenderFrame` and restore its viewport before
  drawing the final preset or transition.
- Remove a detached attachment before updating draw buffers. Otherwise Apple's
  OpenGL driver rejects the preset's framebuffer as incomplete.
- Save the active framebuffer before blur texture allocation, which changes its
  binding. Set non-mipmapped texture filters for newly allocated blur textures.
- Expose stored compiler errors through `what()` and include evaluator errors
  and line numbers in preset failures.
- Resolve unavailable named textures to the original Butterchurn cloud texture.
- Rename legacy user-defined noise1..noise4 functions and sample identifiers
  before transpilation to preserve formulas without compiler-name collisions.
- Report invalid warp/composite shaders through preset failure callbacks instead
  of silently substituting the default effect.
- Use bundled projectM-eval and explicit static GLEW paths in Windows builds.

The stock bundled library always targets framebuffer zero. GPUI renders with
Metal or Vulkan, and Surfman's offscreen target is a caller-owned OpenGL
framebuffer. Targeting framebuffer zero would produce no usable native frame.

Upstream sources:

- https://github.com/projectM-visualizer/projectm-rs
- https://github.com/projectM-visualizer/projectm/blob/master/src/api/include/projectM-4/render_opengl.h
- https://github.com/projectM-visualizer/projectm/blob/master/src/libprojectM/ProjectM.cpp

The Rust bindings are LGPL-3.0-or-later (see LICENSE). The bundled projectM
library and its headers are LGPL-2.1-or-later, with licenses noted in individual
source files. All corresponding source is included here. Static distribution is
part of the GPL-3.0 Feishin application; build instructions and Cargo.lock allow
rebuilding and relinking with modified sources.
