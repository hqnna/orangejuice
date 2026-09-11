//
// Talking to another process on the same machine, through shared memory.
//
// One side is the *reader*, which creates the channel with
// `reader_create_channel` and releases it with `reader_release_channel`. The
// other is the *writer*, which finds it with `writer_connect`, asks for room
// with `writer_reserve_message`, writes into that room and hands it over with
// `writer_commit_message`. The reader takes messages with
// `reader_poll_for_message` and gives each back with
// `reader_done_with_message`.
//
// The queue is a ring, but a message never wraps *within itself*: when one
// will not fit before the end, a sentinel is left in the length slot and the
// message starts again at the beginning. That is what lets the reader hand the
// caller a pointer straight into the shared mapping rather than copying the
// message out of two pieces.
//
// One reader and one writer, one thread each. The cursors are a single
// producer/single consumer pair and nothing here makes them anything else.
//

#load "posix.jai";

Header :: struct {
    magic:   u32;
    version: u32;

    reader_status: enum s32 {
        DISCONNECTED     :: 0;
        PAUSED           :: 1;
        ACTIVELY_READING :: 2;
    };

    writer_status: enum s32 {
        DISCONNECTED     :: 0;
        PAUSED           :: 1;
        ACTIVELY_WRITING :: 2;
    };

    // All three are from the start of the header, so the mapping may be at a
    // different address in each process.
    offset_to_os_specific_header:   u32;
    offset_to_message_queue_header: u32;
    offset_to_message_data:         u32;

    full_memory_region_size_in_bytes: s64;
    // The data area alone: how much unread message can be outstanding.
    data_size_in_bytes:               s64;
}

Read_Channel :: struct {
    header: *Header;
    os_specific: Read_Channel_Os_Specific;
}

Write_Channel :: struct {
    header: *Header;
    os_specific: Write_Channel_Os_Specific;
}

// The two cursors, a cache line apart so that the reader's writes and the
// writer's do not fight over one line.
Message_Queue_Header :: struct {
    read_cursor:  s64;
    pad: [56] u8 = ---;
    write_cursor: s64;
}

Reserved_Message :: struct {
    channel:     *Write_Channel;
    offset:      s64;
    // Overhead included.
    total_bytes: s64;

    // Where the caller writes.
    user_write_pointer: *u8;
}

Read_Message :: struct {
    channel:     *Read_Channel;
    offset:      s64;
    total_bytes: s64;

    user_read_pointer: *u8;
    user_read_bytes:   s64;
}

// Where each part of the mapping starts, from the header. A cache line or two
// apart, so that the parts different processes touch do not share lines.
OFFSET_TO_HEADER_OS_SPECIFIC   :: 64;
OFFSET_TO_MESSAGE_QUEUE_HEADER :: 128;
OFFSET_TO_MESSAGE_DATA         :: 256;

// An 8-byte magic stamp and an 8-byte length before each message. The stamp is
// for finding out what went wrong when something does.
PER_MESSAGE_OVERHEAD_SIZE :: 16;

// Left in a length slot to say "wrap to the start". Every real length is a
// small non-negative number, so this cannot be mistaken for one; it is not -1
// because that is a fill pattern someone else might use.
WRAPPING_SENTINEL :: 0xcafebabef00dface;

#assert size_of(Header) < OFFSET_TO_HEADER_OS_SPECIFIC;
#assert size_of(Header_Os_Specific) + OFFSET_TO_HEADER_OS_SPECIFIC < OFFSET_TO_MESSAGE_QUEUE_HEADER;

reader_create_channel :: (name: string, bytes: s64) -> (success: bool, channel: Read_Channel) {
    success, channel := reader_create_channel_os_specific(name, bytes);
    if !success  return false, .{};

    header := channel.header;

    Basic.Initialize(header);

    header.magic         = (cast(*u32) CHANNEL_MAGIC.data).*;
    header.version       = PROTOCOL_VERSION;
    header.reader_status = .ACTIVELY_READING;

    header.offset_to_os_specific_header   = OFFSET_TO_HEADER_OS_SPECIFIC;
    header.offset_to_message_queue_header = OFFSET_TO_MESSAGE_QUEUE_HEADER;
    header.offset_to_message_data         = OFFSET_TO_MESSAGE_DATA;

    header.full_memory_region_size_in_bytes = bytes + OFFSET_TO_MESSAGE_DATA;
    header.data_size_in_bytes               = bytes;

    mq := get_message_queue_header(header);
    mq.read_cursor  = OFFSET_TO_MESSAGE_DATA;
    mq.write_cursor = OFFSET_TO_MESSAGE_DATA;

    return true, channel;
}

get_message_queue_header :: inline (header: *Header) -> *Message_Queue_Header {
    base := cast(*u8) header;
    return cast(*Message_Queue_Header) (base + header.offset_to_message_queue_header);
}

// ------------------------------------------------------------- the writer ---

// Room for `data_bytes`, or failure when the reader has not kept up. Nothing
// is visible to the reader until `writer_commit_message`.
writer_reserve_message :: (channel: *Write_Channel, data_bytes: s64) -> (success: bool, reserved: Reserved_Message) {
    total_bytes := data_bytes + PER_MESSAGE_OVERHEAD_SIZE;

    header := channel.header;
    arena_start := header.offset_to_message_data;
    arena_end   := arena_start + header.data_size_in_bytes;

    mqh := get_message_queue_header(header);
    if !mqh  return false, .{};

    // Zero means unset: every real offset is past the header.
    target_position := 0;
    write_wrapping_sentinel_at := 0;

    write_cursor := mqh.write_cursor;
    read_cursor  := mqh.read_cursor;
    if write_cursor >= read_cursor {
        remaining := arena_end - write_cursor;
        if remaining >= total_bytes {
            target_position = write_cursor;
        } else {
            // Too little room before the end. If there is space for the
            // sentinel, leave one; with less than that the reader knows to
            // wrap anyway.
            end := header.offset_to_message_data + header.data_size_in_bytes;
            if end - write_cursor >= PER_MESSAGE_OVERHEAD_SIZE  write_wrapping_sentinel_at = write_cursor;

            write_cursor = header.offset_to_message_data;

            // -1 because the write cursor may never meet the read cursor: that
            // is how "empty" is told from "full".
            if total_bytes >= header.data_size_in_bytes - 1  return false, .{};
        }
    }

    if !target_position {
        // The reader may have caught up with us exactly as we wrapped.
        assert(write_cursor <= read_cursor);
        remaining := read_cursor - write_cursor - 1;

        if remaining >= total_bytes {
            target_position = header.offset_to_message_data;
        } else {
            return false, .{};
        }
    }

    if write_wrapping_sentinel_at {
        // At the very start it would send the reader round for ever without
        // making progress — and there would have been room anyway.
        assert(write_wrapping_sentinel_at > header.offset_to_message_data);
        dest := (cast(*u8) header) + write_wrapping_sentinel_at;
        (cast(*u64) dest).* = WRAPPING_SENTINEL;
    }

    assert(target_position > 0);

    data_start := cast(*u8) header;

    result: Reserved_Message;
    result.offset      = write_cursor;
    result.total_bytes = total_bytes;
    result.channel     = channel;

    result.user_write_pointer = data_start + write_cursor + PER_MESSAGE_OVERHEAD_SIZE;

    return true, result;
}

// The stamp and the length go in first and the cursor moves last, so the
// reader never sees a length before the bytes it describes.
writer_commit_message :: (channel: *Write_Channel, message: Reserved_Message) -> (success: bool) {
    assert(message.channel == channel);
    if message.channel != channel  return false;

    mqh := get_message_queue_header(channel.header);
    if !mqh  return false;

    data := cast(*u8) channel.header;

    magic_pointer  := cast(*u64) (data + message.offset);
    length_pointer := cast(*s64) (data + message.offset + 8);

    assert(data + message.offset + 16 == message.user_write_pointer);
    magic_pointer.*  = (cast(*u64) MESSAGE_MAGIC.data).*;
    length_pointer.* = message.total_bytes;

    mqh.write_cursor = message.offset + message.total_bytes;

    return true;
}

// ------------------------------------------------------------- the reader ---

// `found` false with `fatal_error` false only means nothing has arrived yet;
// `fatal_error` means the queue no longer makes sense and must be abandoned.
reader_poll_for_message :: (channel: *Read_Channel) -> (found: bool, fatal_error: bool, message: Read_Message) {
    header := channel.header;

    mqh := get_message_queue_header(header);
    if !mqh  return false, true, .{};

    read_cursor := mqh.read_cursor;
    if read_cursor == mqh.write_cursor  return false, false, .{};

    data := cast(*u8) header;

    magic_pointer  := cast(*u64) (data + read_cursor);
    length_pointer := cast(*s64) (data + read_cursor + 8);

    desired_magic := (cast(*u64) MESSAGE_MAGIC.data).*;
    magic := magic_pointer.*;
    if magic != desired_magic {
        if magic == WRAPPING_SENTINEL {
            // A sentinel at the very start would send us round again without
            // making progress, so it is an error rather than a wrap.
            if read_cursor == header.offset_to_message_data {
                Basic.log_error("In reader_poll_for_message(), we got a WRAPPING_SENTINEL at the start of the buffer, which is invalid. We must stop.\n");
                return false, true, .{};
            }

            mqh.read_cursor = header.offset_to_message_data;
            found, fatal_error, message := #this(channel);
            return found, fatal_error, message;
        }

        Basic.log_error("Invalid magic number 0x% at offset % in reader_poll_for_message(): we got a parse error and must stop.\n", Basic.formatInt(magic, base = 16), read_cursor);

        return false, true, .{};
    }

    length := length_pointer.*;
    if length < PER_MESSAGE_OVERHEAD_SIZE {
        Basic.log_error("In reader_poll_for_message(), we got a length of %, which is invalid. Failing.\n", length);
        return false, true, .{};
    }

    end := header.offset_to_message_data + header.data_size_in_bytes;
    remaining := end - read_cursor;
    if remaining < length {
        Basic.log_error("In reader_poll_for_message(), we got a length of %, which is too long (only % bytes are left in the buffer). Failing.\n", length, remaining);
        return false, true, .{};
    }

    message: Read_Message;
    message.channel     = channel;
    message.offset      = read_cursor;
    message.total_bytes = length;

    message.user_read_pointer = data + read_cursor + PER_MESSAGE_OVERHEAD_SIZE;
    message.user_read_bytes   = length - PER_MESSAGE_OVERHEAD_SIZE;

    return true, false, message;
}

// Releases the room the message was in. Until this is called, the writer may
// not use it.
reader_done_with_message :: (message: *Read_Message) -> bool {
    channel := message.channel;
    assert(channel != null);

    header := channel.header;

    mqh := get_message_queue_header(header);
    if !mqh  return false;

    read_cursor := mqh.read_cursor;
    read_cursor += message.total_bytes;

    // Too little left for even a header: the writer will have wrapped.
    end := header.offset_to_message_data + header.data_size_in_bytes;
    if end - read_cursor < PER_MESSAGE_OVERHEAD_SIZE  read_cursor = OFFSET_TO_MESSAGE_DATA;

    mqh.read_cursor = read_cursor;

    return true;
}

PROTOCOL_VERSION :: 1;
CHANNEL_MAGIC    :: "ShmC";
MESSAGE_MAGIC    :: "ShmM2022";

#scope_module

// The size of the mapping is in the header, which has to be mapped first to
// find it out.
get_size_of_full_mapping_from_stub_header :: (pointer: *void) -> (success: bool, actual_bytes: s64) {
    header_copy := (cast(*Header) pointer).*;
    desired_magic := (cast(*u32) CHANNEL_MAGIC.data).*;

    success := true;
    if header_copy.magic != desired_magic {
        Basic.log_error("In writer_connect(), the magic number of the mapped header did not match. (Wanted 0x%, got 0x%).\n", Basic.formatInt(desired_magic, base = 16), Basic.formatInt(header_copy.magic, base = 16));
        success = false;
    } else if header_copy.version != PROTOCOL_VERSION {
        Basic.log_error("In writer_connect(), the protocol version did not match. (Wanted %, got %).\n", PROTOCOL_VERSION, header_copy.version);
        success = false;
    }

    actual_bytes := header_copy.full_memory_region_size_in_bytes;
    if actual_bytes < OFFSET_TO_MESSAGE_DATA {
        Basic.log_error("In writer_connect(), the full_memory_region_size_in_bytes we got from the header was %, which is too small.\n", actual_bytes);
        success = false;
    }

    return success, actual_bytes;
}

Basic :: #import "Basic";
assert :: Basic.assert;
