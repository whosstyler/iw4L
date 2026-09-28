//! Live scene frost, applied after game post effects and before the sharp menu overlay.
use bevy::{
    core_pipeline::{Core3d, Core3dSystems, FullscreenShader, upscaling::upscaling},
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::ViewTarget,
    },
    window::PrimaryWindow,
};

pub(crate) struct FrostedGlassPlugin;
impl Plugin for FrostedGlassPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "frosted.wgsl");
        app.add_plugins((
            ExtractComponentPlugin::<FrostedGlass>::default(),
            UniformComponentPlugin::<FrostedGlass>::default(),
        ))
        .add_systems(PostUpdate, sync_glass);
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render
                .add_systems(RenderStartup, init_pipeline)
                .add_systems(
                    Core3d,
                    draw_glass
                        .after(Core3dSystems::PostProcess)
                        .before(bevy::ui_render::render_pass::ui_pass)
                        .before(upscaling),
                );
        }
    }
}

#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
struct FrostedGlass {
    // Rectangles in physical pixels, matching the UI's viewport anchors.
    rects: [Vec4; 3],
    viewport: Vec4,
}
fn sync_glass(
    mut commands: Commands,
    enabled: Res<crate::MenuEnabled>,
    stack: Res<crate::RetailMenuStack>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(Entity, Option<&FrostedGlass>), With<Camera3d>>,
) {
    let top = stack.names.last().map(String::as_str);
    let active = enabled.0
        && matches!(
            top,
            Some(
                "ingame_options" | "pause_map" | "pause_social" | "pause_scoreboard" | "leave_game"
            )
        );
    let Ok(window) = windows.single() else {
        return;
    };
    let w = window.resolution.physical_width() as f32;
    let h = window.resolution.physical_height() as f32;
    let scale = crate::model::Canvas::Viewport.scale(w, h);
    let mut rects = crate::pause::PANELS.map(|[x, y, rw, rh]| {
        Vec4::new(
            if x < 0.0 { w + x * scale } else { x * scale },
            y * scale,
            rw * scale,
            rh * scale,
        )
    });
    if matches!(top, Some("pause_map" | "pause_social" | "pause_scoreboard")) {
        rects = [
            Vec4::new(38.0, 83.0, 778.0, 345.0) * scale,
            Vec4::ZERO,
            Vec4::ZERO,
        ];
    }
    for (entity, present) in &cameras {
        if active {
            commands.entity(entity).insert(FrostedGlass {
                rects,
                viewport: Vec4::new(w, h, scale, 0.0),
            });
        } else if present.is_some() {
            commands.entity(entity).remove::<FrostedGlass>();
        }
    }
}

#[derive(Resource)]
struct GlassPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    variants: Vec<(TextureFormat, [CachedRenderPipelineId; 2])>,
}
fn init_pipeline(
    mut commands: Commands,
    device: Res<RenderDevice>,
    assets: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "pause_glass_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<FrostedGlass>(true),
            ),
        ),
    );
    let shader = assets.load("embedded://ui/frosted.wgsl");
    let variants = [
        TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Rgba16Float,
    ]
    .into_iter()
    .map(|format| {
        let ids = ["horizontal", "vertical"].map(|entry| {
            cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some(format!("pause_glass_{entry}").into()),
                layout: vec![layout.clone()],
                vertex: fullscreen.to_vertex_state(),
                fragment: Some(FragmentState {
                    shader: shader.clone(),
                    entry_point: Some(entry.into()),
                    targets: vec![Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                    ..default()
                }),
                ..default()
            })
        });
        (format, ids)
    })
    .collect();
    commands.insert_resource(GlassPipeline {
        layout,
        sampler: device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        }),
        variants,
    });
}
fn draw_glass(
    view: ViewQuery<(&ViewTarget, &DynamicUniformIndex<FrostedGlass>)>,
    pipeline: Option<Res<GlassPipeline>>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<FrostedGlass>>,
    mut context: RenderContext,
) {
    let Some(pipeline) = pipeline else {
        return;
    };
    let (target, index) = view.into_inner();
    let Some((_, ids)) = pipeline
        .variants
        .iter()
        .find(|(format, _)| *format == target.main_texture_format())
    else {
        return;
    };
    // Check both passes before flipping either texture; a half-ready effect must leave the scene intact.
    let (Some(horizontal), Some(vertical)) = (
        cache.get_render_pipeline(ids[0]),
        cache.get_render_pipeline(ids[1]),
    ) else {
        return;
    };
    let Some(binding) = uniforms.uniforms().binding() else {
        return;
    };
    for pass_pipeline in [horizontal, vertical] {
        let post = target.post_process_write();
        let group = context.render_device().create_bind_group(
            "pause_glass_group",
            &cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((post.source, &pipeline.sampler, binding.clone())),
        );
        let mut pass = context
            .command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("pause_glass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: post.destination,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations::default(),
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        pass.set_pipeline(pass_pipeline);
        pass.set_bind_group(0, &group, &[index.index()]);
        pass.draw(0..3, 0..1);
    }
}
