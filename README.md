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