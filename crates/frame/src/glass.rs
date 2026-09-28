pub const TINT: [f32; 4] = [0.075, 0.085, 0.095, 0.66];
pub const BORDER: [f32; 4] = [0.51, 0.56, 0.60, 0.48];

/// Viewport coordinates on the same 854 x 480 canvas as the pause menu.
/// Negative origins anchor to the right or bottom edge.
pub const HUD_PANELS: [[f32; 4]; 7] = [
    [13.0, 10.0, 124.0, 122.0],
    [15.0, -89.0, 190.0, 71.0],
    [-245.0, -72.0, 230.0, 53.0],
    [-270.0, 13.0, 270.0, 19.0],
    [-75.0, -35.0, 13.0, 13.0],
    [-40.0, -35.0, 13.0, 13.0],
    [0.0, 0.0, 0.0, 0.0],
];
