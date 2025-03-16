enable dual_source_blending;

// Compute per pixel atmosphere luminance & transmittance.

#import "constants.wgsl"::{ERROR_RGBA}
#import "camera.wgsl"::{view_space_position_from_depth_buffer, camera_ray_from_screenuv}
#import "global_bindings.wgsl"::{frame_uniforms}
#import "intersections.wgsl"::{Ray}
#import "sh.wgsl"::{evaluate_sh2} // For debugging only.

#import "atmosphere/params.wgsl"::{
    atmosphere_params,
    AtmosphereDebugDrawMode_None,
    AtmosphereDebugDrawMode_Sh,
    AtmosphereDebugDrawMode_NoScatteringOverlay,
}
#import "atmosphere/raymarch.wgsl"::{raymarch_scattering}
#import "atmosphere/sky_and_sun_lighting.wgsl"::{SkyAndSunLightingParams}

@group(2) @binding(0) var transmittance_lut: texture_2d<f32>;
@group(2) @binding(1) var<uniform> sky_and_sun_lighting_params: SkyAndSunLightingParams;
@group(3) @binding(0) var screen_depth: texture_2d<f32>;

const NumScatteringSteps: f32 = 64.0;

struct FragmentResult {
    @location(0) @blend_src(0) scattering : vec4f,
    @location(0) @blend_src(1) transmittance : vec4f,
}

fn sun_disk_luminance(camera_ray: Ray, dir_to_sun: vec3f) -> vec3f {
    let sun = dot(camera_ray.direction, dir_to_sun) - cos(atmosphere_params.sun_disk_diameteter_rad);
    // Since the sun is so bright, this isn't giving us enough antialiasing yet.
    //let antialiased_sun = saturate(sun / (fwidth(sun) * 100.0));
    // Fudging this with a looks good enough.
    let antialiased_sun = saturate(sun / (fwidth(sun) * 100.0));
    return atmosphere_params.sun_disk_illuminance_factor * sky_and_sun_lighting_params.sun_illuminance * antialiased_sun;
}

@fragment
fn fs_main(@location(0) texcoord: vec2f, @builtin(position) position: vec4f) -> FragmentResult {
    let camera_ray = camera_ray_from_screenuv(texcoord);

    // Determine the length of the camera ray when we hit geometry - this length is infinity wherever we hit the sky.
    let depth_buffer_depth = textureLoad(screen_depth, vec2i(position.xy), 0).r;
    let view_space_position = view_space_position_from_depth_buffer(depth_buffer_depth, texcoord);
    let geometry_distance_on_camera_ray = length(view_space_position);

    // For our camera we generally assume a flat planet.
    // But as we march through the atmosphere, we have to take into account that the atmosphere is curved.
    let planet_relative_position_km = vec3(0.0, max(0.0, camera_ray.origin.y * 0.001) + atmosphere_params.ground_radius_km, 0.0);

    var result = raymarch_scattering(
        transmittance_lut,
        camera_ray.direction,
        planet_relative_position_km,
        frame_uniforms.dir_to_sun,
        geometry_distance_on_camera_ray
    );
    result.scattering += sun_disk_luminance(camera_ray, frame_uniforms.dir_to_sun);

    let fragment_result = FragmentResult(vec4f(result.scattering, 1.0), vec4f(result.transmittance, 1.0));

    // DEBUG:
    switch (atmosphere_params.debug_draw_mode) {
        // case AtmosphereDebugDrawMode_None: {
        //     break;
        // }

        case AtmosphereDebugDrawMode_Sh: {
            if depth_buffer_depth == 0.0 {
                let sh_luminance = evaluate_sh2(camera_ray.direction, sky_and_sun_lighting_params.sky_luminance_sh_coefficients);
                return FragmentResult(vec4f(sh_luminance, 1.0), vec4f(0.0));
            }
            break;
        }

        case AtmosphereDebugDrawMode_NoScatteringOverlay: {
            if depth_buffer_depth != 0.0 {
                var debug_result = fragment_result;
                debug_result.transmittance = vec4f(1.0);
                debug_result.scattering *= vec4f(0.0);
                return debug_result;
            }
            break;
        }

        default: {
            break;
        }
    }

    return fragment_result;

    // Debug stuff:
    //return FragmentResult(vec4f(fract(max_marching_distance_km * 0.1)), vec4f(0.0));

    //let world_space_position = (view_space_position * frame_uniforms.view_from_world).xyz + frame_uniforms.camera_position;
    //return FragmentResult(vec4f(fract(abs(world_space_position) * 0.0001), 1.0), vec4f(0.0));

    //let trasmittance_lut = textureSample(transmittance_lut, trilinear_sampler_clamp, texcoord);
    //return FragmentResult(trasmittance_lut, vec4f(0.0));
}
