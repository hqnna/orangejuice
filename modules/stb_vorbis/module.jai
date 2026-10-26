//
// stb_vorbis: decodes Ogg Vorbis audio into 16-bit or float samples, from
// memory, a whole file at once or a frame at a time.
//
// A binding over Sean Barrett's public-domain `stb_vorbis.c`, vendored here
// unmodified as `stb_vorbis.h`: the file is its own header, declaring only
// the interface when `STB_VORBIS_HEADER_ONLY` is defined, and `stb_vorbis.c`
// includes the whole of it to compile the implementation. The first program
// that imports this module builds that into `.build/<cpu>-<os>/stb_vorbis.a`
// with the system's C compiler — see `../stb_image/c_library.jai` for how.
// The procedures and types are generated from the source by `tools/cbind`.
//
// The archive is linked into the executable; a `#run` cannot call into it,
// since the JIT loads shared libraries and this is not one.
//

#load "../stb_image/c_library.jai";

#run build_c_library(#filepath, "stb_vorbis");

#scope_module

#if OS == .LINUX && CPU == .X64 {
    libstb_vorbis :: #library ".build/x86_64-linux/stb_vorbis";
} else #if OS == .LINUX && CPU == .ARM64 {
    libstb_vorbis :: #library ".build/aarch64-linux/stb_vorbis";
} else #if OS == .MACOS && CPU == .ARM64 {
    libstb_vorbis :: #library ".build/aarch64-darwin/stb_vorbis";
}

#scope_export

#load "generated.jai";
