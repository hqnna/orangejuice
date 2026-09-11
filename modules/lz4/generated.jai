//
// The lz4 surface: the types, constants and procedures that `lz4/module.jai`
// does not declare by hand.
//
// Generated from lz4's own headers by `tools/cbind` — see
// `tools/cbind/README.md`. The library itself is not vendored; the `#library`
// in `module.jai` says where it is found.
//

#scope_module

// The reference vendors liblz4; we bind the system one instead, so nothing
// binary lives in the tree.
lz4lib :: #system_library "lz4";

#scope_export

// ------------------------------------------------------------- constants ---

LZ4HC_CLEVEL_DEFAULT :: 9;
LZ4HC_CLEVEL_MAX :: 12;
LZ4HC_CLEVEL_MIN :: 2;
LZ4HC_CLEVEL_OPT_MIN :: 10;
LZ4HC_DICTIONARY_LOGSIZE :: 16;
LZ4HC_HASHTABLESIZE :: 32768;
LZ4HC_HASH_LOG :: 15;
LZ4HC_HASH_MASK :: 32767;
LZ4HC_MAXD :: 65536;
LZ4HC_MAXD_MASK :: 65535;
LZ4_HASHLOG :: 12;
LZ4_HASHTABLESIZE :: 16384;
LZ4_HASH_SIZE_U32 :: 4096;
LZ4_MAX_INPUT_SIZE :: 2113929216;
LZ4_MEMORY_USAGE :: 14;
LZ4_MEMORY_USAGE_DEFAULT :: 14;
LZ4_MEMORY_USAGE_MAX :: 20;
LZ4_MEMORY_USAGE_MIN :: 10;
LZ4_STREAMDECODE_MINSIZE :: 32;
LZ4_STREAMHC_MINSIZE :: 262200;
LZ4_STREAM_MINSIZE :: 16416;
LZ4_VERSION_MAJOR :: 1;
LZ4_VERSION_MINOR :: 10;
LZ4_VERSION_NUMBER :: 11000;
LZ4_VERSION_RELEASE :: 0;

// ------------------------------------------------------------- the types ---

LZ4_stream_t :: LZ4_stream_u;
LZ4_streamDecode_t :: LZ4_streamDecode_u;
LZ4_i8 :: int8_t;
LZ4_byte :: uint8_t;
LZ4_u16 :: uint16_t;
LZ4_u32 :: uint32_t;
LZ4_stream_t_internal :: struct {
    hashTable: [4096] LZ4_u32;
    dictionary: *LZ4_byte;
    dictCtx: *LZ4_stream_t_internal;
    currentOffset: LZ4_u32;
    tableType: LZ4_u32;
    dictSize: LZ4_u32;
}

LZ4_stream_u :: union {
    minStateSize: [16416] u8;
    internal_donotuse: LZ4_stream_t_internal;
}

LZ4_streamDecode_t_internal :: struct {
    externalDict: *LZ4_byte;
    prefixEnd: *LZ4_byte;
    extDictSize: size_t;
    prefixSize: size_t;
}

LZ4_streamDecode_u :: union {
    minStateSize: [32] u8;
    internal_donotuse: LZ4_streamDecode_t_internal;
}

LZ4_streamHC_t :: LZ4_streamHC_u;
LZ4HC_CCtx_internal :: struct {
    hashTable: [32768] LZ4_u32;
    chainTable: [65536] LZ4_u16;
    end: *LZ4_byte;
    prefixStart: *LZ4_byte;
    dictStart: *LZ4_byte;
    dictLimit: LZ4_u32;
    lowLimit: LZ4_u32;
    nextToUpdate: LZ4_u32;
    compressionLevel: s16;
    favorDecSpeed: LZ4_i8;
    dirty: LZ4_i8;
    dictCtx: *LZ4HC_CCtx_internal;
}

LZ4_streamHC_u :: union {
    minStateSize: [262200] u8;
    internal_donotuse: LZ4HC_CCtx_internal;
}

size_t :: u64;
int8_t :: __int8_t;
uint8_t :: __uint8_t;
uint16_t :: __uint16_t;
uint32_t :: __uint32_t;
__int8_t :: u8;
__uint8_t :: u8;
__uint16_t :: u16;
__uint32_t :: u32;

// ------------------------------------------------------------------ enums ---


// ------------------------------------------------------------- procedures ---

LZ4_compress :: (src: *u8, dest: *u8, srcSize: s32) -> s32 #foreign lz4lib;
LZ4_compressBound :: (inputSize: s32) -> s32 #foreign lz4lib;
LZ4_compressHC :: (source: *u8, dest: *u8, inputSize: s32) -> s32 #foreign lz4lib;
LZ4_compressHC2 :: (source: *u8, dest: *u8, inputSize: s32, compressionLevel: s32) -> s32 #foreign lz4lib;
LZ4_compressHC2_continue :: (LZ4HC_Data: *void, source: *u8, dest: *u8, inputSize: s32, compressionLevel: s32) -> s32 #foreign lz4lib;
LZ4_compressHC2_limitedOutput :: (source: *u8, dest: *u8, inputSize: s32, maxOutputSize: s32, compressionLevel: s32) -> s32 #foreign lz4lib;
LZ4_compressHC2_limitedOutput_continue :: (LZ4HC_Data: *void, source: *u8, dest: *u8, inputSize: s32, maxOutputSize: s32, compressionLevel: s32) -> s32 #foreign lz4lib;
LZ4_compressHC2_limitedOutput_withStateHC :: (state: *void, source: *u8, dest: *u8, inputSize: s32, maxOutputSize: s32, compressionLevel: s32) -> s32 #foreign lz4lib;
LZ4_compressHC2_withStateHC :: (state: *void, source: *u8, dest: *u8, inputSize: s32, compressionLevel: s32) -> s32 #foreign lz4lib;
LZ4_compressHC_continue :: (LZ4_streamHCPtr: *LZ4_streamHC_t, source: *u8, dest: *u8, inputSize: s32) -> s32 #foreign lz4lib;
LZ4_compressHC_limitedOutput :: (source: *u8, dest: *u8, inputSize: s32, maxOutputSize: s32) -> s32 #foreign lz4lib;
LZ4_compressHC_limitedOutput_continue :: (LZ4_streamHCPtr: *LZ4_streamHC_t, source: *u8, dest: *u8, inputSize: s32, maxOutputSize: s32) -> s32 #foreign lz4lib;
LZ4_compressHC_limitedOutput_withStateHC :: (state: *void, source: *u8, dest: *u8, inputSize: s32, maxOutputSize: s32) -> s32 #foreign lz4lib;
LZ4_compressHC_withStateHC :: (state: *void, source: *u8, dest: *u8, inputSize: s32) -> s32 #foreign lz4lib;
LZ4_compress_HC :: (src: *u8, dst: *u8, srcSize: s32, dstCapacity: s32, compressionLevel: s32) -> s32 #foreign lz4lib;
LZ4_compress_HC_continue :: (streamHCPtr: *LZ4_streamHC_t, src: *u8, dst: *u8, srcSize: s32, maxDstSize: s32) -> s32 #foreign lz4lib;
LZ4_compress_HC_continue_destSize :: (LZ4_streamHCPtr: *LZ4_streamHC_t, src: *u8, dst: *u8, srcSizePtr: *s32, targetDstSize: s32) -> s32 #foreign lz4lib;
LZ4_compress_HC_destSize :: (stateHC: *void, src: *u8, dst: *u8, srcSizePtr: *s32, targetDstSize: s32, compressionLevel: s32) -> s32 #foreign lz4lib;
LZ4_compress_HC_extStateHC :: (stateHC: *void, src: *u8, dst: *u8, srcSize: s32, maxDstSize: s32, compressionLevel: s32) -> s32 #foreign lz4lib;
LZ4_compress_continue :: (LZ4_streamPtr: *LZ4_stream_t, source: *u8, dest: *u8, inputSize: s32) -> s32 #foreign lz4lib;
LZ4_compress_default :: (src: *u8, dst: *u8, srcSize: s32, dstCapacity: s32) -> s32 #foreign lz4lib;
LZ4_compress_destSize :: (src: *u8, dst: *u8, srcSizePtr: *s32, targetDstSize: s32) -> s32 #foreign lz4lib;
LZ4_compress_fast :: (src: *u8, dst: *u8, srcSize: s32, dstCapacity: s32, acceleration: s32) -> s32 #foreign lz4lib;
LZ4_compress_fast_continue :: (streamPtr: *LZ4_stream_t, src: *u8, dst: *u8, srcSize: s32, dstCapacity: s32, acceleration: s32) -> s32 #foreign lz4lib;
LZ4_compress_fast_extState :: (state: *void, src: *u8, dst: *u8, srcSize: s32, dstCapacity: s32, acceleration: s32) -> s32 #foreign lz4lib;
LZ4_compress_limitedOutput :: (src: *u8, dest: *u8, srcSize: s32, maxOutputSize: s32) -> s32 #foreign lz4lib;
LZ4_compress_limitedOutput_continue :: (LZ4_streamPtr: *LZ4_stream_t, source: *u8, dest: *u8, inputSize: s32, maxOutputSize: s32) -> s32 #foreign lz4lib;
LZ4_compress_limitedOutput_withState :: (state: *void, source: *u8, dest: *u8, inputSize: s32, maxOutputSize: s32) -> s32 #foreign lz4lib;
LZ4_compress_withState :: (state: *void, source: *u8, dest: *u8, inputSize: s32) -> s32 #foreign lz4lib;
LZ4_create :: (inputBuffer: *u8) -> *void #foreign lz4lib;
LZ4_createHC :: (inputBuffer: *u8) -> *void #foreign lz4lib;
LZ4_createStream :: () -> *LZ4_stream_t #foreign lz4lib;
LZ4_createStreamDecode :: () -> *LZ4_streamDecode_t #foreign lz4lib;
LZ4_createStreamHC :: () -> *LZ4_streamHC_t #foreign lz4lib;
LZ4_decoderRingBufferSize :: (maxBlockSize: s32) -> s32 #foreign lz4lib;
LZ4_decompress_fast :: (src: *u8, dst: *u8, originalSize: s32) -> s32 #foreign lz4lib;
LZ4_decompress_fast_continue :: (LZ4_streamDecode: *LZ4_streamDecode_t, src: *u8, dst: *u8, originalSize: s32) -> s32 #foreign lz4lib;
LZ4_decompress_fast_usingDict :: (src: *u8, dst: *u8, originalSize: s32, dictStart: *u8, dictSize: s32) -> s32 #foreign lz4lib;
LZ4_decompress_fast_withPrefix64k :: (src: *u8, dst: *u8, originalSize: s32) -> s32 #foreign lz4lib;
LZ4_decompress_safe :: (src: *u8, dst: *u8, compressedSize: s32, dstCapacity: s32) -> s32 #foreign lz4lib;
LZ4_decompress_safe_continue :: (LZ4_streamDecode: *LZ4_streamDecode_t, src: *u8, dst: *u8, srcSize: s32, dstCapacity: s32) -> s32 #foreign lz4lib;
LZ4_decompress_safe_partial :: (src: *u8, dst: *u8, srcSize: s32, targetOutputSize: s32, dstCapacity: s32) -> s32 #foreign lz4lib;
LZ4_decompress_safe_partial_usingDict :: (src: *u8, dst: *u8, compressedSize: s32, targetOutputSize: s32, maxOutputSize: s32, dictStart: *u8, dictSize: s32) -> s32 #foreign lz4lib;
LZ4_decompress_safe_usingDict :: (src: *u8, dst: *u8, srcSize: s32, dstCapacity: s32, dictStart: *u8, dictSize: s32) -> s32 #foreign lz4lib;
LZ4_decompress_safe_withPrefix64k :: (src: *u8, dst: *u8, compressedSize: s32, maxDstSize: s32) -> s32 #foreign lz4lib;
LZ4_freeHC :: (LZ4HC_Data: *void) -> s32 #foreign lz4lib;
LZ4_freeStream :: (streamPtr: *LZ4_stream_t) -> s32 #foreign lz4lib;
LZ4_freeStreamDecode :: (LZ4_stream: *LZ4_streamDecode_t) -> s32 #foreign lz4lib;
LZ4_freeStreamHC :: (streamHCPtr: *LZ4_streamHC_t) -> s32 #foreign lz4lib;
LZ4_initStream :: (stateBuffer: *void, size: size_t) -> *LZ4_stream_t #foreign lz4lib;
LZ4_initStreamHC :: (buffer: *void, size: size_t) -> *LZ4_streamHC_t #foreign lz4lib;
LZ4_loadDict :: (streamPtr: *LZ4_stream_t, dictionary: *u8, dictSize: s32) -> s32 #foreign lz4lib;
LZ4_loadDictHC :: (streamHCPtr: *LZ4_streamHC_t, dictionary: *u8, dictSize: s32) -> s32 #foreign lz4lib;
LZ4_resetStream :: (streamPtr: *LZ4_stream_t) -> void #foreign lz4lib;
LZ4_resetStreamHC :: (streamHCPtr: *LZ4_streamHC_t, compressionLevel: s32) -> void #foreign lz4lib;
LZ4_resetStreamHC_fast :: (streamHCPtr: *LZ4_streamHC_t, compressionLevel: s32) -> void #foreign lz4lib;
LZ4_resetStreamState :: (state: *void, inputBuffer: *u8) -> s32 #foreign lz4lib;
LZ4_resetStreamStateHC :: (state: *void, inputBuffer: *u8) -> s32 #foreign lz4lib;
LZ4_resetStream_fast :: (streamPtr: *LZ4_stream_t) -> void #foreign lz4lib;
LZ4_saveDict :: (streamPtr: *LZ4_stream_t, safeBuffer: *u8, maxDictSize: s32) -> s32 #foreign lz4lib;
LZ4_saveDictHC :: (streamHCPtr: *LZ4_streamHC_t, safeBuffer: *u8, maxDictSize: s32) -> s32 #foreign lz4lib;
LZ4_setStreamDecode :: (LZ4_streamDecode: *LZ4_streamDecode_t, dictionary: *u8, dictSize: s32) -> s32 #foreign lz4lib;
LZ4_sizeofState :: () -> s32 #foreign lz4lib;
LZ4_sizeofStateHC :: () -> s32 #foreign lz4lib;
LZ4_sizeofStreamState :: () -> s32 #foreign lz4lib;
LZ4_sizeofStreamStateHC :: () -> s32 #foreign lz4lib;
LZ4_slideInputBuffer :: (state: *void) -> *u8 #foreign lz4lib;
LZ4_slideInputBufferHC :: (LZ4HC_Data: *void) -> *u8 #foreign lz4lib;
LZ4_uncompress :: (source: *u8, dest: *u8, outputSize: s32) -> s32 #foreign lz4lib;
LZ4_uncompress_unknownOutputSize :: (source: *u8, dest: *u8, isize: s32, maxOutputSize: s32) -> s32 #foreign lz4lib;
LZ4_versionNumber :: () -> s32 #foreign lz4lib;
LZ4_versionString :: () -> *u8 #foreign lz4lib;
