//
// Reading and writing files.
//
// `read_entire_file` and `write_entire_file` are what most programs want. The
// `file_*` routines under them are for a program that reads a file in pieces,
// seeks about in it, or holds one open across a loop.
//
// `map_entire_file_start` exists so that a caller can be written once and get
// a memory mapping where the platform has a cheap one; on Linux it reads the
// file, since a mapping of a file that changes underneath is a crash waiting
// to happen rather than a saving.
//

#load "unix.jai";

file_write :: inline (file: *File, data: string) -> bool {
    return file_write(file, data.data, data.count);
}

// Empties the builder as it goes unless told otherwise, so nothing is leaked
// even when a write fails part way.
file_write :: (file: *File, builder: *String_Builder, do_reset := true) -> bool {
    defer if do_reset  reset(builder);

    buffer := get_base_buffer(builder);
    while buffer {
        if !file_write(file, get_buffer_data(buffer), buffer.count)  return false;
        buffer = buffer.next;
    }

    return true;
}

write_entire_file :: inline (name: string, data: string) -> bool {
    return write_entire_file(name, data.data, data.count);
}

write_entire_file :: (name: string, data: *void, count: int) -> bool {
    file, success := file_open(name, for_writing = true, keep_existing_content = false);
    if !success  return false;

    defer file_close(*file);

    return file_write(*file, data, count);
}

write_entire_file :: (name: string, builder: *String_Builder, do_reset := true) -> bool {
    file, success := file_open(name, for_writing = true, keep_existing_content = false);
    if !success {
        if do_reset  reset(builder);
        return false;
    }

    defer file_close(*file);

    return file_write(*file, builder, do_reset);
}

read_entire_file :: (name: string, zero_terminated := false, log_errors := true) -> (contents: string, success: bool) {
    s: string;
    f, success := file_open(name, log_errors = log_errors);
    if !success  return s, false;

    defer file_close(*f);

    s, success = read_entire_file(f, zero_terminated);

    if log_errors && !success {
        System :: #import "System";
        code, description := System.get_error_value_and_string();
        log("Read of '%' failed: code %, '%'.\n", name, code, description);
    }

    return s, success;
}

// Whether a mapping is used at all. Only Windows has one here.
USE_MAP_FILE :: (OS == .WINDOWS);
STORE_MAPPED_FILENAMES :: false;

Map_File_Info :: struct {
    data: string;
    #if STORE_MAPPED_FILENAMES {
        filename: string;
    }
}

// For a caller that already has the bytes and wants to hand them to something
// written against this interface.
string_to_map_file_info :: (data: string) -> Map_File_Info {
    file_info: Map_File_Info;
    file_info.data = data;
    return file_info;
}

map_entire_file_start :: (name: string, log_errors := true) -> (file_info: Map_File_Info, success: bool) {
    file_info: Map_File_Info;

    data, ok := read_entire_file(name, false, log_errors);
    if !ok  return .{}, false;

    file_info.data = data;
    #if STORE_MAPPED_FILENAMES  file_info.filename = copy_string(name);
    return file_info, true;
}

map_entire_file_end :: (file_info: *Map_File_Info) {
    free(file_info.data);
    file_info.data = "";

    #if STORE_MAPPED_FILENAMES {
        if file_info.filename {
            free(file_info.filename);
            file_info.filename = "";
        }
    }
}

// `recursive` makes each directory along the path in turn, rather than only
// the last one.
make_directory_if_it_does_not_exist :: (name: string, recursive := false) -> (directory_now_exists: bool) {
    if recursive {
        start_index := 0;
        // A leading separator is the root, which is already there.
        if name && name[0] == #char "/"  start_index = 1;

        index := find_index_from_left(name, #char "/", start_index);
        while index != -1 {
            if !os_make_directory_if_it_does_not_exist(slice(name, 0, index))  return false;
            start_index = index + 1;
            index = find_index_from_left(name, #char "/", start_index);
        }

        // A path ending in a separator has had its last directory made
        // already.
        if start_index == name.count - 1  return true;
    }

    return os_make_directory_if_it_does_not_exist(name);
}

// A relative path made absolute, in temporary storage. It fails for a path
// that does not exist, since resolving the symlinks along it means looking at
// what is really there.
get_absolute_path :: inline (relative_path: string) -> (absolute_path: string, success: bool) {
    return get_absolute_path_unix(relative_path);
}

#scope_module

Seek_Type :: enum {
    START   :: 0;
    CURRENT :: 1;
    END     :: 2;
}

#import "Basic";
#import "String";
