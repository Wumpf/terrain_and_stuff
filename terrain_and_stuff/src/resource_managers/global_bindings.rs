use crate::{
    bluenoise::BluenoiseTextures,
    wgpu_utils::{
        BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutWithDesc, wgpu_buffer_types,
    },
};

#[repr(C, align(16))]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameUniformBuffer {
    pub view_from_world: wgpu_buffer_types::Mat4x3,
    pub projection_from_view: wgpu_buffer_types::Mat4,
    pub projection_from_world: wgpu_buffer_types::Mat4,

    /// Camera position in world space.
    pub camera_position: wgpu_buffer_types::Vec3RowPadded,

    /// Camera direction in world space.
    /// Same as -vec3f(view_from_world[0].z, view_from_world[1].z, view_from_world[2].z)
    pub camera_forward: wgpu_buffer_types::Vec3RowPadded,

    /// (tan(fov_y / 2) * aspect_ratio, tan(fov_y /2)), i.e. half ratio of screen dimension to screen distance in x & y.
    /// Both values are set to f32max for orthographic projection
    pub tan_half_fov: wgpu_buffer_types::Vec2RowPadded,

    /// Direction to the sun or moon in world space.
    pub dir_to_sun: wgpu_buffer_types::Vec3RowPadded,
}

pub struct GlobalBindings {
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: BindGroupLayoutWithDesc,
    frame_uniform_buffer: wgpu::Buffer,
}

impl GlobalBindings {
    pub fn new(device: &wgpu::Device, bluenoise: &BluenoiseTextures) -> Self {
        let frame_uniform_buffer_size = std::mem::size_of::<FrameUniformBuffer>() as u64;
        let frame_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Frame uniform buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: frame_uniform_buffer_size,
            mapped_at_creation: false,
        });

        let bind_group_layout = BindGroupLayoutBuilder::new()
            .next_binding_all(wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(frame_uniform_buffer_size),
            })
            .next_binding_all(wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            })
            .next_binding_all(wgpu::BindingType::Sampler(
                wgpu::SamplerBindingType::NonFiltering,
            ))
            .next_binding_all(wgpu::BindingType::Sampler(
                wgpu::SamplerBindingType::NonFiltering,
            ))
            .next_binding_all(wgpu::BindingType::Sampler(
                wgpu::SamplerBindingType::Filtering,
            ))
            .next_binding_all(wgpu::BindingType::Sampler(
                wgpu::SamplerBindingType::Filtering,
            ));
        let bind_group_layout = bind_group_layout.create(device, "GlobalBindings");

        let nearest_neighbor_sampler_clamp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: "GlobalBindings::nearest_neighbor_sampler_clamp".into(),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let nearest_neighbor_sampler_repeat = device.create_sampler(&wgpu::SamplerDescriptor {
            label: "GlobalBindings::nearest_neighbor_sampler_repeat".into(),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let trilinear_sampler_clamp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: "GlobalBindings::trilinear_sampler_clamp".into(),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let trilinear_sampler_repeat = device.create_sampler(&wgpu::SamplerDescriptor {
            label: "GlobalBindings::trilinear_sampler_repeat".into(),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        let bind_group = BindGroupBuilder::new(&bind_group_layout)
            .buffer(wgpu::BufferBinding {
                buffer: &frame_uniform_buffer,
                offset: 0,
                size: std::num::NonZeroU64::new(frame_uniform_buffer_size),
            })
            .texture(&bluenoise.texture_view_2d)
            .sampler(&nearest_neighbor_sampler_clamp)
            .sampler(&nearest_neighbor_sampler_repeat)
            .sampler(&trilinear_sampler_clamp)
            .sampler(&trilinear_sampler_repeat)
            .create(device, "GlobalBindings");

        Self {
            bind_group,
            bind_group_layout,
            frame_uniform_buffer,
        }
    }

    pub fn update_frame_uniform_buffer(&self, queue: &wgpu::Queue, frame: &FrameUniformBuffer) {
        queue.write_buffer(&self.frame_uniform_buffer, 0, bytemuck::bytes_of(frame));
    }
}
