//! Separate uniform ranges for desktop/left/right views. A cached bind group
//! references one value-sized range; the caller selects its dynamic offset.
pub struct ViewUniforms {
    buffer: wgpu::Buffer,
    stride: u32,
    bytes: u64,
    slots: u32,
}
impl ViewUniforms {
    pub fn new(device: &wgpu::Device, label: &str, bytes: u64, slots: u32) -> Self {
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let stride = bytes.div_ceil(alignment) * alignment;
        assert!(bytes > 0 && slots > 0 && stride * slots as u64 <= device.limits().max_buffer_size);
        assert!(bytes <= device.limits().max_uniform_buffer_binding_size);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: stride * slots as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            stride: u32::try_from(stride).expect("uniform stride"),
            bytes,
            slots,
        }
    }
    pub fn binding(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: &self.buffer,
            offset: 0,
            size: wgpu::BufferSize::new(self.bytes),
        })
    }
    pub fn write(&self, queue: &wgpu::Queue, slot: u32, bytes: &[u8]) -> u32 {
        assert!(slot < self.slots && bytes.len() as u64 == self.bytes);
        let offset = slot * self.stride;
        queue.write_buffer(&self.buffer, offset as u64, bytes);
        offset
    }
}
