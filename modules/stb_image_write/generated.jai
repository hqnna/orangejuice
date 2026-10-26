//
// The stb_image_write surface: what `stb_image_write.h` declares, as `stb_image_write/module.jai`
// presents it.
//
// Generated from the vendored header by `tools/cbind` — see
// `tools/cbind/README.md`. `module.jai` declares `libstb_image_write`, the archive
// the module builds out of `stb_image_write.c` the first time a program imports it.
//


// -------------------------------------------------------------- constants ---


// -------------------------------------------------------------- the types ---


// ------------------------------------------------------------------ enums ---


// ------------------------------------------------------------- procedures ---

stbi_flip_vertically_on_write :: (flip_boolean: s32) -> void #foreign libstb_image_write;
stbi_write_bmp :: (filename: *u8, w: s32, h: s32, comp: s32, data: *void) -> s32 #foreign libstb_image_write;
stbi_write_hdr :: (filename: *u8, w: s32, h: s32, comp: s32, data: *float32) -> s32 #foreign libstb_image_write;
stbi_write_jpg :: (filename: *u8, x: s32, y: s32, comp: s32, data: *void, quality: s32) -> s32 #foreign libstb_image_write;
stbi_write_png :: (filename: *u8, w: s32, h: s32, comp: s32, data: *void, stride_in_bytes: s32) -> s32 #foreign libstb_image_write;
stbi_write_tga :: (filename: *u8, w: s32, h: s32, comp: s32, data: *void) -> s32 #foreign libstb_image_write;

// ---------------------------------------------------------- the overrides ---

stbi_write_func :: #type (_context: *void, data: *void, size: s32) #c_call;

stbi_write_bmp_to_func :: (func: stbi_write_func, _context: *void, w: s32, h: s32, comp: s32, data: *void) -> s32 #foreign libstb_image_write;
stbi_write_hdr_to_func :: (func: stbi_write_func, _context: *void, w: s32, h: s32, comp: s32, data: *float32) -> s32 #foreign libstb_image_write;
stbi_write_jpg_to_func :: (func: stbi_write_func, _context: *void, x: s32, y: s32, comp: s32, data: *void, quality: s32) -> s32 #foreign libstb_image_write;
stbi_write_png_to_func :: (func: stbi_write_func, _context: *void, w: s32, h: s32, comp: s32, data: *void, stride_in_bytes: s32) -> s32 #foreign libstb_image_write;
stbi_write_tga_to_func :: (func: stbi_write_func, _context: *void, w: s32, h: s32, comp: s32, data: *void) -> s32 #foreign libstb_image_write;

// ---------------------------------------------------------------- settings ---

// The header's tuning knobs are globals of the library rather than
// parameters: set them before writing.
stbi_write_png_compression_level : s32 #elsewhere libstb_image_write;
stbi_write_force_png_filter      : s32 #elsewhere libstb_image_write;
stbi_write_tga_with_rle          : s32 #elsewhere libstb_image_write;
