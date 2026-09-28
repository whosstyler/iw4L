use crate::draw2d::{Draw2dCmd, Draw2dList, Draw2dOp, Draw2dProvenance, Draw2dQuad};
use crate::images::HUD_CHROME_NAMESPACE;
use crate::surface::Hud2dSurface;
use assets::MenuCatalog;

pub(crate) const CYAN: [f32; 4] = [0.35, 0.88, 0.96, 1.0];
pub(crate) const WHITE: [f32; 4] = [0.91, 0.94, 0.95, 1.0];
pub(crate) const MUTED: [f32; 4] = [0.62, 0.69, 0.72, 1.0];

pub(crate) struct Paint<'a> {
    pub list: Draw2dList,
    pub surface: &'a Hud2dSurface,

    pub scale: f32,
}
impl<'a> Paint<'a> {
    pub fn new(surface: &'a Hud2dSurface, _catalog: &MenuCatalog) -> Self {
        Self {
            list: Draw2dList::default(),
            surface,
            scale: (surface.width() / 854.0).min(surface.height() / 480.0),
        }
    }
    pub fn rect(&self, [x, y, w, h]: [f32; 4]) -> [f32; 4] {
        [
            (if x < 0.0 { self.surface.width() } else { 0.0 }) + x * self.scale,
            (if y < 0.0 { self.surface.height() } else { 0.0 }) + y * self.scale,
            w * self.scale,
            h * self.scale,
        ]
    }
    pub fn pic(&mut self, r: [f32; 4], material: &str, color: [f32; 4]) {
        let [x, y, w, h] = self.rect(r);
        let (namespace, name) = assets::AssetKey::parse(material)
            .map(|key| (key.namespace, key.name))
            .unwrap_or((HUD_CHROME_NAMESPACE, material.to_owned()));
        self.list.cmds.push(Draw2dCmd {
            x,
            y,
            w,
            h,
            s0: 0.0,
            t0: 0.0,
            s1: 1.0,
            t1: 1.0,
            color,
            material: name,
            material_namespace: namespace,
            op: Draw2dOp::StretchPic,
            provenance: Draw2dProvenance::CgDraw { site: "glass_hud" },
            layer: 1,
        });
    }
    pub fn panel(&mut self, r: [f32; 4]) {
        self.pic(r, "glass_panel", [1.0; 4]);
    }
    pub fn border(&mut self, [x, y, w, h]: [f32; 4], color: [f32; 4]) {
        for r in [
            [x, y, w, 0.5],
            [x, y + h - 0.5, w, 0.5],
            [x, y, 0.5, h],
            [x + w - 0.5, y, 0.5, h],
        ] {
            self.pic(r, "white", color);
        }
    }
    pub fn text(&mut self, x: f32, y: f32, size: f32, text: &str, color: [f32; 4], bold: bool) {
        let font = crate::glass_assets::font(bold);
        let key = font.name.as_str();
        let [x, y, _, _] = self.rect([x, y, 0.0, 0.0]);
        let scale = size * self.scale
            / font
                .glyph('H' as u32)
                .map_or(font.pixel_height.max(1) as f32, |g| {
                    g.pixel_height.max(1) as f32
                });
        self.list.cmds.push(Draw2dCmd {
            x,
            y: y + size * self.scale,
            w: scale,
            h: scale,
            s0: 0.0,
            t0: 0.0,
            s1: 1.0,
            t1: 1.0,
            color,
            material: assets::AssetRef::bare_name(&font.material).into(),
            material_namespace: HUD_CHROME_NAMESPACE,
            op: Draw2dOp::TextRun {
                font: key.into(),
                scale,
                text: text.into(),
                loc_key: String::new(),
                style: 0,
                fx: None,
                glow: None,
            },
            provenance: Draw2dProvenance::CgDraw { site: "glass_hud" },
            layer: 2,
        });
    }
    pub fn text_width(&self, text: &str, size: f32, bold: bool) -> f32 {
        let font = crate::glass_assets::font(bold);
        let height = font
            .glyph('H' as u32)
            .map_or(font.pixel_height.max(1) as f32, |g| {
                g.pixel_height.max(1) as f32
            });
        text.chars()
            .filter_map(|c| font.glyph(c as u32))
            .map(|g| g.dx as f32 * size / height)
            .sum()
    }
    pub fn text_fit(&mut self, x: f32, y: f32, width: f32, size: f32, text: &str, color: [f32; 4]) {
        let fit = (width / self.text_width(text, size, false).max(1.0)).min(1.0);
        self.text(x, y, size * fit, text, color, false);
    }
    pub fn key(&mut self, x: f32, y: f32, text: &str) {
        let width = (self.text_width(text, 7.0, false) + 6.0).max(13.0);
        self.pic([x, y, width, 13.0], "glass_key", [1.0; 4]);
        self.text(x + 3.0, y + 3.0, 7.0, text, MUTED, false);
    }
    pub fn quads(self) -> Vec<Draw2dQuad> {
        let fonts = [false, true]
            .into_iter()
            .map(|bold| {
                let font = crate::glass_assets::font(bold);
                (font.name.clone(), font)
            })
            .collect();
        crate::draw2d::tessellate_fonts(&self.list, &fonts).0
    }
}

pub(crate) fn remaining(snap: &sim::Snapshot) -> String {
    let ms = if snap.meta.kind == gamemode_iw4::GameModeKind::Demolition {
        snap.meta.objectives.round_remaining_ms
    } else {
        snap.meta
            .time_limit_ms
            .saturating_sub(snap.meta.match_elapsed_ms)
    };
    if snap.meta.time_limit_ms == 0 {
        return "--:--".into();
    }
    format!("{}:{:02}", ms / 60000, ms / 1000 % 60)
}
pub(crate) fn map_name(
    identity: Option<&frame::LaunchIdentity>,
    strings: Option<&assets::PreparedLocalizedStrings>,
) -> String {
    let zone = identity.map_or("", |i| i.zone.as_str());
    let key = format!("MPUI_{}", zone.trim_start_matches("mp_").to_uppercase());
    strings
        .and_then(|s| s.0.text(&key))
        .map(str::to_uppercase)
        .unwrap_or_else(|| {
            zone.trim_start_matches("mp_")
                .replace('_', " ")
                .to_uppercase()
        })
}

pub(crate) fn scorebar(
    surface: &Hud2dSurface,
    catalog: &MenuCatalog,
    snap: &sim::Snapshot,
    local: sim::ClientId,
    teams: Option<&assets::SessionTeamSettings>,
) -> Vec<Draw2dQuad> {
    let Some(meta) = snap.meta.for_client(local) else {
        return vec![];
    };
    let team = meta.client_state_team;
    let (mine, other) = if snap.meta.kind.is_team() && matches!(team, 1 | 2) {
        (
            snap.meta.objectives.scores[team as usize],
            snap.meta.objectives.scores[3 - team as usize],
        )
    } else {
        (
            meta.score,
            snap.meta
                .clients
                .iter()
                .filter(|(id, _)| *id != local)
                .map(|(_, m)| m.score)
                .max()
                .unwrap_or(0),
        )
    };
    let mut p = Paint::new(surface, catalog);
    p.pic(frame::glass::HUD_PANELS[1], "glass_score_panel", [1.0; 4]);
    p.pic([15.0, -71.0, 146.0, 0.5], "white", [0.4, 0.55, 0.6, 0.36]);
    p.pic([66.0, -89.0, 0.5, 18.0], "white", [0.4, 0.55, 0.6, 0.36]);
    p.text(29.0, -85.0, 10.0, &remaining(snap), WHITE, true);
    let icon = teams.and_then(|s| {
        if team == 1 {
            s.0.axis.as_ref()
        } else {
            s.0.allies.as_ref()
        }
    });
    p.pic(
        [22.0, -66.0, 40.0, 40.0],
        icon.map(|k| format!("{}:material/glass_mono_{}", k.namespace.as_str(), k.name))
            .as_deref()
            .unwrap_or("glass_skull"),
        WHITE,
    );
    let status = if mine > other {
        "WINNING"
    } else if mine < other {
        "LOSING"
    } else {
        "TIED"
    };
    p.text(
        73.0,
        -84.0,
        8.0,
        status,
        if mine < other {
            [0.96, 0.43, 0.37, 1.0]
        } else {
            CYAN
        },
        false,
    );
    for (index, score) in [mine, other].into_iter().enumerate() {
        let y = -67.0 + index as f32 * 22.0;
        let score_text = score.to_string();
        let score_size = 17.0 * (55.0 / p.text_width(&score_text, 17.0, true).max(1.0)).min(1.0);
        let bar_x = (73.0 + p.text_width(&score_text, score_size, true) + 6.0).max(102.0);
        let bar_width = 162.0 - bar_x;
        p.text(
            73.0,
            y,
            score_size,
            &score_text,
            if index == 0 { WHITE } else { MUTED },
            true,
        );
        if index == 0 {
            p.pic([bar_x - 5.0, y + 3.0, 13.0, 17.0], "glass_glow", [1.0; 4]);
        }
        p.pic(
            [bar_x, y + 9.0, bar_width, 6.0],
            "white",
            [0.07, 0.11, 0.13, 0.9],
        );
        p.border([bar_x, y + 9.0, bar_width, 6.0], [0.4, 0.5, 0.55, 0.4]);
        p.pic(
            [
                bar_x + 1.0,
                y + 10.0,
                ((bar_width - 2.0) * score.max(0) as f32 / snap.meta.score_limit.max(1) as f32)
                    .clamp(1.0, bar_width - 2.0),
                4.0,
            ],
            "white",
            if index == 0 {
                CYAN
            } else {
                [0.66, 0.26, 0.25, 1.0]
            },
        );
    }
    p.quads()
}

pub(crate) fn weaponbar(
    surface: &Hud2dSurface,
    catalog: &MenuCatalog,
    ammo: Option<&crate::ammo::WeaponbarAmmo>,
    name: Option<&str>,
    grenades: [i32; 2],
    grenade_icons: [Option<String>; 2],
    keys: Option<&frame::HudInputView>,
) -> Draw2dList {
    let mut p = Paint::new(surface, catalog);
    let Some(ammo) = ammo else {
        return p.list;
    };
    p.pic(frame::glass::HUD_PANELS[2], "glass_ammo_panel", [1.0; 4]);
    let divider = [0.51, 0.56, 0.60, 0.28];
    p.pic([-85.0, -70.0, 0.5, 41.0], "white", divider);
    p.pic([-215.0, -28.0, 130.0, 0.5], "white", divider);
    let label = name.unwrap_or("").to_uppercase();
    let label_width = p.text_width(&label, 7.5, false).min(125.0);
    let label_x = (-137.0 - label_width * 0.5).min(-94.0 - label_width);
    p.text_fit(label_x, -67.0, 125.0, 7.5, &label, WHITE);
    let clip = ammo
        .clip
        .map(|n| n.max(0).to_string())
        .unwrap_or_else(|| "--".into());
    p.text(
        -125.0 - p.text_width(&clip, 18.0, true),
        -57.0,
        18.0,
        &clip,
        if ammo
            .clip
            .is_some_and(|n| n <= (ammo.clip_size as f32 * ammo.low_ammo_warning_threshold) as i32)
        {
            [1.0, 0.42, 0.35, 1.0]
        } else {
            WHITE
        },
        true,
    );
    if ammo.dual {
        p.text(
            -218.0,
            -65.0,
            20.0,
            &ammo.clip_alt.unwrap_or(0).max(0).to_string(),
            WHITE,
            true,
        );
    }
    p.pic([-121.0, -56.0, 0.5, 16.0], "white", divider);
    if !ammo.clip_only {
        p.text(
            -117.0,
            -53.0,
            11.0,
            &ammo.stock.unwrap_or(0).max(0).to_string(),
            MUTED,
            false,
        );
    }
    let bars = ammo.clip_size.clamp(1, 60);
    let filled = ((ammo.clip.unwrap_or(0).max(0) as f32 / ammo.clip_size.max(1) as f32)
        * bars as f32)
        .ceil() as i32;
    for i in 0..bars {
        p.pic(
            [-222.0 + i as f32 * 97.0 / bars as f32, -36.0, 1.1, 6.5],
            "white",
            if i < filled {
                WHITE
            } else {
                [0.3, 0.35, 0.4, 0.5]
            },
        );
    }
    for (i, count) in grenades.into_iter().enumerate() {
        let x = -85.0 + i as f32 * 35.0;
        if i == 1 {
            p.pic([x, -70.0, 0.5, 31.0], "white", divider);
        }
        p.pic(
            [x + 5.0, -64.0, 20.0, 23.0],
            grenade_icons[i].as_deref().unwrap_or(if i == 0 {
                "glass_frag"
            } else {
                "glass_tactical"
            }),
            WHITE,
        );
        p.text(
            x + 26.0,
            -48.0,
            7.5,
            &count.max(0).to_string(),
            WHITE,
            false,
        );
        if let Some(key) = keys.and_then(|k| k.grenade_keys[i].as_deref()) {
            p.key(x + 10.0, -35.0, key);
        }
    }
    p.list
}

pub(crate) fn icon_pixels(name: &str) -> Option<(u32, u32, Vec<u8>)> {
    if !matches!(
        name,
        "glass_arrow" | "glass_dot" | "glass_frag" | "glass_tactical" | "glass_skull"
    ) {
        return None;
    }
    let mut rgba = vec![0; 64 * 64 * 4];
    for y in 0..64 {
        for x in 0..64 {
            let mut hits = 0;
            for sy in 0..3 {
                for sx in 0..3 {
                    let u = (x as f32 + (sx as f32 + 0.5) / 3.0) / 64.0;
                    let v = (y as f32 + (sy as f32 + 0.5) / 3.0) / 64.0;
                    let d = (u - 0.5).abs();
                    let inside = match name {
                        "glass_arrow" => {
                            v > 0.08
                                && v < 0.87
                                && d < (v - 0.08) * 0.46
                                && !(v > 0.42 && d < (v - 0.42) * 0.45)
                        }
                        "glass_dot" => (u - 0.5).powi(2) + (v - 0.5).powi(2) < 0.13,
                        "glass_frag" => {
                            ((u - 0.47) / 0.28).powi(2) + ((v - 0.61) / 0.31).powi(2) < 1.0
                                || (u > 0.40 && u < 0.58 && v > 0.14 && v < 0.38)
                                || (u > 0.55 && u < 0.72 && v > 0.22 && v < 0.30)
                        }
                        "glass_tactical" => {
                            (u > 0.36 && u < 0.62 && v > 0.27 && v < 0.88)
                                || (u > 0.4 && u < 0.58 && v > 0.12 && v < 0.25)
                        }
                        _ => {
                            (((u - 0.5) / 0.32).powi(2) + ((v - 0.42) / 0.32).powi(2) < 1.0
                                || (d < 0.20 && v > 0.50 && v < 0.81))
                                && !((u - 0.37).powi(2) + (v - 0.45).powi(2) < 0.009)
                                && !((u - 0.63).powi(2) + (v - 0.45).powi(2) < 0.009)
                        }
                    };
                    hits += inside as u32;
                }
            }
            let i = (y * 64 + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[255, 255, 255, (hits * 255 / 9) as u8]);
        }
    }
    Some((64, 64, rgba))
}
