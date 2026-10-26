//
// stb_image: decodes PNG, JPEG, BMP, TGA, GIF, PSD, HDR, PIC and PNM images into
// pixels.
//
// A binding over Sean Barrett's public-domain `stb_image.h`, which is vendored
// here unmodified. `stb_image.c` compiles its implementation, and the first
// program that imports this module builds it into
// `.build/<cpu>-<os>/stb_image.a` with the system's C compiler — see
// `c_library.jai` for how, and `docs/spec.md` §10.1 for why the
// distribution ships C source rather than a library. The procedures and
// types are generated from the header by `tools/cbind`.
//
// The archive is linked into the executable; a `#run` cannot call into it,
// since the JIT loads shared libraries and this is not one.
//

#load "c_library.jai";

#run build_c_library(#filepath, "stb_image");

#scope_module

#if OS == .LINUX && CPU == .X64 {
    libstb_image :: #library ".build/x86_64-linux/stb_image";
} else #if OS == .LINUX && CPU == .ARM64 {
    libstb_image :: #library ".build/aarch64-linux/stb_image";
} else #if OS == .MACOS && CPU == .ARM64 {
    libstb_image :: #library ".build/aarch64-darwin/stb_image";
}

#scope_export

#load "generated.jai";
