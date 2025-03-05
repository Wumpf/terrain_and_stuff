pub struct PrimaryDepthBuffer {
    view: wgpu::TextureView,
}

impl PrimaryDepthBuffer {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub const STATE_WRITE: wgpu::DepthStencilState = wgpu::DepthStencilState {
        format: Self::FORMAT,
        depth_write_enabled: true,
        depth_compare: wgpu::CompareFunction::GreaterEqual, // Near plane is at 0, infinity is at 1.
        bias: wgpu::DepthBiasState {
            constant: 0,
            slope_scale: 0.0,
            clamp: 0.0,
        },
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState::IGNORE,
            back: wgpu::StencilFaceState::IGNORE,
            read_mask: 0,
            write_mask: 0,
        },
    };

    // pub const STATE_IGNORE: wgpu::DepthStencilState = wgpu::DepthStencilState {
    //     depth_write_enabled: false,
    //     depth_compare: wgpu::CompareFunction::Always,
    //     ..Self::STATE_WRITE
    // };

    pub fn new(device: &wgpu::Device, resolution: glam::UVec2) -> Self {
        let size = wgpu::Extent3d {
            width: resolution.x,
            height: resolution.y,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Buffer"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            // Need to be able to read out the depth buffer.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Depth32Float],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { view }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}
