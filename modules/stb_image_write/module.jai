//
// stb_image_write: encodes pixels as PNG, BMP, TGA, JPEG or HDR, to a file or through
// a callback.
//
// A binding over Sean Barrett's public-domain `stb_image_write.h`, which is vendored
// here unmodified. `stb_image_write.c` compiles its implementation, and the first
// program that imports this module builds it into
// `.build/<cpu>-<os>/stb_image_write.a` with the system's C compiler — see
// `../stb_image/c_library.jai` for how, and `docs/spec.md` §10.1 for why the
// distribution ships C source rather than a library. The procedures and
// types are generated from the header by `tools/cbind`.
//
// The archive is linked into the executable; a `#run` cannot call into it,
// since the JIT loads shared libraries and this is not one.
//

#load "../stb_image/c_library.jai";

#run build_c_library(#filepath, "stb_image_write");

#scope_module

#if OS == .LINUX && CPU == .X64 {
    libstb_image_write :: #library ".build/x86_64-linux/stb_image_write";
} else #if OS == .LINUX && CPU == .ARM64 {
    libstb_image_write :: #library ".build/aarch64-linux/stb_image_write";
} else #if OS == .MACOS && CPU == .ARM64 {
    libstb_image_write :: #library ".build/aarch64-darwin/stb_image_write";
}

#scope_export

#load "generated.jai";
