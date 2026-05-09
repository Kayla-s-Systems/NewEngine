# newengine-shader-compiler

Small runtime shader compiler adapter for NewEngine.

The crate intentionally does not link `shaderc-sys`. It invokes `glslc` from:

1. `NEWENGINE_GLSLC`
2. `%VULKAN_SDK%/Bin/glslc.exe` on Windows
3. `glslc` from `PATH`

This keeps the gameplay/editor build independent from native `shaderc-sys` CMake builds.
