Terrain & Stuff
========================================================
A graphics demo where I play around with - you guessed - it terrain and stuff!

* Spiritual successor to https://github.com/Wumpf/terrainwatersim
* Framework based on https://github.com/Wumpf/minifb_wgpu_web_and_desktop

TODO: more readme :)

Sky rendering
-----------------

Based on [Sébastien Hillaire's "A Scalable and Production Ready
Sky and Atmosphere Rendering Technique"](https://sebh.github.io/publications/egsr2020.pdf) (Eurographics Symposium on Rendering 2020).
See also Sébastien's [talk](https://www.youtube.com/watch?v=SW30QX1wxTY) about the topic at SIGGRAPH's 2020 Physically Based Shading Course.

Various implementations exists:
* [official demo implementation](https://github.com/sebh/UnrealEngineSkyAtmosphere) by Sébastien himself.
* [Andrew Helmer's ShaderToy](https://www.shadertoy.com/view/slSXRW).
* [Lukas Herzbeger's rigorous WebGPU implementation](https://github.com/JolifantoBambla/webgpu-sky-atmosphere) with many different configuration options and compute shader variants

TODO: haven't implemented the whole thing yet (multiple scattering is missing for instance), also there seems to be some issues.


Display transform
-----------------

All calculations are done in Bt.709 linear luminance values (i.e. using photometric units if not specified otherwise).

As explained in https://seblagarde.wordpress.com/wp-content/uploads/2015/07/course_notes_moving_frostbite_to_pbr_v32.pdf
it has some advantages to keep all units radiometric for non-spectral rendering and a fixed conversion factor can be assumed.

The HDR Bt.709 stimulus is then mapped to LDR using [Tony McMapface](https://github.com/h3r2tic/tony-mc-mapface)
and converted to sRGB "gamma" space by applying the sRGB OETF.
TODO: Would be interesting to explore supporting Display P3 output as well!
