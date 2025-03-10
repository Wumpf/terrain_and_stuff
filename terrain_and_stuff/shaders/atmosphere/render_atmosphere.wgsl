enable dual_source_blending;

// Compute per pixel atmosphere luminance & transmittance.

#import "constants.wgsl"::{ERROR_RGBA}
#import "camera.wgsl"::{view_space_position_from_depth_buffer, camera_ray_from_screenuv}
#import "global_bindings.wgsl"::{frame_uniforms}
#import "intersections.wgsl"::{Ray}

#import "atmosphere/constants.wgsl"::{
    ground_radius_km,
    sun_diameteter_rad,
    sun_unscattered_luminance,
}
#import "atmosphere/raymarch.wgsl"::{raymarch_scattering}


@group(1) @binding(0) var transmittance_lut: texture_2d<f32>;
@group(1) @binding(1) var screen_depth: texture_2d<f32>;

const NumScatteringSteps: f32 = 64.0;

struct FragmentResult {
    @location(0) @blend_src(0) scattering : vec4f,
    @location(0) @blend_src(1) transmittance : vec4f,
}

fn sun_disk_luminance(camera_ray: Ray, dir_to_sun: vec3f, transmittance: vec3f) -> vec3f {
    let sun = dot(camera_ray.direction, dir_to_sun) - cos(sun_diameteter_rad);
    // Since the sun is so bright, this isn't giving us enough antialiasing yet.
    //let antialiased_sun = saturate(sun / (fwidth(sun) * 100.0));
    // Fudging this with a looks good enough.
    let antialiased_sun = saturate(sun / (fwidth(sun) * 1000.0));
    return sun_unscattered_luminance * transmittance * antialiased_sun;
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
    let planet_relative_position_km = vec3(0.0, max(0.0, camera_ray.origin.y * 0.001) + ground_radius_km, 0.0);

    var result = raymarch_scattering(
        transmittance_lut,
        camera_ray.direction,
        planet_relative_position_km,
        frame_uniforms.dir_to_sun,
        geometry_distance_on_camera_ray
    );
    result.scattering += sun_disk_luminance(camera_ray, frame_uniforms.dir_to_sun, result.transmittance);

    let fragment_result = FragmentResult(vec4f(result.scattering, 1.0), vec4f(result.transmittance, 1.0));

    // DEBUG:
    // Disable sky wherever there's something in the depth buffer for debugging the effect of the atmosphere on the landscape.
    if false && depth_buffer_depth != 0.0 {
        var debug_result = fragment_result;
        debug_result.transmittance = vec4f(1.0);
        debug_result.scattering *= vec4f(0.0);
        return debug_result;
    }

    return fragment_result;

    // Debug stuff:
    //return ScatteringResult(vec4f(fract(max_marching_distance_km * 0.1)), vec4f(0.0));

    //let world_space_position = (view_space_position * frame_uniforms.view_from_world).xyz + frame_uniforms.camera_position;
    //return ScatteringResult(vec4f(fract(abs(world_space_position) * 0.0001), 1.0), vec4f(0.0));

    //let trasmittance_lut = textureSample(transmittance_lut, trilinear_sampler_clamp, texcoord);
    //return ScatteringResult(trasmittance_lut, vec4f(0.0));
}
