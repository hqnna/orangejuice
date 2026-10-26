//
// The part of ALSA's PCM interface Sound_Player plays through: opening the
// default playback device, setting its format, and writing frames to it.
//
// Generated from `alsa/asoundlib.h` by `tools/cbind` — see
// `tools/cbind/README.md`. `module.jai` declares `libasound`.
//


// -------------------------------------------------------------- constants ---

SND_PCM_ASYNC :: 2;
SND_PCM_NONBLOCK :: 1;

// -------------------------------------------------------------- the types ---

snd_pcm_uframes_t :: u64;
snd_pcm_sframes_t :: s64;
snd_pcm_t :: _snd_pcm;
// Opaque: the library owns the contents.
_snd_pcm :: struct {}


// ------------------------------------------------------------------ enums ---

_snd_pcm_stream :: enum s32 {
    SND_PCM_STREAM_PLAYBACK :: 0;
    SND_PCM_STREAM_CAPTURE;
    SND_PCM_STREAM_LAST :: SND_PCM_STREAM_CAPTURE;
}
snd_pcm_stream_t :: _snd_pcm_stream;

_snd_pcm_access :: enum s32 {
    SND_PCM_ACCESS_MMAP_INTERLEAVED :: 0;
    SND_PCM_ACCESS_MMAP_NONINTERLEAVED;
    SND_PCM_ACCESS_MMAP_COMPLEX;
    SND_PCM_ACCESS_RW_INTERLEAVED;
    SND_PCM_ACCESS_RW_NONINTERLEAVED;
    SND_PCM_ACCESS_LAST :: SND_PCM_ACCESS_RW_NONINTERLEAVED;
}
snd_pcm_access_t :: _snd_pcm_access;

_snd_pcm_format :: enum s32 {
    SND_PCM_FORMAT_UNKNOWN :: -1;
    SND_PCM_FORMAT_S8 :: 0;
    SND_PCM_FORMAT_U8;
    SND_PCM_FORMAT_S16_LE;
    SND_PCM_FORMAT_S16_BE;
    SND_PCM_FORMAT_U16_LE;
    SND_PCM_FORMAT_U16_BE;
    SND_PCM_FORMAT_S24_LE;
    SND_PCM_FORMAT_S24_BE;
    SND_PCM_FORMAT_U24_LE;
    SND_PCM_FORMAT_U24_BE;
    SND_PCM_FORMAT_S32_LE;
    SND_PCM_FORMAT_S32_BE;
    SND_PCM_FORMAT_U32_LE;
    SND_PCM_FORMAT_U32_BE;
    SND_PCM_FORMAT_FLOAT_LE;
    SND_PCM_FORMAT_FLOAT_BE;
    SND_PCM_FORMAT_FLOAT64_LE;
    SND_PCM_FORMAT_FLOAT64_BE;
    SND_PCM_FORMAT_IEC958_SUBFRAME_LE;
    SND_PCM_FORMAT_IEC958_SUBFRAME_BE;
    SND_PCM_FORMAT_MU_LAW;
    SND_PCM_FORMAT_A_LAW;
    SND_PCM_FORMAT_IMA_ADPCM;
    SND_PCM_FORMAT_MPEG;
    SND_PCM_FORMAT_GSM;
    SND_PCM_FORMAT_S20_LE;
    SND_PCM_FORMAT_S20_BE;
    SND_PCM_FORMAT_U20_LE;
    SND_PCM_FORMAT_U20_BE;
    SND_PCM_FORMAT_SPECIAL :: 31;
    SND_PCM_FORMAT_S24_3LE :: 32;
    SND_PCM_FORMAT_S24_3BE;
    SND_PCM_FORMAT_U24_3LE;
    SND_PCM_FORMAT_U24_3BE;
    SND_PCM_FORMAT_S20_3LE;
    SND_PCM_FORMAT_S20_3BE;
    SND_PCM_FORMAT_U20_3LE;
    SND_PCM_FORMAT_U20_3BE;
    SND_PCM_FORMAT_S18_3LE;
    SND_PCM_FORMAT_S18_3BE;
    SND_PCM_FORMAT_U18_3LE;
    SND_PCM_FORMAT_U18_3BE;
    SND_PCM_FORMAT_G723_24;
    SND_PCM_FORMAT_G723_24_1B;
    SND_PCM_FORMAT_G723_40;
    SND_PCM_FORMAT_G723_40_1B;
    SND_PCM_FORMAT_DSD_U8;
    SND_PCM_FORMAT_DSD_U16_LE;
    SND_PCM_FORMAT_DSD_U32_LE;
    SND_PCM_FORMAT_DSD_U16_BE;
    SND_PCM_FORMAT_DSD_U32_BE;
    SND_PCM_FORMAT_LAST :: SND_PCM_FORMAT_DSD_U32_BE;
    SND_PCM_FORMAT_S16 :: SND_PCM_FORMAT_S16_LE;
    SND_PCM_FORMAT_U16 :: SND_PCM_FORMAT_U16_LE;
    SND_PCM_FORMAT_S24 :: SND_PCM_FORMAT_S24_LE;
    SND_PCM_FORMAT_U24 :: SND_PCM_FORMAT_U24_LE;
    SND_PCM_FORMAT_S32 :: SND_PCM_FORMAT_S32_LE;
    SND_PCM_FORMAT_U32 :: SND_PCM_FORMAT_U32_LE;
    SND_PCM_FORMAT_FLOAT :: SND_PCM_FORMAT_FLOAT_LE;
    SND_PCM_FORMAT_FLOAT64 :: SND_PCM_FORMAT_FLOAT64_LE;
    SND_PCM_FORMAT_IEC958_SUBFRAME :: SND_PCM_FORMAT_IEC958_SUBFRAME_LE;
    SND_PCM_FORMAT_S20 :: SND_PCM_FORMAT_S20_LE;
    SND_PCM_FORMAT_U20 :: SND_PCM_FORMAT_U20_LE;
}
snd_pcm_format_t :: _snd_pcm_format;

_snd_pcm_state :: enum s32 {
    SND_PCM_STATE_OPEN :: 0;
    SND_PCM_STATE_SETUP;
    SND_PCM_STATE_PREPARED;
    SND_PCM_STATE_RUNNING;
    SND_PCM_STATE_XRUN;
    SND_PCM_STATE_DRAINING;
    SND_PCM_STATE_PAUSED;
    SND_PCM_STATE_SUSPENDED;
    SND_PCM_STATE_DISCONNECTED;
    SND_PCM_STATE_LAST :: SND_PCM_STATE_DISCONNECTED;
    SND_PCM_STATE_PRIVATE1 :: 1024;
}
snd_pcm_state_t :: _snd_pcm_state;


// ------------------------------------------------------------- procedures ---

snd_pcm_avail :: (pcm: *snd_pcm_t) -> snd_pcm_sframes_t #foreign libasound;
snd_pcm_avail_update :: (pcm: *snd_pcm_t) -> snd_pcm_sframes_t #foreign libasound;
snd_pcm_close :: (pcm: *snd_pcm_t) -> s32 #foreign libasound;
snd_pcm_drain :: (pcm: *snd_pcm_t) -> s32 #foreign libasound;
snd_pcm_drop :: (pcm: *snd_pcm_t) -> s32 #foreign libasound;
snd_pcm_get_params :: (pcm: *snd_pcm_t, buffer_size: *snd_pcm_uframes_t, period_size: *snd_pcm_uframes_t) -> s32 #foreign libasound;
snd_pcm_nonblock :: (pcm: *snd_pcm_t, nonblock: s32) -> s32 #foreign libasound;
snd_pcm_open :: (pcm: **snd_pcm_t, name: *u8, stream: snd_pcm_stream_t, mode: s32) -> s32 #foreign libasound;
snd_pcm_prepare :: (pcm: *snd_pcm_t) -> s32 #foreign libasound;
snd_pcm_recover :: (pcm: *snd_pcm_t, err: s32, silent: s32) -> s32 #foreign libasound;
snd_pcm_set_params :: (pcm: *snd_pcm_t, format: snd_pcm_format_t, access: snd_pcm_access_t, channels: u32, rate: u32, soft_resample: s32, latency: u32) -> s32 #foreign libasound;
snd_pcm_start :: (pcm: *snd_pcm_t) -> s32 #foreign libasound;
snd_pcm_state :: (pcm: *snd_pcm_t) -> snd_pcm_state_t #foreign libasound;
snd_pcm_writei :: (pcm: *snd_pcm_t, buffer: *void, size: snd_pcm_uframes_t) -> snd_pcm_sframes_t #foreign libasound;
snd_strerror :: (errnum: s32) -> *u8 #foreign libasound;
