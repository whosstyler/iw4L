use bevy::prelude::Resource;
use math_iw4::{angles_to_axis, axis_to_angles, matrix_multiply};
use weapon_iw4::WeaponState;

use crate::anim::fpv_pose::FpvBoltFrame;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InspectPose {
    pub angles: [f32; 3],
    pub shift: [f32; 3],
}

impl InspectPose {
    fn channels(self) -> [f32; 6] {
        let [p, y, r] = self.angles;
        let [f, s, u] = self.shift;
        [p, y, r, f, s, u]
    }

    fn from_channels(c: [f32; 6]) -> Self {
        Self {
            angles: [c[0], c[1], c[2]],
            shift: [c[3], c[4], c[5]],
        }
    }

    fn scaled(self, weight: f32) -> Self {
        Self::from_channels(self.channels().map(|c| c * weight))
    }
}

struct InspectKey {
    time: f32,
    pose: InspectPose,
}

const fn key(time: f32, angles: [f32; 3], shift: [f32; 3]) -> InspectKey {
    InspectKey {
        time,
        pose: InspectPose { angles, shift },
    }
}

const INSPECT_KEYS: [InspectKey; 6] = [
    key(0.0, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
    key(0.45, [-10.0, 12.0, -40.0], [1.0, -2.5, 2.0]),
    key(1.35, [-12.0, 15.0, -46.0], [1.2, -2.8, 2.2]),
    key(1.9, [-4.0, 32.0, 14.0], [2.6, -3.0, 1.2]),
    key(2.7, [-6.0, 29.0, 17.0], [2.4, -2.9, 1.4]),
    key(3.25, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
];

const INSPECT_BLEND_OUT_SECS: f32 = 0.15;

const INSPECT_STALE_MS: i32 = 250;

const PIVOT_FORWARD_FRACTION: f32 = 0.55;

const FALLBACK_PIVOT_FORWARD_LEFT_UP: [f32; 3] = [12.0, -4.0, -3.0];

const MUZZLE_FORWARD_RANGE: core::ops::RangeInclusive<f32> = 4.0..=60.0;

const MUZZLE_LATERAL_LIMIT: f32 = 20.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum InspectPhase {
    #[default]
    Idle,
    Playing {
        elapsed: f32,
    },
    Leaving {
        from: InspectPose,
        remaining: f32,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct InspectInputs {
    pub pressed: bool,
    pub interrupted: bool,
    pub weapon: u32,
    pub weaponstate: i32,
    pub weaponstate_secondary: i32,
    pub weapon_pos_frac: f32,
    pub time_ms: i32,
    pub dt_secs: f32,
}

impl InspectInputs {
    fn weapon_settled(&self) -> bool {
        let ready = WeaponState::Ready as i32;
        self.weapon != 0
            && !self.interrupted
            && self.weaponstate == ready
            && self.weaponstate_secondary == ready
            && self.weapon_pos_frac <= 0.0
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct WeaponInspect {
    phase: InspectPhase,
    weapon: u32,
    held: bool,
    last_time_ms: Option<i32>,
}

impl WeaponInspect {
    pub fn advance(&mut self, input: InspectInputs) -> Option<InspectPose> {
        let rising = input.pressed && !self.held;
        self.held = input.pressed;
        let stale = self.last_time_ms.is_some_and(|last| {
            !(0..=INSPECT_STALE_MS).contains(&input.time_ms.wrapping_sub(last))
        });
        self.last_time_ms = Some(input.time_ms);
        if stale {
            self.phase = InspectPhase::Idle;
        }
        let dt = if input.dt_secs.is_finite() {
            input.dt_secs.max(0.0)
        } else {
            0.0
        };
        let settled = input.weapon_settled();
        match self.phase {
            InspectPhase::Idle => {
                if rising && settled {
                    self.weapon = input.weapon;
                    self.phase = InspectPhase::Playing { elapsed: 0.0 };
                }
                None
            }
            InspectPhase::Playing { elapsed } => {
                if input.weapon != self.weapon {
                    self.phase = InspectPhase::Idle;
                    return None;
                }
                if !settled {
                    let from = sample_inspect(elapsed);
                    self.phase = InspectPhase::Leaving {
                        from,
                        remaining: INSPECT_BLEND_OUT_SECS,
                    };
                    return Some(from);
                }
                let elapsed = elapsed + dt;
                if elapsed >= inspect_duration_secs() {
                    self.phase = InspectPhase::Idle;
                    return None;
                }
                self.phase = InspectPhase::Playing { elapsed };
                Some(sample_inspect(elapsed))
            }
            InspectPhase::Leaving { from, remaining } => {
                let remaining = remaining - dt;
                if remaining <= 0.0 || input.weapon != self.weapon {
                    self.phase = InspectPhase::Idle;
                    return None;
                }
                self.phase = InspectPhase::Leaving { from, remaining };
                Some(from.scaled(smoothstep(remaining / INSPECT_BLEND_OUT_SECS)))
            }
        }
    }
}

pub fn inspect_duration_secs() -> f32 {
    INSPECT_KEYS[INSPECT_KEYS.len() - 1].time
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn secant(a: &InspectKey, b: &InspectKey, channel: usize) -> f32 {
    (b.pose.channels()[channel] - a.pose.channels()[channel]) / (b.time - a.time)
}

fn monotone_tangent(index: usize, channel: usize) -> f32 {
    if index == 0 || index + 1 >= INSPECT_KEYS.len() {
        return 0.0;
    }
    let before = secant(&INSPECT_KEYS[index - 1], &INSPECT_KEYS[index], channel);
    let after = secant(&INSPECT_KEYS[index], &INSPECT_KEYS[index + 1], channel);
    if before * after <= 0.0 {
        return 0.0;
    }
    let limit = 3.0 * before.abs().min(after.abs());
    (0.5 * (before + after)).clamp(-limit, limit)
}

pub fn sample_inspect(time_secs: f32) -> InspectPose {
    let last = INSPECT_KEYS.len() - 1;
    if !time_secs.is_finite() || time_secs <= INSPECT_KEYS[0].time {
        return INSPECT_KEYS[0].pose;
    }
    if time_secs >= INSPECT_KEYS[last].time {
        return INSPECT_KEYS[last].pose;
    }
    let index = INSPECT_KEYS
        .windows(2)
        .position(|pair| time_secs < pair[1].time)
        .unwrap_or(last - 1);
    let a = &INSPECT_KEYS[index];
    let b = &INSPECT_KEYS[index + 1];
    let span = b.time - a.time;
    let t = (time_secs - a.time) / span;
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    let pa = a.pose.channels();
    let pb = b.pose.channels();
    let mut out = [0.0; 6];
    for (channel, value) in out.iter_mut().enumerate() {
        let ma = monotone_tangent(index, channel) * span;
        let mb = monotone_tangent(index + 1, channel) * span;
        *value = h00 * pa[channel] + h10 * ma + h01 * pb[channel] + h11 * mb;
    }
    InspectPose::from_channels(out)
}

fn forward_left_up(origin_forward_right_up: [f32; 3]) -> [f32; 3] {
    let [f, r, u] = origin_forward_right_up;
    [f, -r, u]
}

fn rotate(axis: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    core::array::from_fn(|i| v[0] * axis[0][i] + v[1] * axis[1][i] + v[2] * axis[2][i])
}

pub fn inspect_pivot_local(bolt: Option<&FpvBoltFrame>) -> [f32; 3] {
    let muzzle = bolt.and_then(|frame| {
        let bone = frame.tags.flash.or(frame.tags.flash_silenced)?;
        let t = frame.bones.get(usize::from(bone))?.w_axis;
        Some([-t.z, -t.x, t.y])
    });
    match muzzle {
        Some([f, l, u])
            if MUZZLE_FORWARD_RANGE.contains(&f)
                && l.abs() <= MUZZLE_LATERAL_LIMIT
                && u.abs() <= MUZZLE_LATERAL_LIMIT =>
        {
            [f * PIVOT_FORWARD_FRACTION, l, u]
        }
        _ => FALLBACK_PIVOT_FORWARD_LEFT_UP,
    }
}

pub fn apply_inspect_pose(
    origin: [f32; 3],
    angles: [f32; 3],
    pivot_local: [f32; 3],
    pose: InspectPose,
) -> ([f32; 3], [f32; 3]) {
    let gun_axis = angles_to_axis(angles);
    let inspect_axis = angles_to_axis(pose.angles);
    let o = forward_left_up(origin);
    let arm = rotate(gun_axis, pivot_local);
    let pivot: [f32; 3] = core::array::from_fn(|i| o[i] + arm[i]);
    let swung = rotate(inspect_axis, core::array::from_fn(|i| o[i] - pivot[i]));
    let shift = forward_left_up(pose.shift);
    let placed: [f32; 3] = core::array::from_fn(|i| pivot[i] + swung[i] + shift[i]);
    let angles = axis_to_angles(matrix_multiply(gun_axis, inspect_axis));
    (forward_left_up(placed), angles)
}
