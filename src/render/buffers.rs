use wgpu::util::DeviceExt;

use crate::render::types::{as_bytes, Uniforms};
use crate::scene::cube::{VERTICES, INDICES};

pub struct GpuBuffers {
    pub vertex: wgpu::Buffer,
    pub index: wgpu::Buffer,
    pub uniform: wgpu::Buffer,
}

pub fn create(device: &wgpu::Device) -> GpuBuffers {
    let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vertex buffer"),
        contents: as_bytes(VERTICES),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("index buffer"),
        contents: as_bytes(INDICES),
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
    });
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("uniform buffer"),
        contents: as_bytes(&[Uniforms { mvp: [[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]] }]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    GpuBuffers { vertex, index, uniform }
}
