use crate::wgpu_utils::{BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutWithDesc};

pub struct GlobalBindings {
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: BindGroupLayoutWithDesc,
}

impl GlobalBindings {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = BindGroupLayoutBuilder::new()
            .next_binding_all(wgpu::BindingType::Sampler(
                wgpu::SamplerBindingType::NonFiltering,
            ))
            .next_binding_all(wgpu::BindingType::Sampler(
                wgpu::SamplerBindingType::NonFiltering,
            ));
        let bind_group_layout = bind_group_layout.create(device, "GlobalBindings");

        let nearest_neighbor_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: "GlobalBindings::nearest_neighbor_sampler".into(),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let trilinear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: "GlobalBindings::trilinear_sampler".into(),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        let bind_group = BindGroupBuilder::new(&bind_group_layout)
            .sampler(&nearest_neighbor_sampler)
            .sampler(&trilinear_sampler)
            .create(device, "GlobalBindings");

        Self {
            bind_group,
            bind_group_layout,
        }
    }
}
