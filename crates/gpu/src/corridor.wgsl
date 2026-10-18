// A fixed viewpoint looks into a five-sided room. Only the rain sheets animate;
// surface intersections and perspective remain stationary.
struct Params { screen: vec4<f32>, atlas: vec4<f32>, view: vec4<f32> }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var sheets: texture_2d_array<u32>;
@group(0) @binding(2) var atlas: texture_2d<f32>;
@group(0) @binding(3) var atlas_sampler: sampler;

@vertex fn vertex_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let positions = array(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(positions[i], 0.0, 1.0);
}
fn mask(uv: vec2<f32>) -> f32 { return textureSampleLevel(atlas, atlas_sampler, uv, 0.0).a; }

@fragment fn fragment_main(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let p = (pixel.xy / params.screen.xy) * 2.0 - 1.0;
    let ray = normalize(vec3<f32>(p.x * params.screen.x / params.screen.y * 0.82, -p.y * 0.82, 1.0));
    let side = select(-4.0, 4.0, ray.x >= 0.0);
    let horizontal = select(-2.8, 2.8, ray.y >= 0.0);
    let tx = 4.0 / max(abs(ray.x), 0.00001);
    let ty = 2.8 / max(abs(ray.y), 0.00001);
    let tz = 12.0 / ray.z;
    let back = tz <= min(tx, ty);
    let wall = back || tx < ty;
    let distance = min(tz, min(tx, ty));
    let hit = ray * distance;
    let scale = params.view.x;
    var surface = vec2<f32>(hit.z * 12.0 / scale, (2.8 - hit.y) * 7.5 / scale);
    var face = select(0, 1, side > 0.0);
    if back {
        surface = vec2<f32>((hit.x + 4.0) * 12.0 / scale, (2.8 - hit.y) * 7.5 / scale);
        face = 4;
    } else if !wall {
        surface = vec2<f32>((hit.x + 4.0) * 12.0 / scale, hit.z * 7.5 / scale);
        face = select(2, 3, horizontal > 0.0);
    }
    // Orient each inward-facing surface so letters stay readable.
    if face == 1 { surface.x = -surface.x; }
    if face == 2 { surface.y = -surface.y; }
    let cell = vec2<i32>(floor(surface));
    let sheet_cell = vec2<i32>(((cell.x % 192) + 192) % 192, ((cell.y % 64) + 64) % 64);
    let data = textureLoad(sheets, sheet_cell, face, 0);
    let index = data.x + data.y * 256u;
    let tile = vec2<f32>(f32(index % 16u), f32(index / 16u));
    let local = vec2<f32>(fract(surface.x), 1.0 - fract(surface.y));
    let uv = (tile * params.atlas.zw + vec2<f32>(1.0) + local * (params.atlas.zw - vec2<f32>(2.0))) / params.atlas.xy;
    let texel = vec2<f32>(1.0) / params.atlas.xy;
    let ink = mask(uv);
    let halo = (mask(uv + vec2<f32>(texel.x, 0.0)) + mask(uv - vec2<f32>(texel.x, 0.0)) +
        mask(uv + vec2<f32>(0.0, texel.y)) + mask(uv - vec2<f32>(0.0, texel.y))) * 0.25;
    let bright = f32(data.z) / 255.0;
    let head = data.w > 0u;
    let green = select(vec3<f32>(0.015, 0.9, 0.12), vec3<f32>(0.72, 1.0, 0.82), head);
    let fog = exp(-max(distance - 4.0, 0.0) * 0.055);
    // Suppress subpixel glyphs at the far end instead of producing shimmering dots.
    let detail = 1.0 - smoothstep(35.0, 85.0, distance);
    let light = select(0.85, 1.0, wall);
    let color = green * bright * (ink * 1.15 + halo * select(0.25, 0.65, head)) * fog * detail * light;
    let vignette = 1.0 - 0.15 * dot(p * 0.5, p * 0.5);
    return vec4<f32>(color * vignette, 1.0);
}
