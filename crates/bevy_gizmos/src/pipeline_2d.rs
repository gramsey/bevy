use crate::{
    config::{GizmoLineJoint, GizmoLineStyle, GizmoMeshConfig},
    line_gizmo_vertex_buffer_layouts, line_joint_gizmo_vertex_buffer_layouts, DrawLineGizmo,
    DrawLineJointGizmo, GizmoRenderSystems, GpuLineGizmo, LineGizmoUniformBindgroupLayout,
    SetLineGizmoBindGroup,
};
use bevy_app::{App, Plugin};
use bevy_asset::{load_embedded_asset, Handle};
use bevy_core_pipeline::core_2d::{Transparent2d, CORE_2D_DEPTH_FORMAT};

use bevy_ecs::{
    prelude::Entity,
    resource::Resource,
    schedule::IntoScheduleConfigs,
    system::{Query, Res, ResMut},
    world::{FromWorld, World},
};
use bevy_image::BevyDefault as _;
use bevy_math::FloatOrd;
use bevy_render::sync_world::MainEntity;
use bevy_render::{
    render_asset::{prepare_assets, RenderAssets},
    render_phase::{
        AddRenderCommand, DrawFunctions, PhaseItemExtraIndex, SetItemPipeline,
        ViewSortedRenderPhases,
    },
    render_resource::*,
    view::{ExtractedView, Msaa, RenderLayers, ViewTarget},
    Render, RenderApp, RenderSystems,
};
use bevy_sprite::{Mesh2dPipeline, Mesh2dPipelineKey, SetMesh2dViewBindGroup};
use tracing::error;

pub struct LineGizmo2dPlugin;

impl Plugin for LineGizmo2dPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_command::<Transparent2d, DrawLineGizmo2d>()
            .add_render_command::<Transparent2d, DrawLineGizmo2dStrip>()
            .add_render_command::<Transparent2d, DrawLineJointGizmo2d>()
            .init_resource::<SpecializedRenderPipelines<LineGizmoPipeline>>()
            .init_resource::<SpecializedRenderPipelines<LineJointGizmoPipeline>>()
            .configure_sets(
                Render,
                GizmoRenderSystems::QueueLineGizmos2d
                    .in_set(RenderSystems::Queue)
                    .ambiguous_with(bevy_sprite::queue_sprites)
                    .ambiguous_with(
                        bevy_sprite::queue_material2d_meshes::<bevy_sprite::ColorMaterial>,
                    ),
            )
            .add_systems(
                Render,
                (queue_line_gizmos_2d, queue_line_joint_gizmos_2d)
                    .in_set(GizmoRenderSystems::QueueLineGizmos2d)
                    .after(prepare_assets::<GpuLineGizmo>),
            );
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<LineGizmoPipeline>();
        render_app.init_resource::<LineJointGizmoPipeline>();
    }
}

#[derive(Clone, Resource)]
struct LineGizmoPipeline {
    mesh_pipeline: Mesh2dPipeline,
    uniform_layout: BindGroupLayout,
    shader: Handle<Shader>,
}

impl FromWorld for LineGizmoPipeline {
    fn from_world(render_world: &mut World) -> Self {
        LineGizmoPipeline {
            mesh_pipeline: render_world.resource::<Mesh2dPipeline>().clone(),
            uniform_layout: render_world
                .resource::<LineGizmoUniformBindgroupLayout>()
                .layout
                .clone(),
            shader: load_embedded_asset!(render_world, "lines.wgsl"),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct LineGizmoPipelineKey {
    mesh_key: Mesh2dPipelineKey,
    strip: bool,
    line_style: GizmoLineStyle,
}

impl SpecializedRenderPipeline for LineGizmoPipeline {
    type Key = LineGizmoPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let format = if key.mesh_key.contains(Mesh2dPipelineKey::HDR) {
            ViewTarget::TEXTURE_FORMAT_HDR
        } else {
            TextureFormat::bevy_default()
        };

        let shader_defs = vec![
            #[cfg(feature = "webgl")]
            "SIXTEEN_BYTE_ALIGNMENT".into(),
        ];

        let layout = vec![
            self.mesh_pipeline.view_layout.clone(),
            self.uniform_layout.clone(),
        ];

        let fragment_entry_point = match key.line_style {
            GizmoLineStyle::Solid => "fragment_solid",
            GizmoLineStyle::Dotted => "fragment_dotted",
            GizmoLineStyle::Dashed { .. } => "fragment_dashed",
        };

        RenderPipelineDescriptor {
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: "vertex".into(),
                shader_defs: shader_defs.clone(),
                buffers: line_gizmo_vertex_buffer_layouts(key.strip),
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs,
                entry_point: fragment_entry_point.into(),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            layout,
            primitive: PrimitiveState::default(),
            depth_stencil: Some(DepthStencilState {
                format: CORE_2D_DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: CompareFunction::Always,
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: key.mesh_key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            label: Some("LineGizmo Pipeline 2D".into()),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        }
    }
}

#[derive(Clone, Resource)]
struct LineJointGizmoPipeline {
    mesh_pipeline: Mesh2dPipeline,
    uniform_layout: BindGroupLayout,
    shader: Handle<Shader>,
}

impl FromWorld for LineJointGizmoPipeline {
    fn from_world(render_world: &mut World) -> Self {
        LineJointGizmoPipeline {
            mesh_pipeline: render_world.resource::<Mesh2dPipeline>().clone(),
            uniform_layout: render_world
                .resource::<LineGizmoUniformBindgroupLayout>()
                .layout
                .clone(),
            shader: load_embedded_asset!(render_world, "line_joints.wgsl"),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct LineJointGizmoPipelineKey {
    mesh_key: Mesh2dPipelineKey,
    joints: GizmoLineJoint,
}

impl SpecializedRenderPipeline for LineJointGizmoPipeline {
    type Key = LineJointGizmoPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let format = if key.mesh_key.contains(Mesh2dPipelineKey::HDR) {
            ViewTarget::TEXTURE_FORMAT_HDR
        } else {
            TextureFormat::bevy_default()
        };

        let shader_defs = vec![
            #[cfg(feature = "webgl")]
            "SIXTEEN_BYTE_ALIGNMENT".into(),
        ];

        let layout = vec![
            self.mesh_pipeline.view_layout.clone(),
            self.uniform_layout.clone(),
        ];

        if key.joints == GizmoLineJoint::None {
            error!("There is no entry point for line joints with GizmoLineJoints::None. Please consider aborting the drawing process before reaching this stage.");
        };

        let entry_point = match key.joints {
            GizmoLineJoint::Miter => "vertex_miter",
            GizmoLineJoint::Round(_) => "vertex_round",
            GizmoLineJoint::None | GizmoLineJoint::Bevel => "vertex_bevel",
        };

        RenderPipelineDescriptor {
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: entry_point.into(),
                shader_defs: shader_defs.clone(),
                buffers: line_joint_gizmo_vertex_buffer_layouts(),
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs,
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            layout,
            primitive: PrimitiveState::default(),
            depth_stencil: Some(DepthStencilState {
                format: CORE_2D_DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: CompareFunction::Always,
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: key.mesh_key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            label: Some("LineJointGizmo Pipeline 2D".into()),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        }
    }
}

type DrawLineGizmo2d = (
    SetItemPipeline,
    SetMesh2dViewBindGroup<0>,
    SetLineGizmoBindGroup<1>,
    DrawLineGizmo<false>,
);
type DrawLineGizmo2dStrip = (
    SetItemPipeline,
    SetMesh2dViewBindGroup<0>,
    SetLineGizmoBindGroup<1>,
    DrawLineGizmo<true>,
);
type DrawLineJointGizmo2d = (
    SetItemPipeline,
    SetMesh2dViewBindGroup<0>,
    SetLineGizmoBindGroup<1>,
    DrawLineJointGizmo,
);

fn queue_line_gizmos_2d(
    draw_functions: Res<DrawFunctions<Transparent2d>>,
    pipeline: Res<LineGizmoPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<LineGizmoPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    line_gizmos: Query<(Entity, &MainEntity, &GizmoMeshConfig)>,
    line_gizmo_assets: Res<RenderAssets<GpuLineGizmo>>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent2d>>,
    mut views: Query<(&ExtractedView, &Msaa, Option<&RenderLayers>)>,
) {
    let draw_function = draw_functions.read().get_id::<DrawLineGizmo2d>().unwrap();
    let draw_function_strip = draw_functions
        .read()
        .get_id::<DrawLineGizmo2dStrip>()
        .unwrap();

    for (view, msaa, render_layers) in &mut views {
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view.retained_view_entity)
        else {
            continue;
        };

        let mesh_key = Mesh2dPipelineKey::from_msaa_samples(msaa.samples())
            | Mesh2dPipelineKey::from_hdr(view.hdr);

        let render_layers = render_layers.unwrap_or_default();
        for (entity, main_entity, config) in &line_gizmos {
            if !config.render_layers.intersects(render_layers) {
                continue;
            }

            let Some(line_gizmo) = line_gizmo_assets.get(&config.handle) else {
                continue;
            };

            if line_gizmo.list_vertex_count > 0 {
                let pipeline = pipelines.specialize(
                    &pipeline_cache,
                    &pipeline,
                    LineGizmoPipelineKey {
                        mesh_key,
                        strip: false,
                        line_style: config.line_style,
                    },
                );
                transparent_phase.add(Transparent2d {
                    entity: (entity, *main_entity),
                    draw_function,
                    pipeline,
                    sort_key: FloatOrd(f32::INFINITY),
                    batch_range: 0..1,
                    extra_index: PhaseItemExtraIndex::None,
                    extracted_index: usize::MAX,
                    indexed: false,
                });
            }

            if line_gizmo.strip_vertex_count >= 2 {
                let pipeline = pipelines.specialize(
                    &pipeline_cache,
                    &pipeline,
                    LineGizmoPipelineKey {
                        mesh_key,
                        strip: true,
                        line_style: config.line_style,
                    },
                );
                transparent_phase.add(Transparent2d {
                    entity: (entity, *main_entity),
                    draw_function: draw_function_strip,
                    pipeline,
                    sort_key: FloatOrd(f32::INFINITY),
                    batch_range: 0..1,
                    extra_index: PhaseItemExtraIndex::None,
                    extracted_index: usize::MAX,
                    indexed: false,
                });
            }
        }
    }
}
fn queue_line_joint_gizmos_2d(
    draw_functions: Res<DrawFunctions<Transparent2d>>,
    pipeline: Res<LineJointGizmoPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<LineJointGizmoPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    line_gizmos: Query<(Entity, &MainEntity, &GizmoMeshConfig)>,
    line_gizmo_assets: Res<RenderAssets<GpuLineGizmo>>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent2d>>,
    mut views: Query<(&ExtractedView, &Msaa, Option<&RenderLayers>)>,
) {
    let draw_function = draw_functions
        .read()
        .get_id::<DrawLineJointGizmo2d>()
        .unwrap();

    for (view, msaa, render_layers) in &mut views {
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view.retained_view_entity)
        else {
            continue;
        };

        let mesh_key = Mesh2dPipelineKey::from_msaa_samples(msaa.samples())
            | Mesh2dPipelineKey::from_hdr(view.hdr);

        let render_layers = render_layers.unwrap_or_default();
        for (entity, main_entity, config) in &line_gizmos {
            if !config.render_layers.intersects(render_layers) {
                continue;
            }

            let Some(line_gizmo) = line_gizmo_assets.get(&config.handle) else {
                continue;
            };

            if line_gizmo.strip_vertex_count < 3 || config.line_joints == GizmoLineJoint::None {
                continue;
            }

            let pipeline = pipelines.specialize(
                &pipeline_cache,
                &pipeline,
                LineJointGizmoPipelineKey {
                    mesh_key,
                    joints: config.line_joints,
                },
            );
            transparent_phase.add(Transparent2d {
                entity: (entity, *main_entity),
                draw_function,
                pipeline,
                sort_key: FloatOrd(f32::INFINITY),
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                extracted_index: usize::MAX,
                indexed: false,
            });
        }
    }
}
