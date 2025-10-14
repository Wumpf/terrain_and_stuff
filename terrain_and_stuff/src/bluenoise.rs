use wgpu::util::DeviceExt as _;

pub struct BluenoiseTextures {
    pub texture_view_2d: wgpu::TextureView,
}

impl BluenoiseTextures {
    pub fn load(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let texture_2d_png_bytes =
            include_bytes!("../../assets/bluenoise/stbn_vec1_2Dx1D_128x128x64_0.png");

        // Load the PNG using the `png` crate.
        let decoder = png::Decoder::new(std::io::Cursor::new(&texture_2d_png_bytes));
        let mut reader = decoder.read_info().expect("Failed to read PNG info");
        let mut buf = vec![
            0;
            reader
                .output_buffer_size()
                .expect("Can't retrieve output buffer")
        ];
        let info = reader
            .next_frame(&mut buf)
            .expect("Failed to decode PNG frame");
        let data = &buf[..info.buffer_size()];

        // Convert data to single channel
        let num_pixels = info.width as usize * info.height as usize;
        assert_eq!(data.len(), num_pixels * 4);
        let mut data_singlechannel = vec![0u8; num_pixels];
        for i in 0..num_pixels {
            let r = data[i * 4];
            data_singlechannel[i] = r;
        }

        let texture_2d = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Bluenoise Texture 2D"),
                size: wgpu::Extent3d {
                    width: info.width,
                    height: info.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &data_singlechannel,
        );

        let texture_view_2d = texture_2d.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Bluenoise Texture 2D"),
            ..Default::default()
        });

        Self { texture_view_2d }
    }
}
