//
// Sound_Player: plays sounds — 16-bit PCM and IMA ADPCM from WAV files, and
// Ogg Vorbis — mixed together into the system's default playback device.
//
//     player := New(Sound_Player);
//     init(player, window);
//     data := New(Mixer_Sound_Data);          // filled in from Wav_File, or
//     data.type = .OGG_COMPRESSED;            // the bytes of an .ogg file
//     data.buffer = file_data;
//     stream := make_stream(player, data);
//     while running {
//         pre_entity_update(player);
//         for player.streams  it.marked = true;   // the ones still wanted
//         post_entity_update(player, dt);
//     }
//
// A `Mixer_Sound_Data` is a sound: its samples, or the compressed bytes they
// are decoded from the first time a stream plays it. A `Sound_Stream` is one
// playing of it, with a volume, a rate, a category and a place it has reached.
// Streams belong to the entities that started them: `pre_entity_update`
// clears every stream's `marked`, the program marks the ones its entities
// still want, and `post_entity_update` stops the rest — then mixes, writing
// as many frames as the device has room for. A stream that reaches the end
// of its sound stops too, unless it is `.REPEATING`, which goes back to the
// start from `repeat_end_position` (or from the end of the sound, when that
// is not set).
//
// The mix is 44100 Hz stereo 16-bit. A sound of another rate is resampled
// linearly, at `desired_rate` times its own speed; a mono sound is played on
// both channels. The volume of a stream is its `user_volume_scale` times its
// category's level in `category_volumes` times `master_volume`.
//
// Mixing happens in `post_entity_update`, on the program's own thread: there
// is no mixing thread, so a program that stops calling it stops the sound
// once ALSA's buffer runs out. `mix_streams` mixes into a buffer of the
// program's own instead, which works without a device at all. `init` fails —
// and the player still mixes, into nothing — when there is no ALSA device,
// and on a system other than Linux, which has no backend yet.
//

#import "Basic";
#import "Math";
#import "stb_vorbis";
WT :: #import "Window_Type";

#scope_export

Sound_Category :: enum u8 {
    GENERAL_SFX;
    MUSIC;
    AMBIENCE;
    UI;
    DIALOGUE;
}

Sound_Data_Type :: enum u8 {
    UNINITIALIZED;
    LINEAR_SAMPLE_ARRAY;
    ADPCM_COMPRESSED;
    OGG_COMPRESSED;
}

Mixer_Sound_Data :: struct {
    name: string;
    full_path: string;

    // The bytes of the file the sound came from.
    buffer: string;

    type: Sound_Data_Type;
    nchannels: u16;
    sampling_rate: u32;

    // `.LINEAR_SAMPLE_ARRAY`: the samples, interleaved, and how many there
    // are across all channels. `.ADPCM_COMPRESSED` points `samples` at the
    // encoded blocks, which `wSamplesPerBlock` and `nBlockAlign` describe.
    samples: *s16;
    nsamples_times_nchannels: s64;
    wSamplesPerBlock: u16;
    nBlockAlign: u16;

    // What a compressed sound decoded to, the first time it was played.
    decoded: [] s16;
    decode_failed: bool;
}

Sound_Stream_Flags :: enum_flags u32 {
    REPEATING :: 0x1;
    // Set once the stream has been stopped, by reaching its end or by
    // `stop_stream`; the player lets go of it at the next update.
    STOPPED   :: 0x2;
    PAUSED    :: 0x4;
}

Sound_Stream :: struct {
    sound_data: *Mixer_Sound_Data;
    flags: Sound_Stream_Flags;
    category := Sound_Category.GENERAL_SFX;

    user_volume_scale := 1.0;
    desired_rate      := 1.0;

    // In frames of the sound. Zero or less is the end of the sound.
    repeat_end_position: s64;

    // Set by the program between `pre_entity_update` and
    // `post_entity_update` for a stream it still wants.
    marked: bool;

    // Where the stream is, in frames of the sound.
    play_cursor: float64;
}

Sound_Player :: struct {
    streams: [..] *Sound_Stream;

    master_volume := 1.0;
    category_volumes: [#run enum_highest_value(Sound_Category) + 1] float = .[1, 1, 1, 1, 1];

    // When set, the last `HISTORY_FRAMES` frames that were mixed are kept in
    // `history`, oldest first, for a program that draws the output.
    update_history: bool;
    history: [..] s16;

    sampling_rate: u32 = MIX_SAMPLING_RATE;
    nchannels: u16 = 2;

    window: WT.Window_Type;
    audible_when_window_is_not_in_focus := true;

    device: *void;
    initted: bool;
}

MIX_SAMPLING_RATE :: 44100;

// The ALSA device `init` opens. "null" takes the frames and plays nothing,
// which is what a test wants.
playback_device_name := "default";
HISTORY_FRAMES :: 4096;

// Opens the default playback device. False when there is none; the player
// can still be updated and mixed, into a buffer of the program's own.
init :: (player: *Sound_Player, window: WT.Window_Type, audible_when_window_is_not_in_focus := true, mix_in_update := true) -> bool {
    player.window = window;
    player.audible_when_window_is_not_in_focus = audible_when_window_is_not_in_focus;
    if player.device  return true;

    player.device = device_open(player);
    player.initted = player.device != null;
    return player.initted;
}

shutdown :: (player: *Sound_Player) {
    if player.device {
        device_close(player.device);
        player.device = null;
    }
    for player.streams  free(it);
    array_reset(*player.streams);
    array_reset(*player.history);
    player.initted = false;
}

// Starts playing `data`. The stream is the player's; it lives until it ends
// or the program stops marking it.
make_stream :: (player: *Sound_Player, data: *Mixer_Sound_Data, category := Sound_Category.GENERAL_SFX) -> *Sound_Stream {
    if !data  return null;
    prepare_sound(data);
    stream := New(Sound_Stream);
    stream.sound_data = data;
    stream.category = category;
    array_add(*player.streams, stream);
    return stream;
}

stop_stream :: (stream: *Sound_Stream) {
    if stream  stream.flags |= .STOPPED;
}

// How many frames `data` has, once it has been decoded.
sound_length_in_frames :: (data: *Mixer_Sound_Data) -> s64 {
    prepare_sound(data);
    if !data.nchannels  return 0;
    return data.nsamples_times_nchannels / data.nchannels;
}

pre_entity_update :: (player: *Sound_Player) {
    for player.streams  it.marked = false;
}

// Lets go of the streams nobody marked and the ones that have stopped, then
// writes as much of the mix as the device has room for.
post_entity_update :: (player: *Sound_Player, dt: float) {
    for player.streams {
        if !it.marked  it.flags |= .STOPPED;
    }
    release_stopped_streams(player);

    if !player.device  return;
    frames := device_room(player.device);
    if frames <= 0  return;
    output := NewArray(frames * player.nchannels, s16,, temp);
    mix_streams(player, output);
    device_write(player.device, output, frames);
    release_stopped_streams(player);
}

// Mixes the next `output.count / nchannels` frames of every playing stream
// into `output`, interleaved, and moves the streams on by that much.
mix_streams :: (player: *Sound_Player, output: [] s16) {
    channels := cast(s64) player.nchannels;
    if channels <= 0  return;
    frames := output.count / channels;
    mix := NewArray(frames * channels, float,, temp);

    for stream: player.streams {
        if stream.flags & (.STOPPED | .PAUSED)  continue;
        mix_stream(player, stream, mix, frames);
    }

    for mix  output[it_index] = cast(s16) clamp(it * player.master_volume, -32768, 32767);

    if player.update_history {
        array_add(*player.history, ..output);
        keep := HISTORY_FRAMES * channels;
        if player.history.count > keep {
            drop := player.history.count - keep;
            memcpy(player.history.data, player.history.data + drop, keep * size_of(s16));
            player.history.count = keep;
        }
    }
}

#scope_module

// The device is ALSA's on Linux. Elsewhere there is none yet, and `init`
// says so; mixing into a buffer works everywhere.
#if OS == .LINUX {
    #load "alsa_generated.jai";

    libasound :: #library,system "libasound";

    device_open :: (player: *Sound_Player) -> *void {
        device: *snd_pcm_t;
        if snd_pcm_open(*device, temp_c_string(playback_device_name), .SND_PCM_STREAM_PLAYBACK, SND_PCM_NONBLOCK) < 0  return null;
        LATENCY_MICROSECONDS :: 60000;
        result := snd_pcm_set_params(device, .SND_PCM_FORMAT_S16_LE, .SND_PCM_ACCESS_RW_INTERLEAVED, player.nchannels, player.sampling_rate, 1, LATENCY_MICROSECONDS);
        if result < 0 {
            log_error("Sound_Player: the playback device would not take 16-bit stereo at % Hz: %\n", player.sampling_rate, to_string(snd_strerror(result)));
            snd_pcm_close(device);
            return null;
        }
        return device;
    }

    // How many frames the device has room for, recovering it from an underrun
    // first.
    device_room :: (device: *void) -> s64 {
        pcm := cast(*snd_pcm_t) device;
        room := snd_pcm_avail_update(pcm);
        if room < 0 {
            snd_pcm_recover(pcm, cast(s32) room, 1);
            room = snd_pcm_avail_update(pcm);
        }
        return room;
    }

    device_write :: (device: *void, output: [] s16, frames: s64) {
        pcm := cast(*snd_pcm_t) device;
        written := snd_pcm_writei(pcm, output.data, cast(u64) frames);
        if written < 0  snd_pcm_recover(pcm, cast(s32) written, 1);
    }

    device_close :: (device: *void) {
        pcm := cast(*snd_pcm_t) device;
        snd_pcm_drop(pcm);
        snd_pcm_close(pcm);
    }
} else {
    device_open  :: (player: *Sound_Player) -> *void { return null; }
    device_room  :: (device: *void) -> s64 { return 0; }
    device_write :: (device: *void, output: [] s16, frames: s64) {}
    device_close :: (device: *void) {}
}

release_stopped_streams :: (player: *Sound_Player) {
    for player.streams {
        if it.flags & .STOPPED {
            free(it);
            remove it;
        }
    }
}

// The interleaved samples of a sound, decoding it the first time.
samples_of :: (data: *Mixer_Sound_Data) -> [] s16 {
    prepare_sound(data);
    if data.type == .LINEAR_SAMPLE_ARRAY {
        view: [] s16;
        view.data = data.samples;
        view.count = data.nsamples_times_nchannels;
        return view;
    }
    return data.decoded;
}

prepare_sound :: (data: *Mixer_Sound_Data) {
    if data.decoded.count || data.decode_failed  return;
    if data.type == {
      case .OGG_COMPRESSED;
        channels, rate: s32;
        output: *s16;
        count := stb_vorbis_decode_memory(data.buffer.data, cast(s32) data.buffer.count, *channels, *rate, *output);
        if count <= 0 || channels <= 0 {
            log_error("Sound_Player: '%' did not decode as Ogg Vorbis.\n", data.name);
            data.decode_failed = true;
            return;
        }
        data.nchannels = cast(u16) channels;
        data.sampling_rate = cast(u32) rate;
        data.nsamples_times_nchannels = count * channels;
        data.decoded.data = output;
        data.decoded.count = data.nsamples_times_nchannels;
      case .ADPCM_COMPRESSED;
        data.decoded = decode_ima_adpcm(data);
        if !data.decoded.count  data.decode_failed = true;
    }
}

mix_stream :: (player: *Sound_Player, stream: *Sound_Stream, mix: [] float, frames: s64) {
    data := stream.sound_data;
    samples := samples_of(data);
    source_channels := cast(s64) data.nchannels;
    if !samples.count || source_channels <= 0 || !data.sampling_rate {
        stream.flags |= .STOPPED;
        return;
    }
    length := samples.count / source_channels;
    end := length;
    if stream.repeat_end_position > 0 && stream.repeat_end_position < length  end = stream.repeat_end_position;

    step := cast(float64) data.sampling_rate / player.sampling_rate * stream.desired_rate;
    if step <= 0  return;
    volume := stream.user_volume_scale * player.category_volumes[stream.category];
    channels := cast(s64) player.nchannels;

    cursor := stream.play_cursor;
    for frame: 0..frames - 1 {
        if cursor >= cast(float64) end {
            if !(stream.flags & .REPEATING) || end <= 0 {
                stream.flags |= .STOPPED;
                break;
            }
            cursor -= cast(float64) end;
            if cursor >= cast(float64) end  cursor = 0;
        }
        index := cast(s64) cursor;
        fraction := cast(float) (cursor - index);
        next := index + 1;
        if next >= end  next = ifx stream.flags & .REPEATING then 0 else index;
        for channel: 0..channels - 1 {
            from := min(channel, source_channels - 1);
            a := cast(float) samples[index * source_channels + from];
            b := cast(float) samples[next * source_channels + from];
            mix[frame * channels + channel] += (a + (b - a) * fraction) * volume;
        }
        cursor += step;
    }
    stream.play_cursor = cursor;
}

// IMA ADPCM as a WAV file stores it (`WAVE_FORMAT_DVI_ADPCM`): blocks of
// `nBlockAlign` bytes, each starting with a four-byte header per channel —
// the first sample and the step index — followed by four-bit codes, eight
// samples of one channel per four bytes, channel after channel.
decode_ima_adpcm :: (data: *Mixer_Sound_Data) -> [] s16 {
    channels := cast(s64) data.nchannels;
    block_align := cast(s64) data.nBlockAlign;
    per_block := cast(s64) data.wSamplesPerBlock;
    if channels <= 0 || block_align < 4 * channels || per_block <= 0  return .[];
    if !data.samples  return .[];

    encoded: [] u8;
    encoded.data = cast(*u8) data.samples;
    encoded.count = data.buffer.count - (cast(*u8) data.samples - data.buffer.data);
    if encoded.count <= 0 || encoded.count > data.buffer.count  return .[];

    blocks := (encoded.count + block_align - 1) / block_align;
    wanted := data.nsamples_times_nchannels;
    if wanted <= 0  wanted = blocks * per_block * channels;
    output := NewArray(blocks * per_block * channels, s16);

    predictor: [2] s32;
    step_index: [2] s32;
    written := 0;
    for block: 0..blocks - 1 {
        start := block * block_align;
        size := min(block_align, encoded.count - start);
        if size < 4 * channels  break;
        bytes := encoded.data + start;
        frame_base := block * per_block;
        for channel: 0..channels - 1 {
            header := bytes + channel * 4;
            predictor[channel] = cast(s32) (cast(s16) (cast(u16) header[0] | (cast(u16) header[1] << 8)));
            step_index[channel] = clamp(cast(s32) header[2], 0, 88);
            output[frame_base * channels + channel] = cast(s16) predictor[channel];
        }
        frames_in_block := 1;
        at := 4 * channels;
        while at + 4 * channels <= size && frames_in_block < per_block {
            for channel: 0..channels - 1 {
                chunk := bytes + at + channel * 4;
                for byte: 0..3 {
                    for half: 0..1 {
                        code := (chunk[byte] >> (half * 4)) & 0xF;
                        frame := frames_in_block + byte * 2 + half;
                        sample := ima_step(*predictor[channel], *step_index[channel], code);
                        if frame < per_block  output[(frame_base + frame) * channels + channel] = sample;
                    }
                }
            }
            frames_in_block += 8;
            at += 4 * channels;
        }
        written = (frame_base + min(frames_in_block, per_block)) * channels;
    }
    output.count = min(min(written, wanted), output.count);
    return output;
}

ima_step :: (predictor: *s32, step_index: *s32, code: u8) -> s16 {
    step := IMA_STEPS[step_index.*];
    difference := step >> 3;
    if code & 1  difference += step >> 2;
    if code & 2  difference += step >> 1;
    if code & 4  difference += step;
    if code & 8  predictor.* -= difference;
    else         predictor.* += difference;
    predictor.* = clamp(predictor.*, -32768, 32767);
    step_index.* = clamp(step_index.* + IMA_INDEX_CHANGE[code & 7], 0, 88);
    return cast(s16) predictor.*;
}

// The quantizer step sizes and how each code moves through them, as the IMA
// ADPCM specification gives them.
IMA_INDEX_CHANGE :: s32.[-1, -1, -1, -1, 2, 4, 6, 8];
IMA_STEPS :: s32.[
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45,
    50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230,
    253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963,
    1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327,
    3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442,
    11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];
