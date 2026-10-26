//
// The stb_vorbis surface: what `stb_vorbis.c` declares when it is included as
// a header, as `stb_vorbis/module.jai` presents it.
//
// Generated from the vendored source by `tools/cbind` — see
// `tools/cbind/README.md`. `module.jai` declares `libstb_vorbis`, the archive
// the module builds out of `stb_vorbis.c` the first time a program imports it.
//


// -------------------------------------------------------------- constants ---


// -------------------------------------------------------------- the types ---

stb_vorbis_alloc :: struct {
    alloc_buffer: *u8;
    alloc_buffer_length_in_bytes: s32;
}

stb_vorbis_info :: struct {
    sample_rate: u32;
    channels: s32;
    setup_memory_required: u32;
    setup_temp_memory_required: u32;
    temp_memory_required: u32;
    max_frame_size: s32;
}

stb_vorbis_comment :: struct {
    vendor: *u8;
    comment_list_length: s32;
    comment_list: **u8;
}

// Opaque: the library owns the contents.
stb_vorbis :: struct {}


// ------------------------------------------------------------------ enums ---

STBVorbisError :: enum s32 {
    VORBIS__no_error;
    VORBIS_need_more_data :: 1;
    VORBIS_invalid_api_mixing;
    VORBIS_outofmem;
    VORBIS_feature_not_supported;
    VORBIS_too_many_channels;
    VORBIS_file_open_failure;
    VORBIS_seek_without_length;
    VORBIS_unexpected_eof :: 10;
    VORBIS_seek_invalid;
    VORBIS_invalid_setup :: 20;
    VORBIS_invalid_stream;
    VORBIS_missing_capture_pattern :: 30;
    VORBIS_invalid_stream_structure_version;
    VORBIS_continued_packet_flag_invalid;
    VORBIS_incorrect_stream_serial_number;
    VORBIS_invalid_first_page;
    VORBIS_bad_packet_type;
    VORBIS_cant_find_last_page;
    VORBIS_seek_failed;
    VORBIS_ogg_skeleton_not_supported;
}


// ------------------------------------------------------------- procedures ---

stb_vorbis_close :: (f: *stb_vorbis) -> void #foreign libstb_vorbis;
stb_vorbis_decode_frame_pushdata :: (f: *stb_vorbis, datablock: *u8, datablock_length_in_bytes: s32, channels: *s32, output: ***float32, samples: *s32) -> s32 #foreign libstb_vorbis;
stb_vorbis_decode_memory :: (mem: *u8, len: s32, channels: *s32, sample_rate: *s32, output: **s16) -> s32 #foreign libstb_vorbis;
stb_vorbis_flush_pushdata :: (f: *stb_vorbis) -> void #foreign libstb_vorbis;
stb_vorbis_get_comment :: (f: *stb_vorbis) -> stb_vorbis_comment #foreign libstb_vorbis;
stb_vorbis_get_error :: (f: *stb_vorbis) -> s32 #foreign libstb_vorbis;
stb_vorbis_get_file_offset :: (f: *stb_vorbis) -> u32 #foreign libstb_vorbis;
stb_vorbis_get_frame_float :: (f: *stb_vorbis, channels: *s32, output: ***float32) -> s32 #foreign libstb_vorbis;
stb_vorbis_get_frame_short :: (f: *stb_vorbis, num_c: s32, buffer: **s16, num_samples: s32) -> s32 #foreign libstb_vorbis;
stb_vorbis_get_frame_short_interleaved :: (f: *stb_vorbis, num_c: s32, buffer: *s16, num_shorts: s32) -> s32 #foreign libstb_vorbis;
stb_vorbis_get_info :: (f: *stb_vorbis) -> stb_vorbis_info #foreign libstb_vorbis;
stb_vorbis_get_sample_offset :: (f: *stb_vorbis) -> s32 #foreign libstb_vorbis;
stb_vorbis_get_samples_float :: (f: *stb_vorbis, channels: s32, buffer: **float32, num_samples: s32) -> s32 #foreign libstb_vorbis;
stb_vorbis_get_samples_float_interleaved :: (f: *stb_vorbis, channels: s32, buffer: *float32, num_floats: s32) -> s32 #foreign libstb_vorbis;
stb_vorbis_get_samples_short :: (f: *stb_vorbis, channels: s32, buffer: **s16, num_samples: s32) -> s32 #foreign libstb_vorbis;
stb_vorbis_get_samples_short_interleaved :: (f: *stb_vorbis, channels: s32, buffer: *s16, num_shorts: s32) -> s32 #foreign libstb_vorbis;
stb_vorbis_open_memory :: (data: *u8, len: s32, error: *s32, alloc_buffer: *stb_vorbis_alloc) -> *stb_vorbis #foreign libstb_vorbis;
stb_vorbis_open_pushdata :: (datablock: *u8, datablock_length_in_bytes: s32, datablock_memory_consumed_in_bytes: *s32, error: *s32, alloc_buffer: *stb_vorbis_alloc) -> *stb_vorbis #foreign libstb_vorbis;
stb_vorbis_seek :: (f: *stb_vorbis, sample_number: u32) -> s32 #foreign libstb_vorbis;
stb_vorbis_seek_frame :: (f: *stb_vorbis, sample_number: u32) -> s32 #foreign libstb_vorbis;
stb_vorbis_seek_start :: (f: *stb_vorbis) -> s32 #foreign libstb_vorbis;
stb_vorbis_stream_length_in_samples :: (f: *stb_vorbis) -> u32 #foreign libstb_vorbis;
stb_vorbis_stream_length_in_seconds :: (f: *stb_vorbis) -> float32 #foreign libstb_vorbis;
