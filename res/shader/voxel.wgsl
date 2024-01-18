// Vertex shader

struct Camera {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
    world_pos_and_chunk_size: vec4<i32>,
}

struct Settings {
    sunlight_intensity: u32,
    base_light_value: f32,
    light_power_factor: f32,
    tile_size: f32,
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@group(1) @binding(0)
var<uniform> camera: Camera;

@group(2) @binding(0)
var<uniform> settings: Settings;

@group(3) @binding(0)
var<uniform> world_pos: vec4<i32>;

struct VertexInput {
    @location(0) pos: vec4<u32>,
    @location(1) normal: vec4<i32>,
    @location(2) color: vec4<u32>,
    @location(3) texture_atlas_pos: vec2<f32>,
    @location(4) brightness_transparency: vec2<u32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) pos: vec3<f32>,
    @location(1) texture_atlas_pos: vec2<f32>,
    @location(2) color: vec3<f32>,
    @location(3) axis1: vec3<f32>,
    @location(4) axis2: vec3<f32>,
    @location(5) brightness: f32,
    @location(6) transparency: f32,
}

@vertex
fn vs_main(
    block: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.texture_atlas_pos = block.texture_atlas_pos;

    {
        let chunk_size = camera.world_pos_and_chunk_size[3];
        let chunk_world_pos = vec3(world_pos.x, world_pos.y, world_pos.z);
        let camera_world_pos = vec3(camera.world_pos_and_chunk_size.x, camera.world_pos_and_chunk_size.y, camera.world_pos_and_chunk_size.z);

        let chunk_offset = vec3((chunk_world_pos.x - camera_world_pos.x) * chunk_size, (chunk_world_pos.y - camera_world_pos.y) * chunk_size, (chunk_world_pos.z - camera_world_pos.z) * chunk_size);

        let block_pos = vec3(i32(block.pos.x), i32(block.pos.y), i32(block.pos.z));

        let pos_i32 = block_pos + chunk_offset;
        let pos_f32 = vec3(f32(pos_i32.x), f32(pos_i32.y), f32(pos_i32.z));

        out.pos = pos_f32;
        out.clip_position = camera.view_proj * vec4<f32>(pos_f32, 1.0);
    }

    {
        let normal = vec3(f32(block.normal.x), f32(block.normal.y), f32(block.normal.z));

        var e1: vec3<f32> = cross(normal, vec3(0.0, 1.0, 0.0));
        if e1.x == 0.0 && e1.y == 0.0 && e1.z == 0.0 {
            e1 = cross(normal, vec3(0.0, 0.0, 1.0));
        }
        let e2 = cross(normal, e1);
    
        out.axis1 = e1;
        out.axis2 = e2;
    }

    {
        var color_raw = vec3(block.color.x, block.color.y, block.color.z);

        if block.color.w > 0u {
            let relative_sunlight_strength = i32(block.color.w) - (15 - i32(settings.sunlight_intensity));
            
            if relative_sunlight_strength > 0 {
                let relative_sunlight_strength = u32(relative_sunlight_strength);

                if color_raw.x < relative_sunlight_strength {
                    color_raw.x = relative_sunlight_strength;
                }
                if color_raw.y < relative_sunlight_strength {
                    color_raw.y = relative_sunlight_strength;
                }
                if color_raw.z < relative_sunlight_strength {
                    color_raw.z = relative_sunlight_strength;
                }
            }
        }

        let color = vec3(f32(color_raw.x) / 16.0, f32(color_raw.y) / 16.0, f32(color_raw.z) / 16.0);
        out.color = vec3(settings.base_light_value + pow(color.x, settings.light_power_factor), settings.base_light_value + pow(color.y, settings.light_power_factor), settings.base_light_value + pow(color.z, settings.light_power_factor));
    }

    {
        var brightness: f32;
        
        switch block.brightness_transparency.x {
            default {
                brightness = 1.0;
            }
            case 1u {
                brightness = 0.8;
            }
            case 2u {
                brightness = 0.6;
            }
            case 3u {
                brightness = 0.4;
            }
        }

        out.brightness = brightness;
    }


    {
        var transparency: f32;
    
        switch block.brightness_transparency.y {
            default {
                transparency = 1.0;
            }
            case 1u {
                transparency = 0.6;
            }
        }

        out.transparency = transparency;
    }

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tile_uv = vec2(dot(in.axis1, in.pos), dot(in.axis2, in.pos));
    let tex_coord = in.texture_atlas_pos + fract(tile_uv) * settings.tile_size;

    let color = in.brightness * textureSample(t_diffuse, s_diffuse, tex_coord);
    return vec4<f32>(in.color.x * color.x, in.color.y * color.y, in.color.z * color.z, in.transparency);
}
