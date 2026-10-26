
// ---------------------------------------------------------------- settings ---

// The header's tuning knobs are globals of the library rather than
// parameters: set them before writing.
stbi_write_png_compression_level : s32 #elsewhere libstb_image_write;
stbi_write_force_png_filter      : s32 #elsewhere libstb_image_write;
stbi_write_tga_with_rle          : s32 #elsewhere libstb_image_write;
