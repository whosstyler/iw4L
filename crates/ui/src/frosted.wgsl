#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Glass {
    rects: array<vec4<f32>, 3>,
    viewport: vec4<f32>,
};
@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> glass: Glass;

fn inside_panel(p: vec2<f32>, expand: f32) -> bool {
    for (var i = 0u; i < 3u; i += 1u) {
        let rect = glass.rects[i];
        if rect.z <= 0.0 { continue; }
        let center = rect.xy + rect.zw * 0.5;
        let radius = 2.5 * glass.viewport.z;
        let q = abs(p-center) - rect.zw*0.5 + vec2(radius-expand);
        let distance = length(max(q,vec2(0.0))) + min(max(q.x,q.y),0.0) - radius;
        if distance <= 0.0 { return true; }
    }
    return false;
}
fn blur(uv: vec2<f32>, axis: vec2<f32>) -> vec4<f32> {
    var color = vec4(0.0);
    var total = 0.0;
    // Separable Gaussian, 17 taps per pass, resolution-independent radius.
    for (var i = -8; i <= 8; i += 1) {
        let offset = f32(i);
        let weight = exp(-offset*offset/24.5);
        color += textureSampleLevel(scene,scene_sampler,uv + axis*offset*1.65*glass.viewport.z/glass.viewport.xy,0.0)*weight;
        total += weight;
    }
    return color/total;
}
@fragment
fn horizontal(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    if inside_panel(in.uv*glass.viewport.xy,0.0) {
        return blur(in.uv,vec2(1.0,0.0));
    }
    return textureSampleLevel(scene,scene_sampler,in.uv,0.0);
}
@fragment
fn vertical(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var color = textureSampleLevel(scene,scene_sampler,in.uv,0.0);
    let p = in.uv*glass.viewport.xy;
    if inside_panel(p,0.0) {
        color = blur(in.uv,vec2(0.0,1.0));
        let luminance = dot(color.rgb,vec3(0.2126,0.7152,0.0722));
        let grain = (fract(sin(dot(p,vec2(12.9898,78.233)))*43758.5453)-0.5)*0.012;
        color = vec4(mix(color.rgb,vec3(luminance),0.35)+vec3(grain),color.a);
    }
    // A restrained vignette holds the corners down while leaving the central scene visible.
    let edge = pow(length((in.uv-vec2(0.5))*vec2(1.2,1.0)),1.5);
    return vec4(color.rgb*(1.0-clamp(edge*0.44,0.0,0.40)),color.a);
}
