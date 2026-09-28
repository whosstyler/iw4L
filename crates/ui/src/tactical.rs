use crate::{
    model::Screen,
    pause::{art, base, line, panel, screen, widget},
    screens::InGameMenuInfo,
};

#[derive(Clone, Debug, Default)]
pub struct TacticalInfo {
    pub origin: [f32; 3],
    pub uv: [f32; 2],
    pub heading: f32,
    pub markers: Vec<TacticalMarker>,
}
#[derive(Clone, Debug)]
pub struct TacticalMarker {
    pub uv: [f32; 2],
    pub label: Option<String>,
}

pub(crate) fn gather(
    presented: &net::PresentedSnapshot,
    local: sim::ClientId,
    compass: &assets::SessionCompass,
) -> Option<TacticalInfo> {
    let corners = compass.corners.as_ref()?;
    let north = compass.north_yaw.unwrap_or(0.0);
    let bounds = hud_iw4::compass_map_bounds_from_minimap_corners(corners.a, corners.b, north)?;
    let ps = presented.player(local)?;
    let snap = presented.snapshot()?;
    let uv = |p: [f32; 3]| hud_iw4::compass_partial_map_uv(bounds, [p[0], p[1]], 1.0).center;
    let mut info = TacticalInfo {
        origin: ps.origin,
        uv: uv(ps.origin),
        heading: (north - ps.viewangles[1]).rem_euclid(360.0),
        markers: Vec::new(),
    };
    let team = snap.meta.for_client(local)?.client_state_team;
    if snap.meta.kind.is_team() && matches!(team, 1 | 2) {
        for (id, meta) in &snap.meta.clients {
            if *id != local
                && meta.client_state_team == team
                && let Some(other) = presented.alive_player(*id)
            {
                info.markers.push(TacticalMarker {
                    uv: uv(other.origin),
                    label: None,
                });
            }
        }
        for objective in snap.meta.objectives.flags.iter().chain(
            snap.meta
                .objectives
                .bombs
                .iter()
                .filter(|b| !b.destroyed)
                .map(|b| &b.view),
        ) {
            info.markers.push(TacticalMarker {
                uv: uv(objective.origin),
                label: Some(objective.label.clone()),
            });
        }
    }
    Some(info)
}

pub(crate) fn build(info: &InGameMenuInfo) -> Screen {
    let cyan = [0.35, 0.88, 0.96, 1.0];
    let muted = [0.59, 0.67, 0.69, 1.0];
    let mut widgets = base();
    let mut card = panel("tactical/glass", [29.0, 23.0, 418.0, 424.0], true);
    card.style.fore_color = frame::glass::TINT;
    card.style.border_color = frame::glass::BORDER;
    card.style.corner_radius = 10.0;
    widgets.push(card);
    widgets.push(line("tactical/header_line", [46.0, 60.0, 383.0, 0.6]));
    let heading = info.tactical.as_ref().map_or(0.0, |t| t.heading);
    let names = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    for step in -4..=5 {
        let index = (heading / 45.0).floor() as i32 + step;
        let x = 230.0 + (index as f32 * 45.0 - heading) * 1.95;
        if (48.0..405.0).contains(&x) {
            let mut label = widget(
                &format!("tactical/heading/{step}"),
                [x, 31.0, 32.0, 20.0],
                names[index.rem_euclid(8) as usize],
                15.0,
            );
            label.style.fore_color = if (index as f32 * 45.0 - heading).abs() < 5.0 {
                cyan
            } else {
                muted
            };
            widgets.push(label);
        }
    }
    for i in 0..=17 {
        widgets.push(line(
            &format!("tactical/tick/{i}"),
            [48.0 + i as f32 * 22.0, 53.0, 0.5, 3.0],
        ));
    }
    let mut indicator = line("tactical/heading_selected", [231.0, 56.0, 16.0, 1.6]);
    indicator.style.fore_color = cyan;
    widgets.push(indicator);
    let map_rect = [46.0, 61.0, 382.0, 318.0];
    if let Some(compass) = &info.compass {
        let mut map = art("tactical/map", map_rect, compass);
        map.style.image_contain = false;
        map.style.fore_color = [0.75, 0.82, 0.83, 0.88];
        widgets.push(map);
    } else {
        widgets.push(widget(
            "tactical/unavailable",
            [70.0, 205.0, 325.0, 28.0],
            "Map overview unavailable",
            14.0,
        ));
    }
    let position = |uv: [f32; 2]| {
        [
            map_rect[0] + uv[0] * map_rect[2],
            map_rect[1] + uv[1] * map_rect[3],
        ]
    };
    let visible = |uv: [f32; 2]| {
        uv.iter()
            .all(|v| v.is_finite() && (0.02..=0.98).contains(v))
    };
    if let Some(tactical) = &info.tactical {
        for (index, marker) in tactical.markers.iter().enumerate() {
            if !visible(marker.uv) {
                continue;
            }
            let [x, y] = position(marker.uv);
            if let Some(label) = &marker.label {
                let mut badge = panel(
                    &format!("tactical/objective/{index}"),
                    [x - 8.0, y - 8.0, 16.0, 16.0],
                    false,
                );
                badge.style.fore_color = [0.02, 0.05, 0.06, 0.75];
                badge.style.border_color = muted;
                widgets.push(badge);
                widgets.push(widget(
                    &format!("tactical/objective_text/{index}"),
                    [x - 4.0, y - 7.0, 16.0, 16.0],
                    label,
                    12.0,
                ));
            } else {
                let mut dot = art(
                    &format!("tactical/friendly/{index}"),
                    [x - 4.0, y - 4.0, 8.0, 8.0],
                    "pause_icon_dot",
                );
                dot.style.fore_color = cyan;
                widgets.push(dot);
            }
        }
        if visible(tactical.uv) {
            let [x, y] = position(tactical.uv);
            let radius = 60.0_f32
                .min(x - map_rect[0])
                .min(map_rect[0] + map_rect[2] - x)
                .min(y - map_rect[1])
                .min(map_rect[1] + map_rect[3] - y);
            let mut cone = art(
                "tactical/view_cone",
                [x - radius, y - radius, radius * 2.0, radius * 2.0],
                "pause_icon_cone",
            );
            cone.style.fore_color = [0.50, 0.85, 0.93, 0.15];
            cone.style.image_rotation_degrees = tactical.heading;
            widgets.push(cone);
            let mut glow = art(
                "tactical/player_glow",
                [x - 14.0, y - 14.0, 28.0, 28.0],
                "pause_icon_dot",
            );
            glow.style.fore_color = [0.3, 0.8, 0.98, 0.13];
            widgets.push(glow);
            let mut arrow = art(
                "tactical/player",
                [x - 10.0, y - 10.0, 20.0, 20.0],
                "pause_icon_player",
            );
            arrow.style.fore_color = cyan;
            arrow.style.image_rotation_degrees = tactical.heading;
            widgets.push(arrow);
        }
        let grid = format!(
            "GRID {}{}",
            (b'A' + (tactical.uv[0] * 8.0).floor().clamp(0.0, 7.0) as u8) as char,
            (tactical.uv[1] * 8.0).floor().clamp(0.0, 7.0) as u8 + 1
        );
        let mut stamp = panel("tactical/coordinates_bg", [352.0, 70.0, 76.0, 47.0], false);
        stamp.style.border_color = [0.0; 4];
        stamp.style.fore_color = [0.015, 0.035, 0.04, 0.82];
        widgets.push(stamp);
        for (i, text) in [
            grid,
            format!(
                "{:.1} / {:.1} m",
                tactical.origin[0] * 0.0254,
                tactical.origin[1] * 0.0254
            ),
            format!("ELEV {:.0} m", tactical.origin[2] * 0.0254),
        ]
        .iter()
        .enumerate()
        {
            let mut label = widget(
                &format!("tactical/coordinate/{i}"),
                [358.0, 76.0 + i as f32 * 11.0, 69.0, 13.0],
                text,
                7.0,
            );
            label.style.fore_color = muted;
            widgets.push(label);
        }
    }
    widgets.push(line("tactical/name_rule", [50.0, 380.0, 211.0, 0.6]));
    widgets.push(widget(
        "tactical/map_name",
        [50.0, 389.0, 260.0, 25.0],
        &info.map.to_uppercase(),
        14.0,
    ));
    widgets.push(line("tactical/mode_rule", [50.0, 416.0, 211.0, 0.6]));
    let mut mode = widget(
        "tactical/mode",
        [50.0, 420.0, 255.0, 20.0],
        &format!("OPERATION: {}", info.mode.to_uppercase()),
        8.0,
    );
    mode.style.fore_color = muted;
    widgets.push(mode);
    widgets.push(line("tactical/legend_rule", [322.0, 384.0, 0.5, 53.0]));
    for (i, (image, label)) in [
        ("pause_icon_north", "YOU"),
        ("pause_icon_dot", "TEAMMATE"),
        ("", "OBJECTIVE"),
    ]
    .iter()
    .enumerate()
    {
        let y = 386.0 + i as f32 * 17.0;
        let mut icon = if image.is_empty() {
            widget(
                "tactical/legend_objective",
                [338.0, y, 12.0, 13.0],
                "A",
                11.0,
            )
        } else {
            art(
                &format!("tactical/legend_icon/{i}"),
                [338.0, y, 12.0, 12.0],
                image,
            )
        };
        icon.style.fore_color = if i < 2 { cyan } else { muted };
        widgets.push(icon);
        let mut text = widget(
            &format!("tactical/legend_text/{i}"),
            [358.0, y + 1.0, 74.0, 13.0],
            label,
            8.0,
        );
        text.style.fore_color = muted;
        widgets.push(text);
    }
    widgets.push(widget(
        "tactical/close",
        [32.0, 456.0, 250.0, 16.0],
        "F1 / ESC   CLOSE MAP",
        8.0,
    ));
    screen("pause_map", widgets)
}
