Terrain & Stuff
========================================================
A graphics demo where I play around with - you guessed - it terrain and stuff!

* Spiritual successor to https://github.com/Wumpf/terrainwatersim
* Framework based on https://github.com/Wumpf/minifb_wgpu_web_and_desktop

TODO: more readme :)


Tech Stack
-----------------

* [wgpu](https://github.com/gfx-rs/wgpu) for rendering
* [wesl](https://github.com/wgsl-tooling-wg/wesl-rs) for shader (pre-)processing
* [minifb](https://github.com/emoon/minifb) for windowing and event handling
* [egui](https://github.com/emilk/egui) for UI
    * using a custom, minimalistic binding to minifb instead
      of [eframe](https://github.com/emilk/egui/tree/master/crates/eframe)

Sky rendering
-----------------

Based on [Sébastien Hillaire's "A Scalable and Production Ready
Sky and Atmosphere Rendering Technique"](https://sebh.github.io/publications/egsr2020.pdf) (Eurographics Symposium on
Rendering 2020).
See also Sébastien's [talk](https://www.youtube.com/watch?v=SW30QX1wxTY) about the topic at SIGGRAPH's 2020 Physically
Based Shading Course.

Various implementations exists:

* [official demo implementation](https://github.com/sebh/UnrealEngineSkyAtmosphere) by Sébastien himself.
* [Andrew Helmer's ShaderToy](https://www.shadertoy.com/view/slSXRW).
* [Lukas Herzbeger's rigorous WebGPU implementation](https://github.com/JolifantoBambla/webgpu-sky-atmosphere) with many
  different configuration options and compute shader variants

Display transform
-----------------

All calculations are done in Bt.709 linear luminance values (i.e. using photometric units if not specified otherwise).

As explained in https://seblagarde.wordpress.com/wp-content/uploads/2015/07/course_notes_moving_frostbite_to_pbr_v32.pdf
it has some advantages to keep all units radiometric for non-spectral rendering, and a fixed conversion factor can be
assumed.

The HDR Bt.709 stimulus is then mapped to LDR using [Tony McMapface](https://github.com/h3r2tic/tony-mc-mapface)
and converted to sRGB "gamma" space by applying the sRGB OETF.

TODO: Would be interesting to explore supporting Display P3 output as well!
Unfortunately, that would mean we need a different display transform then since there's no HDR output for Tony.


Lighting
-----------------
TODO: Shadows

Indirect sky light
==================

Compute a 3 band (order 2) spherical harmonic of the sky's luminance, ignoring the sun.
Sun is obviously _a lot_ brighter than everything else,
so not only would it ruin the SH coefficients for everything else but we can also do more interesting
BRDFs by treating it as a simple directional light.

Then, for the actual indirect lighting, compute illuminance by integrating cosine lobe
TODO: Why stop at lambert/illuminance here? Might as well sample luminance in a direction to approximate some
specularity as well!

This is done by a compute shader sampling the sky's luminance at a fixed height using same method we use per screen
pixel when raymarching the atmosphere.
Steps:

* each invocation computes one sample on the sphere and keeps this sample in register
* for SH coefficient:
    * compute the sample's coefficient value
    * write it to shared memory
    * perform a reduction over the shared memory to get the average coefficient value
    * write final coefficient value to the output buffer

Once we have the SH coefficients, we can use it to use it for "ambient" lighting and combine it with direct sun light.
