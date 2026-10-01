// The retail world surface shader. Direct3D 8 of 2002 does all its maths on the stored (gamma) values and writes them
// straight to the screen, so this shader works on gamma values too and hands the sRGB framebuffer their linear
// equivalents, which the hardware encodes back to exactly what Direct3D would have written.
//   Gouraud (shader 1): texture * vertex colour           CRenderShader_Gouraud_Texture
//   Lightmap (shaders 2 + 4): texture * lightmap          pass 1 lightmap, pass 2 blends DESTCOLOR * ZERO with fog colour white
//   Fullbright (DTX flag, +2): drawn over the lit surface with the texture alpha as the mask (SRCALPHA / INVSRCALPHA), so the
//   glass of a lit window is fullbright while the plaster around it (alpha 0) keeps its lighting   CRenderShader_*_Fullbright
// Table fog is linear in the view depth; the lightmap surface fogs each pass separately (lightmap towards the fog colour,
// texture towards white), which the product below reproduces.
#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::{view, globals}}

struct RetailWorld {
    // x: alpha factor, y: alpha cutoff (0 = none), z: shading (bit 0 lightmap, bit 1 fullbright mask), w: additive (fog colour black)
    surface: vec4<f32>,
    // rgb: fog colour (gamma), w: fog enabled
    fog_color: vec4<f32>,
    // x: fog start, y: fog end, z: far clip (view depth, world units), w: 1 = opaque surface (the texture alpha never reaches the target)
    fog_range: vec4<f32>,
    // xy: texture pan per second (UVPan texture effect group)
    pan: vec4<f32>,
    // x: texture effect (0 none, 1 EnvMap, 2 EnvMapAlpha, 3 DetailTex), y: detail uv scale (0.2 * DTX scale)
    effect: vec4<f32>,
    // rgb: current light group colour, w: 1 = the material carries the light group layer
    group: vec4<f32>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> surface: RetailWorld;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var base_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var base_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var lightmap_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var lightmap_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var effect_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var effect_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var group_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var group_sampler: sampler;

fn gamma_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(base_texture, base_sampler, in.uv + surface.pan.xy * globals.time);
    // DTX texture commands (Lithtech.exe 0x526b80 / 0x52cbe0 / 0x525f90), all on the gamma values in the world's own (LithTech) numbers:
    // EnvMap adds the second texture signed (ADDSIGNED), EnvMapAlpha adds it weighted by the base alpha (MODULATEALPHA_ADDCOLOR), DetailTex adds a
    // tiled detail texture signed. The environment lookup is the fixed function camera space reflection vector taken back to world space
    // (matrix 0x5ab34c = camera rotation): u = 0.5 - 0.5 r.x, v = 0.5 - 0.5 r.y (EnvScale 1), sampled per pixel instead of per vertex.
    var base_rgb = texel.rgb;
    let effect_kind = u32(surface.effect.x);
    if effect_kind == 1u || effect_kind == 2u {
        let eye_ray = normalize(in.world_position.xyz - view.world_position);
        let normal = normalize(in.world_normal);
        let reflection = eye_ray - 2.0 * dot(eye_ray, normal) * normal;
        let environment = textureSample(effect_texture, effect_sampler, vec2<f32>(0.5 - 0.5 * reflection.x, 0.5 - 0.5 * reflection.y)).rgb;
        base_rgb = select(clamp(base_rgb + environment - vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0)), clamp(base_rgb + texel.a * environment, vec3<f32>(0.0), vec3<f32>(1.0)), effect_kind == 2u);
    } else if effect_kind == 3u {
        let detail = textureSample(effect_texture, effect_sampler, in.uv * surface.effect.y).rgb;
        base_rgb = clamp(base_rgb + detail - vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    var light = textureSampleLevel(lightmap_texture, lightmap_sampler, in.uv_b, 0.0).rgb;
    // Light group (SetLightGroupColor): the lightmap texel gets intensity * colour added, saturating like the bytes of the D3D texture.
    var group_add = vec3<f32>(0.0);
    if surface.group.w > 0.5 {
        group_add = textureSampleLevel(group_texture, group_sampler, in.uv_b, 0.0).r * surface.group.rgb;
        light = clamp(light + group_add, vec3<f32>(0.0), vec3<f32>(1.0));
    }
    let depth = -(view.view_from_world * vec4<f32>(in.world_position.xyz, 1.0)).z;
    if depth > surface.fog_range.z {
        discard;
    }
    // Additive brushes blend ONE/ONE with the object colour (alpha, alpha, alpha) as the vertex colour (Lithtech.exe 0x53d5b0): the texture alpha is not read.
    let alpha = select(texel.a * surface.surface.x, surface.surface.x, surface.surface.w > 0.5);
    if surface.surface.y > 0.0 && alpha < surface.surface.y {
        discard;
    }
    let fog_span = max(surface.fog_range.y - surface.fog_range.x, 0.0001);
    let visible = select(1.0, clamp((surface.fog_range.y - depth) / fog_span, 0.0, 1.0), surface.fog_color.w > 0.5);
    var fog = surface.fog_color.rgb;
    if surface.surface.w > 0.5 {
        fog = vec3<f32>(0.0);
    }
    var color: vec3<f32>;
    let shading = u32(surface.surface.z);
    if (shading & 1u) == 0u {
#ifdef VERTEX_COLORS
        var shade = in.color.rgb;
        if surface.group.w > 0.5 {
            shade = clamp(shade + in.uv_b.x * surface.group.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
        }
#else
        let shade = vec3<f32>(1.0);
#endif
        color = mix(fog, base_rgb * shade, visible);
    } else {
        color = mix(fog, light, visible) * mix(vec3<f32>(1.0), base_rgb, visible);
    }
    if (shading & 2u) != 0u {
        color = mix(color, mix(fog, base_rgb, visible), texel.a);
    }
    // Texture alpha carries envmap masks and other data: only the blended and additive surfaces may write it (the mirrored display blends by it).
    return vec4<f32>(gamma_to_linear(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0))), select(alpha, 1.0, surface.fog_range.w > 0.5));
}
