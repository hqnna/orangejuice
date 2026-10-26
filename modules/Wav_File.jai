//
// Wav_File: reading the header of a RIFF WAVE file.
//
//     format, samples, ok, extra := get_wav_header(file_data);
//     if ok && format.wFormatTag == WAVE_FORMAT_PCM { ... }
//
// `samples` is a view into the buffer that was passed in — the bytes of the
// `data` chunk — so nothing is copied and the buffer has to outlive it. The
// format is the `fmt ` chunk as Windows' `WAVEFORMATEX` lays it out, and
// `extra` carries what an IMA ADPCM file adds: the samples per block from the
// format's extension and the sample count from the `fact` chunk.
//
// `write_wav_header` goes the other way, for 16-bit PCM.
//

#import "Basic";

WAVE_FORMAT_PCM        :: 0x0001;
WAVE_FORMAT_IEEE_FLOAT :: 0x0003;
WAVE_FORMAT_DVI_ADPCM  :: 0x0011;
WAVE_FORMAT_EXTENSIBLE :: 0xFFFE;

WAVEFORMATEX :: struct {
    wFormatTag:      u16;
    nChannels:       u16;
    nSamplesPerSec:  u32;
    nAvgBytesPerSec: u32;
    nBlockAlign:     u16;
    wBitsPerSample:  u16;
    cbSize:          u16;
}

Wav_Extra_Info :: struct {
    wSamplesPerBlock:             u16;
    wSamplesAccordingToFactChunk: s32;
}

// Walks the chunks of a RIFF WAVE file. `success` is false when the bytes are
// not one, or have no `fmt ` or `data` chunk.
get_wav_header :: (buffer: string) -> (format: WAVEFORMATEX, samples: string, success: bool, extra: Wav_Extra_Info) {
    format: WAVEFORMATEX;
    samples: string;
    extra: Wav_Extra_Info;

    if buffer.count < 12  return format, samples, false, extra;
    if !tag_is(buffer, 0, "RIFF") || !tag_is(buffer, 8, "WAVE")  return format, samples, false, extra;

    found_format := false;
    found_data := false;
    at := 12;
    while at + 8 <= buffer.count {
        size := cast(s64) read_u32(buffer, at + 4);
        body := at + 8;
        if size < 0 || body + size > buffer.count {
            // A truncated last chunk still has what it has.
            size = buffer.count - body;
        }

        if tag_is(buffer, at, "fmt ") && size >= 16 {
            format.wFormatTag      = read_u16(buffer, body);
            format.nChannels       = read_u16(buffer, body + 2);
            format.nSamplesPerSec  = read_u32(buffer, body + 4);
            format.nAvgBytesPerSec = read_u32(buffer, body + 8);
            format.nBlockAlign     = read_u16(buffer, body + 12);
            format.wBitsPerSample  = read_u16(buffer, body + 14);
            if size >= 18  format.cbSize = read_u16(buffer, body + 16);
            if format.wFormatTag == WAVE_FORMAT_DVI_ADPCM && size >= 20 {
                extra.wSamplesPerBlock = read_u16(buffer, body + 18);
            }
            found_format = true;
        } else if tag_is(buffer, at, "fact") && size >= 4 {
            extra.wSamplesAccordingToFactChunk = cast(s32) read_u32(buffer, body);
        } else if tag_is(buffer, at, "data") {
            samples.data = buffer.data + body;
            samples.count = size;
            found_data = true;
        }

        // Chunks are padded to an even length.
        at = body + size + (size & 1);
    }

    return format, samples, found_format && found_data, extra;
}

// A 44-byte header for `data_size` bytes of 16-bit PCM, ready to be followed
// by the samples.
write_wav_header :: (builder: *String_Builder, data_size: s64, channels: u16, sampling_rate: u32) {
    bits :: 16;
    block_align := cast(u16)(channels * bits / 8);
    append(builder, "RIFF");
    put_u32(builder, cast(u32)(36 + data_size));
    append(builder, "WAVEfmt ");
    put_u32(builder, 16);
    put_u16(builder, WAVE_FORMAT_PCM);
    put_u16(builder, channels);
    put_u32(builder, sampling_rate);
    put_u32(builder, sampling_rate * block_align);
    put_u16(builder, block_align);
    put_u16(builder, bits);
    append(builder, "data");
    put_u32(builder, cast(u32) data_size);
}

#scope_module

tag_is :: (buffer: string, at: s64, tag: string) -> bool {
    if at + 4 > buffer.count  return false;
    for 0..3  if buffer[at + it] != tag[it]  return false;
    return true;
}

read_u16 :: (buffer: string, at: s64) -> u16 {
    return cast(u16) buffer[at] | (cast(u16) buffer[at + 1] << 8);
}

read_u32 :: (buffer: string, at: s64) -> u32 {
    return cast(u32) buffer[at] | (cast(u32) buffer[at + 1] << 8) | (cast(u32) buffer[at + 2] << 16) | (cast(u32) buffer[at + 3] << 24);
}

put_u16 :: (builder: *String_Builder, value: u16) {
    append(builder, cast(u8)(value & 0xFF));
    append(builder, cast(u8)(value >> 8));
}

put_u32 :: (builder: *String_Builder, value: u32) {
    for 0..3  append(builder, cast(u8)((value >> (it * 8)) & 0xFF));
}
