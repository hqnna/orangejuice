//
// The POSIX half: a POSIX shared-memory object, mapped into both processes.
//

// A second magic, so that a mapping made by a different platform's half of
// this module is recognised as such rather than parsed.
Header_Os_Specific :: struct {
    magic: u32;
}

Read_Channel_Os_Specific :: struct {
    shm: s32;
}

Write_Channel_Os_Specific :: struct {
    shm: s32;
}

// Creates the object, sizes it and maps the whole thing. The reader knows how
// big it is, so there is only one mapping to make.
reader_create_channel_os_specific :: (name: string, bytes: s64) -> (success: bool, channel: Read_Channel) {
    total_bytes := bytes + OFFSET_TO_MESSAGE_DATA;
    c_name := Basic.to_c_string(name, Basic.temp);

    shm := Posix.shm_open(c_name, Posix.O_CREAT | Posix.O_RDWR, 0x1ff);
    if shm == -1 {
        error_code, description := System.get_error_value_and_string();
        Basic.log_error("Unable to map a view of '%' for % bytes (which is the user-requested % bytes, plus overhead). Error code %, %", name, total_bytes, bytes, error_code, description);
        return false, .{};
    }

    if Posix.ftruncate(shm, total_bytes) == -1 {
        error_code, description := System.get_error_value_and_string();
        Basic.log_error("Unable to ftruncate '%' for % bytes (which is the user-requested % bytes, plus overhead). Error code %, %. The file descriptor was %.\n", name, total_bytes, bytes, error_code, description, shm);
        return false, .{};
    }

    // `MAP_POPULATE` faults the pages in now rather than on first touch, so
    // the first message does not pay for the whole mapping.
    flags: s32 = Posix.MAP_SHARED | Posix.MAP_POPULATE;
    pointer := Posix.mmap(null, cast(u64) total_bytes, Posix.PROT_READ | Posix.PROT_WRITE, flags, shm, 0);
    if pointer == Posix.MAP_FAILED {
        error_code, description := System.get_error_value_and_string();
        Basic.log_error("Unable to mmap for the channel's memory segment, name '%' bytes '%'. Error code %, %. The file descriptor was %.\n", name, bytes, error_code, description, shm);
        Posix.close(shm);
        return false, .{};
    }

    data := cast(*u8) pointer;
    header_os := cast(*Header_Os_Specific) (data + OFFSET_TO_HEADER_OS_SPECIFIC);
    Basic.Initialize(header_os);
    header_os.magic = (cast(*u32) HEADER_OS_MAGIC.data).*;

    channel: Read_Channel;
    channel.header          = cast(*Header) pointer;
    channel.os_specific.shm = shm;

    return true, channel;
}

reader_release_channel :: (channel: *Read_Channel) -> bool {
    data := cast(*u8) channel.header;
    if !data  return false;

    Posix.munmap(data, cast(u64) channel.header.full_memory_region_size_in_bytes);

    os := *channel.os_specific;
    if os.shm  Posix.close(os.shm);

    return true;
}

// Finds a channel the reader made. The size is in the header, so this maps the
// header, reads it, unmaps it and maps the whole region.
writer_connect :: (name: string, name_not_found_is_quiet := false) -> (success: bool, channel: Write_Channel) {
    c_name := Basic.to_c_string(name, Basic.temp);
    shm := Posix.shm_open(c_name, Posix.O_RDWR, 0);
    if shm == -1 {
        if !name_not_found_is_quiet {
            error_code, description := System.get_error_value_and_string();
            Basic.log_error("In writer_connect(), unable to map a view of name '%'. Error code %, %\n", name, error_code, description);
        }
        return false, .{};
    }

    header_pointer := Posix.mmap(null, size_of(Header), Posix.PROT_READ | Posix.PROT_WRITE, Posix.MAP_SHARED, shm, 0);
    if header_pointer == Posix.MAP_FAILED {
        error_code, description := System.get_error_value_and_string();
        Basic.log_error("In writer_connect(), unable to mmap for the channel's memory segment, name '%'. Error code %, %. The file descriptor was %.\n", name, error_code, description, shm);
        Posix.close(shm);
        return false, .{};
    }

    header_success, actual_bytes := get_size_of_full_mapping_from_stub_header(header_pointer);

    Posix.munmap(header_pointer, size_of(Header));

    if !header_success {
        Posix.close(shm);
        return false, .{};
    }

    header_pointer = Posix.mmap(null, cast(u64) actual_bytes, Posix.PROT_READ | Posix.PROT_WRITE, Posix.MAP_SHARED, shm, 0);
    if header_pointer == Posix.MAP_FAILED {
        error_code, description := System.get_error_value_and_string();
        Basic.log_error("In writer_connect(), unable to mmap for the channel's memory segment, name '%'. Error code %, %\n", name, error_code, description);
        Posix.close(shm);
        return false, .{};
    }

    data := cast(*u8) header_pointer;
    header_os := cast(*Header_Os_Specific) (data + OFFSET_TO_HEADER_OS_SPECIFIC);

    Basic.Initialize(header_os);
    header_os.magic = (cast(*u32) HEADER_OS_MAGIC.data).*;

    channel: Write_Channel;
    channel.header = cast(*Header) header_pointer;
    channel.os_specific.shm = shm;

    return true, channel;
}

writer_disconnect :: (channel: *Write_Channel) -> bool {
    Posix.munmap(channel.header, cast(u64) channel.header.full_memory_region_size_in_bytes);
    Posix.close(channel.os_specific.shm);
    return true;
}

#scope_module

Posix  :: #import "POSIX";
System :: #import "System";

HEADER_OS_MAGIC :: "ShmP";
