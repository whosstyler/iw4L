//! Pause-screen layout. Coordinates follow the 854 x 480 design, with edge anchors.
use crate::model::{
    Canvas, Content, Enabled, Modality, Rect640, Screen, ScreenCmd, Style, UiIntent, Widget,
};
use crate::nav::{SelectionBar, UI_PASS_FOCUS};
use crate::screens::InGameMenuInfo;
use bevy::prelude::*;

const WHITE: [f32; 4] = [0.95, 0.96, 0.97, 1.0];
const MUTED: [f32; 4] = [0.69, 0.74, 0.79, 1.0];
pub(crate) const PANELS: [[f32; 4]; 3] = [
    [38.0, 83.0, 200.0, 217.0],
    [-260.0, 68.0, 238.0, 156.0],
    [-260.0, 233.0, 238.0, 201.0],
];

pub(crate) fn widget(id: &str, r: [f32; 4], text: &str, size: f32) -> Widget {
    Widget {
        id: id.into(),
        rect: Rect640 {
            x: r[0],
            y: r[1],
            w: r[2],
            h: r[3],
            horz_align: if r[0] < 0.0 { 3 } else { 1 },
            vert_align: 1,
        },
        style: Style {
            modern: true,
            canvas: Canvas::Viewport,
            fore_color: WHITE,
            text_key: text.into(),
            text_scale: size / 48.0,
            letter_spacing: 0.45,
            ..Default::default()
        },
        content: Content::Label,
        focusable: false,
        enabled: Enabled::Always,
        on_focus: vec![],
        on_activate: vec![],
        icon: String::new(),
        help: None,
        focus_order: None,
    }
}
pub(crate) fn panel(id: &str, rect: [f32; 4], rounded: bool) -> Widget {
    let mut w = widget(id, rect, "", 0.0);
    w.content = Content::Panel;
    // The scene shader supplies the frost; this tint also provides a readable fallback.
    w.style.fore_color = frame::glass::TINT;
    w.style.border_color = frame::glass::BORDER;
    w.style.corner_radius = if rounded { 2.5 } else { 0.0 };
    w
}
pub(crate) fn art(id: &str, rect: [f32; 4], image: &str) -> Widget {
    let mut w = widget(id, rect, "", 0.0);
    w.content = Content::Image;
    w.style.background = image.into();
    w.style.image_contain = true;
    w
}
pub(crate) fn line(id: &str, r: [f32; 4]) -> Widget {
    let mut w = panel(id, r, false);
    w.style.fore_color = [0.54, 0.59, 0.62, 0.44];
    w.style.border_color = [0.0; 4];
    w
}
pub(crate) fn screen(id: &str, widgets: Vec<Widget>) -> Screen {
    Screen {
        id: id.into(),
        layer: crate::UiLayer::Shell,
        modality: Modality::Opaque,
        background: None,
        bed: None,
        widgets,
        focus_overrides: vec![],
        on_open: vec![],
        on_back: vec![ScreenCmd::Back],
    }
}
pub(crate) fn base() -> Vec<Widget> {
    let mut dim = panel("ingame_options/dim", [0.0, 0.0, 640.0, 480.0], false);
    dim.rect.horz_align = 4;
    dim.rect.vert_align = 4;
    dim.style.fore_color = [0.0, 0.0, 0.0, 0.22];
    dim.style.border_color = [0.0; 4];
    vec![dim]
}
fn footer(widgets: &mut Vec<Widget>, full: bool) {
    for (index, (key, name, x)) in [
        ("ESC", "Back", 32.0),
        ("F1", "View Map", 107.0),
        ("F2", "Social", 191.0),
    ]
    .into_iter()
    .enumerate()
    {
        if !full && index > 0 {
            break;
        }
        let mut cap = panel(
            &format!("pause/key/{key}"),
            [x, -31.0, if index == 0 { 24.0 } else { 18.0 }, 15.0],
            true,
        );
        cap.rect.vert_align = 3;
        cap.style.text_key = key.into();
        cap.style.text_scale = 8.0 / 48.0;
        cap.style.text_align_x = 3.5;
        cap.style.text_align_y = 2.0;
        // Panel fore_color is its fill, so use a separate text label.
        let mut text = widget(
            &format!("pause/key_label/{key}"),
            [x + 3.5, -29.0, 22.0, 13.0],
            key,
            8.0,
        );
        text.rect.vert_align = 3;
        cap.style.text_key.clear();
        let mut hint = widget(
            &format!("pause/hint/{key}"),
            [x + if index == 0 { 31.0 } else { 25.0 }, -29.0, 55.0, 14.0],
            name,
            8.0,
        );
        hint.rect.vert_align = 3;
        widgets.extend([cap, text, hint]);
        if full && index < 2 {
            let mut divider = line(
                &format!("pause/footer_divider/{index}"),
                [x + if index == 0 { 61.0 } else { 71.0 }, -28.0, 0.5, 9.0],
            );
            divider.rect.vert_align = 3;
            widgets.push(divider);
        }
    }
}

pub(crate) fn options(info: Option<&InGameMenuInfo>) -> Screen {
    let empty = InGameMenuInfo::default();
    let info = info.unwrap_or(&empty);
    let map = if info.map.is_empty() {
        "UNKNOWN MAP"
    } else {
        &info.map
    };
    let mode = info.mode.to_uppercase();
    let mut widgets = base();
    let mut crumb = widget(
        "pause/breadcrumb",
        [38.0, 23.0, 300.0, 12.0],
        &format!("M U L T I P L A Y E R   |   {}", map.to_uppercase()),
        7.0,
    );
    crumb.style.fore_color = MUTED;
    let mut title = widget(
        "pause/title",
        [36.0, 35.0, 370.0, 38.0],
        "MATCH PAUSED",
        27.0,
    );
    title.style.bold = true;
    widgets.extend([crumb, title]);
    let clock = chrono::Local::now().format("%-I:%M %p").to_string();
    widgets.push(widget(
        "pause/session",
        [-260.0, 21.0, 238.0, 14.0],
        &format!("{}   |   {}   |   {clock}", info.zone.to_uppercase(), mode),
        6.6,
    ));
    for (i, (id, name, icon, cmd)) in [
        (
            "resume",
            "RESUME MATCH",
            "play",
            ScreenCmd::Emit(UiIntent::ResumeMatch),
        ),
        (
            "choose_class",
            "LOADOUTS",
            "rifle",
            ScreenCmd::Open("ingame_class".into()),
        ),
        (
            "scoreboard",
            "SCOREBOARD",
            "people",
            ScreenCmd::Open("pause_scoreboard".into()),
        ),
        (
            "options",
            "SETTINGS",
            "gear",
            ScreenCmd::Open("options".into()),
        ),
        (
            "social",
            "SOCIAL",
            "people",
            ScreenCmd::Open("pause_social".into()),
        ),
        (
            "leave",
            "LEAVE MATCH",
            "exit",
            ScreenCmd::Open("leave_game".into()),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 83.0 + i as f32 * 36.2;
        let mut row = widget(
            &format!("ingame_options/{id}"),
            [38.0, y, 200.0, 36.2],
            name,
            10.5,
        );
        row.content = Content::Button;
        row.focusable = true;
        row.focus_order = Some(i as u32);
        row.style.fill_color = [0.075, 0.085, 0.095, 0.65];
        row.style.border_color = frame::glass::BORDER;
        row.style.text_align_x = 42.0;
        row.style.text_align_y = 12.0;
        row.style.bold = i == 0;
        row.on_activate = vec![cmd];
        row.on_focus = vec![ScreenCmd::PlaySound("mouse_over".into())];
        widgets.push(row);
        let mut icon = art(
            &format!("pause/icon/{id}"),
            [49.0, y + 10.0, 18.0, 18.0],
            &format!("pause_icon_{icon}"),
        );
        icon.style.fore_color = if i == 0 { WHITE } else { MUTED };
        widgets.push(icon);
    }
    widgets.push(panel("pause/match_card", PANELS[1], true));
    widgets.push(widget(
        "pause/mode",
        [-247.0, 80.0, 210.0, 16.0],
        &mode,
        9.0,
    ));
    widgets.push(art(
        "pause/emblem",
        [-247.0, 97.0, 34.0, 35.0],
        info.icon
            .as_ref()
            .map(|k| k.display())
            .as_deref()
            .unwrap_or("pause_icon_skull"),
    ));
    let mut map_label = widget(
        "pause/map_name",
        [-199.0, 102.0, 169.0, 28.0],
        &map.to_uppercase(),
        19.0,
    );
    map_label.style.bold = true;
    // Fit long localized map names without crossing the card edge.
    map_label.style.text_scale *= (14.0 / map.chars().count().max(1) as f32).min(1.0);
    widgets.push(map_label);
    let mut desc = widget(
        "pause/description",
        [-247.0, 138.0, 211.0, 36.0],
        info.description
            .as_deref()
            .unwrap_or("Eliminate opponents and complete the match objectives."),
        9.0,
    );
    desc.style.text_wrap = true;
    desc.style.fore_color = [0.79, 0.82, 0.85, 1.0];
    widgets.push(desc);
    widgets.push(line("pause/divider", [-246.0, 175.0, 208.0, 0.6]));
    let score = info
        .score_limit
        .map(|n| if n > 0 { n.to_string() } else { "NONE".into() })
        .unwrap_or_else(|| "—".into());
    let limit = info
        .time_limit_ms
        .map(|ms| {
            if ms > 0 {
                format!("{}:{:02}", ms / 60000, ms / 1000 % 60)
            } else {
                "NONE".into()
            }
        })
        .unwrap_or_else(|| "—".into());
    for (i, (label, value, x)) in [
        ("SCORE LIMIT", score, -246.0),
        ("PLAYERS", info.players.len().to_string(), -161.0),
        ("TIME LIMIT", limit, -91.0),
    ]
    .into_iter()
    .enumerate()
    {
        widgets.push(widget(
            &format!("pause/stat_label/{i}"),
            [x, 186.0, 72.0, 12.0],
            label,
            6.7,
        ));
        let mut value = widget(
            &format!("pause/stat/{i}"),
            [x, 197.0, 69.0, 18.0],
            &value,
            12.0,
        );
        value.style.bold = true;
        widgets.push(value);
    }
    widgets.push(line("pause/stat_divider/0", [-177.0, 189.0, 0.6, 20.0]));
    widgets.push(line("pause/stat_divider/1", [-103.0, 189.0, 0.6, 20.0]));
    widgets.push(panel("pause/map_card", PANELS[2], true));
    widgets.push(widget(
        "pause/map_heading",
        [-247.0, 240.0, 184.0, 16.0],
        "MAP OVERVIEW",
        8.5,
    ));
    widgets.push(widget("pause/north", [-41.0, 245.0, 14.0, 15.0], "N", 8.0));
    widgets.push(art(
        "pause/north_arrow",
        [-42.0, 256.0, 9.0, 9.0],
        "pause_icon_north",
    ));
    if let Some(compass) = &info.compass {
        let mut map = art("pause/map", [-247.0, 255.0, 215.0, 168.0], compass);
        // Preserve the entire map, even when a map's compass asset is square.
        map.style.image_contain = true;
        widgets.push(map);
    } else {
        widgets.push(widget(
            "pause/no_map",
            [-226.0, 321.0, 170.0, 25.0],
            "Map overview unavailable",
            9.0,
        ));
    }
    footer(&mut widgets, true);
    screen("ingame_options", widgets)
}

pub(crate) fn detail(id: &str, info: Option<&InGameMenuInfo>) -> Screen {
    if id == "pause_map" {
        return crate::tactical::build(info.unwrap_or(&InGameMenuInfo::default()));
    }
    let mut widgets = base();
    let empty = InGameMenuInfo::default();
    let info = info.unwrap_or(&empty);
    let title = match id {
        "pause_social" => "SOCIAL",
        _ => "SCOREBOARD",
    };
    let mut heading = widget("pause/detail_title", [38.0, 35.0, 400.0, 40.0], title, 27.0);
    heading.style.bold = true;
    widgets.push(heading);
    widgets.push(panel("pause/detail_card", [38.0, 83.0, 778.0, 345.0], true));
    {
        widgets.push(widget(
            "pause/columns",
            [54.0, 98.0, 300.0, 17.0],
            "PLAYER",
            8.0,
        ));
        for (i, name) in ["SCORE", "KILLS", "DEATHS"].iter().enumerate() {
            widgets.push(widget(
                &format!("pause/column/{i}"),
                [580.0 + i as f32 * 72.0, 98.0, 65.0, 17.0],
                name,
                8.0,
            ));
        }
        if info.players.is_empty() {
            widgets.push(widget(
                "pause/empty",
                [54.0, 136.0, 500.0, 25.0],
                "No players connected",
                11.0,
            ));
        }
        for (i, player) in info.players.iter().take(18).enumerate() {
            let y = 121.0 + i as f32 * 15.5;
            widgets.push(widget(
                &format!("pause/player/{i}"),
                [54.0, y, 510.0, 16.0],
                &player.name,
                9.0,
            ));
            for (col, val) in [player.score, player.kills, player.deaths]
                .iter()
                .enumerate()
            {
                widgets.push(widget(
                    &format!("pause/player/{i}/{col}"),
                    [580.0 + col as f32 * 72.0, y, 65.0, 16.0],
                    &val.to_string(),
                    9.0,
                ));
            }
        }
    }
    footer(&mut widgets, false);
    screen(id, widgets)
}

pub(crate) fn spawn_selection(parent: &mut ChildSpawnerCommands, scale: f32) {
    parent
        .spawn((
            SelectionBar,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(0.7 * scale)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.24, 0.56, 0.65, 0.22)),
            BorderColor::all(Color::srgba(0.48, 0.89, 0.98, 0.95)),
            BoxShadow(vec![ShadowStyle {
                color: Color::srgba(0.22, 0.76, 0.90, 0.22),
                x_offset: Val::ZERO,
                y_offset: Val::ZERO,
                blur_radius: Val::Px(9.0 * scale),
                ..default()
            }]),
            Visibility::Hidden,
            UI_PASS_FOCUS,
            Pickable::IGNORE,
        ))
        .with_child((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(1.6 * scale),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.53, 0.96, 1.0)),
            UI_PASS_FOCUS,
            Pickable::IGNORE,
        ));
}

// Small original monochrome UI symbols, supersampled for sharp scaling without external assets.
pub(crate) fn generated_image(stem: &str) -> Option<(u32, u32, Vec<u8>)> {
    let kind = stem.strip_prefix("pause_icon_")?;
    let mut pixels = vec![0; 64 * 64 * 4];
    for y in 0..64 {
        for x in 0..64 {
            let mut n = 0;
            for sy in 0..3 {
                for sx in 0..3 {
                    let p = Vec2::new(
                        (x as f32 + (sx as f32 + 0.5) / 3.0) / 64.0,
                        (y as f32 + (sy as f32 + 0.5) / 3.0) / 64.0,
                    );
                    n += symbol(kind, p) as u32;
                }
            }
            let i = (y * 64 + x) * 4;
            pixels[i..i + 4].copy_from_slice(&[255, 255, 255, (n * 255 / 9) as u8]);
        }
    }
    Some((64, 64, pixels))
}
fn polygon(p: Vec2, points: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let [x, y] = points[i];
        let [px, py] = points[j];
        if (y > p.y) != (py > p.y) && p.x < (px - x) * (p.y - y) / (py - y) + x {
            inside = !inside;
        }
        j = i;
    }
    inside
}
fn symbol(kind: &str, p: Vec2) -> bool {
    let circle = |x: f32, y: f32, r: f32| p.distance(Vec2::new(x, y)) < r;
    let rect = |x: f32, y: f32, w: f32, h: f32| p.x > x && p.x < x + w && p.y > y && p.y < y + h;
    match kind {
        "dot" => circle(0.5, 0.5, 0.45),
        "cone" => p.y < 0.5 && (p.x - 0.5).abs() < (0.5 - p.y) * 0.7 && circle(0.5, 0.5, 0.49),
        "play" => polygon(p, &[[0.25, 0.15], [0.80, 0.5], [0.25, 0.85]]),
        "player" => {
            p.y > 0.08
                && p.y < 0.87
                && (p.x - 0.5).abs() < (p.y - 0.08) * 0.46
                && !(p.y > 0.42 && (p.x - 0.5).abs() < (p.y - 0.42) * 0.45)
        }
        "north" => polygon(p, &[[0.5, 0.12], [0.90, 0.88], [0.5, 0.70], [0.1, 0.88]]),
        "people" => {
            circle(0.34, 0.30, 0.14)
                || circle(0.70, 0.38, 0.11)
                || (circle(0.34, 0.83, 0.29) && p.y < 0.80)
                || (circle(0.70, 0.82, 0.22) && p.y < 0.80)
        }
        "gear" => {
            let d = p - Vec2::splat(0.5);
            let a = d.y.atan2(d.x);
            let r = d.length();
            r > 0.16 && r < if (a * 8.0).cos() > 0.15 { 0.40 } else { 0.31 }
        }
        "exit" => {
            rect(0.18, 0.14, 0.56, 0.075)
                || rect(0.18, 0.79, 0.56, 0.075)
                || rect(0.67, 0.14, 0.075, 0.70)
                || rect(0.18, 0.14, 0.075, 0.21)
                || rect(0.18, 0.67, 0.075, 0.19)
                || rect(0.10, 0.46, 0.43, 0.08)
                || polygon(
                    p,
                    &[
                        [0.41, 0.31],
                        [0.63, 0.50],
                        [0.41, 0.69],
                        [0.41, 0.58],
                        [0.51, 0.50],
                        [0.41, 0.42],
                    ],
                )
        }
        "rifle" => {
            polygon(
                p,
                &[
                    [0.08, 0.48],
                    [0.26, 0.43],
                    [0.31, 0.36],
                    [0.66, 0.36],
                    [0.66, 0.42],
                    [0.93, 0.42],
                    [0.93, 0.47],
                    [0.65, 0.47],
                    [0.61, 0.52],
                    [0.56, 0.53],
                    [0.64, 0.74],
                    [0.52, 0.76],
                    [0.45, 0.55],
                    [0.37, 0.55],
                    [0.33, 0.73],
                    [0.24, 0.70],
                    [0.27, 0.52],
                    [0.10, 0.64],
                ],
            ) || rect(0.37, 0.29, 0.25, 0.035)
        }
        "skull" => {
            let hex = |r: f32| {
                polygon(
                    p,
                    &[
                        [0.5, 0.5 - r],
                        [0.5 + r * 0.87, 0.5 - r * 0.5],
                        [0.5 + r * 0.87, 0.5 + r * 0.5],
                        [0.5, 0.5 + r],
                        [0.5 - r * 0.87, 0.5 + r * 0.5],
                        [0.5 - r * 0.87, 0.5 - r * 0.5],
                    ],
                )
            };
            (hex(0.49) && !hex(0.45))
                || (hex(0.41) && !hex(0.40))
                || ((circle(0.5, 0.44, 0.23) || rect(0.36, 0.51, 0.28, 0.23))
                    && !circle(0.40, 0.48, 0.067)
                    && !circle(0.60, 0.48, 0.067)
                    && !polygon(p, &[[0.5, 0.54], [0.55, 0.63], [0.45, 0.63]])
                    && !(p.y > 0.66 && ((p.x * 42.0) as i32 % 3 == 0)))
        }
        _ => false,
    }
}

pub(crate) fn resume_match(
    mut intents: MessageReader<UiIntent>,
    mut enabled: ResMut<crate::menu::MenuEnabled>,
    mut stack: ResMut<crate::retail_menu::RetailMenuStack>,
    mut focus: ResMut<crate::nav::Focus>,
) {
    if intents
        .read()
        .any(|intent| *intent == UiIntent::ResumeMatch)
        && let Some(index) = stack.names.iter().position(|name| name == "ingame_options")
    {
        stack.names.truncate(index);
        enabled.0 = false;
        focus.widget = None;
    }
}

// Refresh the clock, connected players and scores without rebuilding UI every frame.
pub(crate) fn refresh_match_details(
    time: Res<Time>,
    enabled: Res<crate::menu::MenuEnabled>,
    stack: Res<crate::retail_menu::RetailMenuStack>,
    mut revision: ResMut<crate::menu::ShellRevision>,
    mut last: Local<f64>,
    presented: Option<Res<net::PresentedSnapshot>>,
    mut previous: Local<String>,
) {
    if enabled.0
        && stack.names.last().is_some_and(|name| {
            matches!(
                name.as_str(),
                "ingame_options" | "pause_social" | "pause_scoreboard" | "pause_map"
            )
        })
        && time.elapsed_secs_f64() - *last
            >= if stack.names.last().is_some_and(|n| n == "pause_map") {
                0.1
            } else {
                1.0
            }
    {
        *last = time.elapsed_secs_f64();
        let mut signature = chrono::Local::now().format("%Y%m%d%H%M").to_string();
        if let Some(snapshot) = presented.as_deref().and_then(|p| p.snapshot()) {
            use std::fmt::Write;
            let _ = write!(
                signature,
                "|{}|{}",
                snapshot.meta.score_limit, snapshot.meta.time_limit_ms
            );
            if stack.names.last().is_some_and(|n| n == "pause_map") {
                let _ = write!(signature, "|{}", snapshot.tick.0);
            }
            for (id, player) in &snapshot.meta.clients {
                let _ = write!(
                    signature,
                    "|{:?}:{:?}:{}:{}:{}",
                    id, player.name, player.score, player.kills, player.deaths
                );
            }
        }
        if *previous != signature {
            *previous = signature;
            revision.0 = revision.0.wrapping_add(1);
        }
    }
}
