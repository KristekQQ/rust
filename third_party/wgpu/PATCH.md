# Local wgpu patch

This is the upstream `wgpu` 30.0.1 crate with a narrow WebXR adapter option
extension. Upstream's WebGPU request adapter options do not expose
`xrCompatible`, which is needed when creating an adapter for immersive WebXR.

Changes:

- Adds `Instance::new_webgpu_xr`, which creates a WebGPU context that requests
  adapters with `xrCompatible: true`.
- Keeps the ordinary instance constructor's adapter request unchanged.
