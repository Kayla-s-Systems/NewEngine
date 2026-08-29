use super::*;

impl PendingLitPipelineBuild {
    pub(super) fn create_bind_resources(
        &mut self,
        r: &mut dyn MaterialRenderDevice,
    ) -> MaterialDomainResult<()> {
        self.bgl = Some(
            r.create_bind_group_layout(
                BindGroupLayoutDesc::new(vec![
                    BindingKind::UniformBuffer,
                    BindingKind::Texture2D,
                    BindingKind::Texture2D,
                    BindingKind::Texture2D,
                    BindingKind::Texture2D,
                    BindingKind::Sampler,
                    // binding 6: local point/spot shadow atlas. Appended after the
                    // legacy sampler to preserve existing shader binding numbers.
                    BindingKind::Texture2D,
                ])
                .with_label("standard_lit_bgl"),
            )?,
        );
        self.skin_bgl = Some(
            r.create_bind_group_layout(
                BindGroupLayoutDesc::new(vec![BindingKind::StorageBuffer])
                    .with_label("standard_skin_palette_bgl"),
            )?,
        );
        self.white_texture = Some(
            r.create_texture(
                TextureDesc::new(
                    Extent2D::new(1, 1),
                    TextureFormat::Rgba8Unorm,
                    TextureUsage::Sampled,
                )
                .with_label("standard_white_tex")
                .with_data(vec![255, 255, 255, 255]),
            )?,
        );
        self.flat_normal_texture = Some(
            r.create_texture(
                TextureDesc::new(
                    Extent2D::new(1, 1),
                    TextureFormat::Rgba8Unorm,
                    TextureUsage::Sampled,
                )
                .with_label("standard_flat_normal_tex")
                .with_data(vec![128, 128, 255, 255]),
            )?,
        );
        self.repeat_sampler = Some(
            r.create_sampler(
                SamplerDesc::default()
                    .with_label("standard_repeat_sampler")
                    .with_repeat(),
            )?,
        );
        self.clamp_sampler = Some(
            r.create_sampler(
                SamplerDesc::default()
                    .with_label("standard_clamp_sampler")
                    .with_address_u(AddressMode::ClampToEdge)
                    .with_address_v(AddressMode::ClampToEdge)
                    .with_address_w(AddressMode::ClampToEdge),
            )?,
        );
        Ok(())
    }
}
