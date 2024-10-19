pub struct BindGroupLayoutWithDesc {
    pub layout: wgpu::BindGroupLayout,
    pub entries: Vec<wgpu::BindGroupLayoutEntry>,
}

pub struct BindGroupLayoutBuilder {
    entries: Vec<wgpu::BindGroupLayoutEntry>,
    next_binding_index: u32,
}

#[allow(dead_code)]
impl BindGroupLayoutBuilder {
    pub fn new() -> Self {
        BindGroupLayoutBuilder {
            entries: Vec::new(),
            next_binding_index: 0,
        }
    }

    pub fn binding(mut self, binding: wgpu::BindGroupLayoutEntry) -> Self {
        self.next_binding_index = binding.binding + 1;
        self.entries.push(binding);
        self
    }

    pub fn next_binding(self, visibility: wgpu::ShaderStages, ty: wgpu::BindingType) -> Self {
        let binding = self.next_binding_index;
        self.binding(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty,
            count: None,
        })
    }

    pub fn next_binding_compute(self, ty: wgpu::BindingType) -> Self {
        self.next_binding(wgpu::ShaderStages::COMPUTE, ty)
    }

    pub fn next_binding_fragment(self, ty: wgpu::BindingType) -> Self {
        self.next_binding(wgpu::ShaderStages::FRAGMENT, ty)
    }

    pub fn next_binding_vertex(self, ty: wgpu::BindingType) -> Self {
        self.next_binding(wgpu::ShaderStages::VERTEX, ty)
    }

    //pub fn next_binding_rendering(self, ty: wgpu::BindingType) -> Self {
    //    self.next_binding(wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT, ty)
    //}

    pub fn next_binding_all(self, ty: wgpu::BindingType) -> Self {
        self.next_binding(
            wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty,
        )
    }

    pub fn create(self, device: &wgpu::Device, label: &str) -> BindGroupLayoutWithDesc {
        BindGroupLayoutWithDesc {
            layout: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &self.entries,
                label: Some(label),
            }),
            entries: self.entries,
        }
    }
}

// Builder for wgpu::BindGroups following the exact layout from a wgpu::BindGroupLayout
// Makes life simpler by assuming that order of elements in the bind group is equal to order of elements in the bind group layout.
pub struct BindGroupBuilder<'a> {
    layout_with_desc: &'a BindGroupLayoutWithDesc,
    entries: Vec<wgpu::BindGroupEntry<'a>>,
}

#[allow(dead_code)]
impl<'a> BindGroupBuilder<'a> {
    pub fn new(layout_with_desc: &'a BindGroupLayoutWithDesc) -> Self {
        BindGroupBuilder {
            layout_with_desc,
            entries: Vec::new(),
        }
    }

    // Uses same binding index as binding group layout at the same ordering
    pub fn resource(mut self, resource: wgpu::BindingResource<'a>) -> Self {
        assert!(self.entries.len() < self.layout_with_desc.entries.len());

        self.entries.push(wgpu::BindGroupEntry {
            binding: self.layout_with_desc.entries[self.entries.len()].binding,
            resource,
        });
        self
    }

    pub fn buffer(self, buffer_binding: wgpu::BufferBinding<'a>) -> Self {
        self.resource(wgpu::BindingResource::Buffer(buffer_binding))
    }

    pub fn sampler(self, sampler: &'a wgpu::Sampler) -> Self {
        self.resource(wgpu::BindingResource::Sampler(sampler))
    }

    pub fn texture(self, texture_view: &'a wgpu::TextureView) -> Self {
        self.resource(wgpu::BindingResource::TextureView(texture_view))
    }

    pub fn create(&self, device: &wgpu::Device, label: &str) -> wgpu::BindGroup {
        assert_eq!(self.entries.len(), self.layout_with_desc.entries.len());

        let descriptor = wgpu::BindGroupDescriptor {
            layout: &self.layout_with_desc.layout,
            entries: &self.entries,
            label: Some(label),
        };
        device.create_bind_group(&descriptor)
    }
}
