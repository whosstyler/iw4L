use crate::gpu_list::{GpuListLatch, HudTessPass, TessJob};
use crate::images::HudImages;
use assets::{MenuCatalog, SessionTeamSettings};
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use net::{LocalPresentClient, PresentedSnapshot};
#[derive(Component)]
pub(crate) struct ScorebarRaster;
pub(crate) fn spawn_scorebar(root: &mut ChildSpawnerCommands) {
    root.spawn((
        ScorebarRaster,
        GpuListLatch::default(),
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        FocusPolicy::Pass,
    ));
}

pub(crate) fn sys_milliseconds() -> u32 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static ORIGIN: OnceLock<Instant> = OnceLock::new();

    const UPTIME_BIAS_MS: u32 = 60_000;
    ORIGIN.get_or_init(Instant::now).elapsed().as_millis() as u32 + UPTIME_BIAS_MS
}

pub(crate) fn update_scorebar(
    surface: Res<crate::surface::Hud2dSurface>,
    presented: Res<PresentedSnapshot>,
    local: Res<LocalPresentClient>,
    catalog: Option<Res<MenuCatalog>>,
    teams: Option<Res<SessionTeamSettings>>,
    mut hud_images: ResMut<HudImages>,
    mut images: ResMut<Assets<Image>>,
    mut pass: ResMut<HudTessPass>,
    view: Option<Res<frame::ViewSubject>>,
) {
    pass.scorebar = TessJob::Hide;
    if !surface.is_ready() || view.is_some_and(|v| v.in_killcam()) {
        return;
    }
    let (Some(snapshot), Some(catalog)) = (presented.snapshot(), catalog.as_deref()) else {
        return;
    };
    let quads = crate::modern::scorebar(&surface, catalog, snapshot, local.0, teams.as_deref());
    for quad in &quads {
        let _ = hud_images.get(quad.material_namespace, &quad.material, &mut images);
    }
    pass.scorebar = TessJob::Quads(quads);
}
