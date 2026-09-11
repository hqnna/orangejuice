//
// Base64, and the URL-safe alphabet beside it.
//
// Three bytes become four characters, each standing for six bits. A length
// that is not a multiple of three is padded out with `=` unless the caller
// says otherwise — the padding carries no information, which is why a URL
// encoding leaves it off.
//
// The alphabet is a parameter rather than a flag, so a caller with one of its
// own can pass it.
//

base64_encode :: (input: string, table: string = BASE64_ENCODER_TABLE, pad := true) -> string #must {
    // Four characters per three bytes, rounded up.
    groups := (input.count + 2) / 3;
    size := groups * 4;

    // Zero-terminated, so that dropping the padding is a matter of leaving the
    // bytes zero and letting the string end early.
    output := cast(*u8) alloc(size + 1);
    output[size] = 0;

    at := 0;
    out := 0;
    while at < input.count {
        // The three bytes as one 24-bit number, short ones padded with zeroes.
        v := cast(int) input[at];
        v = (v << 8) | (ifx at + 1 < input.count then cast(int) input[at + 1] else 0);
        v = (v << 8) | (ifx at + 2 < input.count then cast(int) input[at + 2] else 0);

        output[out]     = table.data[(v >> 18) & 0x3F];
        output[out + 1] = table.data[(v >> 12) & 0x3F];
        output[out + 2] = ifx at + 1 < input.count then table.data[(v >> 6) & 0x3F] else pad_byte(pad);
        output[out + 3] = ifx at + 2 < input.count then table.data[v & 0x3F]        else pad_byte(pad);

        at  += 3;
        out += 4;
    }

    return to_string(output);
}

// `success` is false as soon as a character the table calls invalid turns up;
// whatever had decoded before that comes back with it. A newline is skipped
// and an `=` ends the input.
base64_decode :: (input: string, table: [256] u8 = BASE64_DECODER_TABLE) -> (result: string #must, success: bool) {
    builder: String_Builder;
    buffer: u32;
    held: int;

    for c: cast([] u8) input {
        d := table[c];
        if d == {
            case WHITESPACE; continue;
            case INVALID;    return builder_to_string(*builder), false;
            case EQUALS;     break;
            case;
                buffer = (buffer << 6) | d;
                held += 1;
                if held == 4 {
                    append(*builder, cast(u8) ((buffer >> 16) & 255));
                    append(*builder, cast(u8) ((buffer >> 8)  & 255));
                    append(*builder, cast(u8) (buffer         & 255));
                    buffer = 0;
                    held = 0;
                }
        }
    }

    // Three characters carry two whole bytes, two carry one; the leftover bits
    // are the padding and are dropped.
    if held == 3 {
        append(*builder, cast(u8) ((buffer >> 10) & 255));
        append(*builder, cast(u8) ((buffer >> 2)  & 255));
    } else if held == 2 {
        append(*builder, cast(u8) ((buffer >> 4) & 255));
    }

    return builder_to_string(*builder), true;
}

base64url_encode :: (input: string) -> string {
    return base64_encode(input, BASE64URL_ENCODER_TABLE, false);
}

base64url_decode :: (input: string) -> (result: string, success: bool) {
    return base64_decode(input, BASE64URL_DECODER_TABLE);
}

BASE64_ENCODER_TABLE :: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
BASE64_DECODER_TABLE :: #run decoder_table(BASE64_ENCODER_TABLE);

BASE64URL_ENCODER_TABLE :: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
BASE64URL_DECODER_TABLE :: #run decoder_table(BASE64URL_ENCODER_TABLE);

#scope_file

#import "Basic";

// The three answers that are not a six-bit value.
WHITESPACE :: 64;
EQUALS     :: 65;
INVALID    :: 66;

// The inverse of an alphabet: which six bits each byte stands for, with the
// three sentinels above for the bytes that stand for none.
decoder_table :: (alphabet: string) -> [256] u8 {
    table: [256] u8;
    for * table  it.* = INVALID;

    table[#char "\n"] = WHITESPACE;
    table[#char "="]  = EQUALS;
    for 0..alphabet.count - 1  table[alphabet[it]] = cast(u8) it;

    return table;
}

pad_byte :: inline (pad: bool) -> u8 {
    // Not padding leaves a zero, which is where the returned string ends.
    return ifx pad then cast(u8) #char "=" else 0;
}
