//
// The stb_image surface: what `stb_image.h` declares, as `stb_image/module.jai`
// presents it.
//
// Generated from the vendored header by `tools/cbind` — see
// `tools/cbind/README.md`. `module.jai` declares `libstb_image`, the archive
// the module builds out of `stb_image.c` the first time a program imports it.
//


// -------------------------------------------------------------- constants ---

STBI_VERSION :: 1;

// -------------------------------------------------------------- the types ---

stbi_uc :: u8;
stbi_us :: u16;
stbi_io_callbacks :: struct {
    read: #type (*void, *u8, s32) -> s32 #c_call;
    skip: #type (*void, s32) -> void #c_call;
    eof: #type (*void) -> s32 #c_call;
}


// ------------------------------------------------------------------ enums ---

STBI :: enum s32 {
    default :: 0;
    grey :: 1;
    grey_alpha :: 2;
    rgb :: 3;
    rgb_alpha :: 4;
}


// ------------------------------------------------------------- procedures ---

stbi_convert_iphone_png_to_rgb :: (flag_true_if_should_convert: s32) -> void #foreign libstb_image;
stbi_convert_iphone_png_to_rgb_thread :: (flag_true_if_should_convert: s32) -> void #foreign libstb_image;
stbi_failure_reason :: () -> *u8 #foreign libstb_image;
stbi_hdr_to_ldr_gamma :: (gamma: float32) -> void #foreign libstb_image;
stbi_hdr_to_ldr_scale :: (scale: float32) -> void #foreign libstb_image;
stbi_image_free :: (retval_from_stbi_load: *void) -> void #foreign libstb_image;
stbi_info :: (filename: *u8, x: *s32, y: *s32, comp: *s32) -> s32 #foreign libstb_image;
stbi_info_from_callbacks :: (clbk: *stbi_io_callbacks, user: *void, x: *s32, y: *s32, comp: *s32) -> s32 #foreign libstb_image;
stbi_info_from_memory :: (buffer: *stbi_uc, len: s32, x: *s32, y: *s32, comp: *s32) -> s32 #foreign libstb_image;
stbi_is_16_bit :: (filename: *u8) -> s32 #foreign libstb_image;
stbi_is_16_bit_from_callbacks :: (clbk: *stbi_io_callbacks, user: *void) -> s32 #foreign libstb_image;
stbi_is_16_bit_from_memory :: (buffer: *stbi_uc, len: s32) -> s32 #foreign libstb_image;
stbi_is_hdr :: (filename: *u8) -> s32 #foreign libstb_image;
stbi_is_hdr_from_callbacks :: (clbk: *stbi_io_callbacks, user: *void) -> s32 #foreign libstb_image;
stbi_is_hdr_from_memory :: (buffer: *stbi_uc, len: s32) -> s32 #foreign libstb_image;
stbi_ldr_to_hdr_gamma :: (gamma: float32) -> void #foreign libstb_image;
stbi_ldr_to_hdr_scale :: (scale: float32) -> void #foreign libstb_image;
stbi_load :: (filename: *u8, x: *s32, y: *s32, channels_in_file: *s32, desired_channels: s32) -> *stbi_uc #foreign libstb_image;
stbi_load_16 :: (filename: *u8, x: *s32, y: *s32, channels_in_file: *s32, desired_channels: s32) -> *stbi_us #foreign libstb_image;
stbi_load_16_from_callbacks :: (clbk: *stbi_io_callbacks, user: *void, x: *s32, y: *s32, channels_in_file: *s32, desired_channels: s32) -> *stbi_us #foreign libstb_image;
stbi_load_16_from_memory :: (buffer: *stbi_uc, len: s32, x: *s32, y: *s32, channels_in_file: *s32, desired_channels: s32) -> *stbi_us #foreign libstb_image;
stbi_load_from_callbacks :: (clbk: *stbi_io_callbacks, user: *void, x: *s32, y: *s32, channels_in_file: *s32, desired_channels: s32) -> *stbi_uc #foreign libstb_image;
stbi_load_from_memory :: (buffer: *stbi_uc, len: s32, x: *s32, y: *s32, channels_in_file: *s32, desired_channels: s32) -> *stbi_uc #foreign libstb_image;
stbi_load_gif_from_memory :: (buffer: *stbi_uc, len: s32, delays: **s32, x: *s32, y: *s32, z: *s32, comp: *s32, req_comp: s32) -> *stbi_uc #foreign libstb_image;
stbi_loadf :: (filename: *u8, x: *s32, y: *s32, channels_in_file: *s32, desired_channels: s32) -> *float32 #foreign libstb_image;
stbi_loadf_from_callbacks :: (clbk: *stbi_io_callbacks, user: *void, x: *s32, y: *s32, channels_in_file: *s32, desired_channels: s32) -> *float32 #foreign libstb_image;
stbi_loadf_from_memory :: (buffer: *stbi_uc, len: s32, x: *s32, y: *s32, channels_in_file: *s32, desired_channels: s32) -> *float32 #foreign libstb_image;
stbi_set_flip_vertically_on_load :: (flag_true_if_should_flip: s32) -> void #foreign libstb_image;
stbi_set_flip_vertically_on_load_thread :: (flag_true_if_should_flip: s32) -> void #foreign libstb_image;
stbi_set_unpremultiply_on_load :: (flag_true_if_should_unpremultiply: s32) -> void #foreign libstb_image;
stbi_set_unpremultiply_on_load_thread :: (flag_true_if_should_unpremultiply: s32) -> void #foreign libstb_image;
stbi_zlib_decode_buffer :: (obuffer: *u8, olen: s32, ibuffer: *u8, ilen: s32) -> s32 #foreign libstb_image;
stbi_zlib_decode_malloc :: (buffer: *u8, len: s32, outlen: *s32) -> *u8 #foreign libstb_image;
stbi_zlib_decode_malloc_guesssize :: (buffer: *u8, len: s32, initial_size: s32, outlen: *s32) -> *u8 #foreign libstb_image;
stbi_zlib_decode_malloc_guesssize_headerflag :: (buffer: *u8, len: s32, initial_size: s32, outlen: *s32, parse_header: s32) -> *u8 #foreign libstb_image;
stbi_zlib_decode_noheader_buffer :: (obuffer: *u8, olen: s32, ibuffer: *u8, ilen: s32) -> s32 #foreign libstb_image;
stbi_zlib_decode_noheader_malloc :: (buffer: *u8, len: s32, outlen: *s32) -> *u8 #foreign libstb_image;

// ------------------------------------------------------------- the C names ---

// The header's anonymous enum, under the names C code passes as
// `desired_channels`, which is an `int`.
STBI_default    :: cast(s32) STBI.default;
STBI_grey       :: cast(s32) STBI.grey;
STBI_grey_alpha :: cast(s32) STBI.grey_alpha;
STBI_rgb        :: cast(s32) STBI.rgb;
STBI_rgb_alpha  :: cast(s32) STBI.rgb_alpha;
