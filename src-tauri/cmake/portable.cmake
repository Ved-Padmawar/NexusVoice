# Release toolchain: turn off ggml's -march=native.
#
# Left on, the binary carries the *build* machine's instruction set (AVX-512 on
# some CI runners) and any CPU without those crashes on first use with
# 0xc000001d. whisper-rs-sys doesn't expose the option and cmake-rs forwards no
# arbitrary env vars, so it goes here — reached via CMAKE_TOOLCHAIN_FILE.
#
# Costs some CPU auto-vectorisation; inference runs on Vulkan/CUDA anyway.
set(GGML_NATIVE OFF CACHE BOOL "" FORCE)
