//
// The POSIX half. A `File` is a C stream rather than a descriptor, which is
// what gives it buffering and what makes `file_read` able to tell the end of a
// file apart from an error.
//

#import "POSIX";

File :: struct {
    handle: *FILE;
}

file_open :: (name: string, for_writing := false, keep_existing_content := false, log_errors := true) -> (file: File, success: bool) {
    c_name := to_c_string(name, temp);

    handle: *FILE;
    if for_writing && keep_existing_content  handle = fopen(c_name, "a+");
    else if for_writing                      handle = fopen(c_name, "wb+");
    else                                     handle = fopen(c_name, "rb");

    file: File;
    file.handle = handle;

    if !handle {
        if log_errors {
            error_code, description := System.get_error_value_and_string();
            log_error("Could not open file \"%\": code %, %", name, error_code, description);
        }
        return file, false;
    }

    return file, true;
}

// Moves a file or a whole directory.
file_move :: (name_old: string, name_new: string) -> bool {
    return rename(to_c_string(name_old, temp), to_c_string(name_new, temp)) == 0;
}

file_delete :: (filename: string) -> bool {
    return _remove(to_c_string(filename, temp)) == 0;
}

// A hard link at `linkpath` to `filepath`. Whatever was at `linkpath` is
// removed first.
file_link :: (linkpath: string, filepath: string) -> bool {
    c_link := to_c_string(linkpath, temp);
    unlink(c_link);
    return link(to_c_string(filepath, temp), c_link) == 0;
}

file_close :: (file: *File) {
    assert(file.handle != null);
    if file.handle  fclose(file.handle);
}

is_valid :: (file: File) -> bool {
    return file.handle != null;
}

// At most `bytes_to_read` bytes. The end of the file is `(true, how_much)`,
// not a failure; a real error is `(false, how_much)`.
file_read :: (f: File, vdata: *void, bytes_to_read: s64) -> (success: bool, total_read: s64) {
    data := cast(*u8) vdata;
    if bytes_to_read <= 0  return false, 0;

    if data == null {
        log_error("Attempt to file_read() into a null destination pointer.\n");
        return false, 0;
    }

    read_length: s64 = xx fread(data, 1, xx bytes_to_read, f.handle);
    if read_length < bytes_to_read {
        // `fread` reports a short read the same way for both, so the stream
        // has to be asked which it was.
        if feof(f.handle)  return true, read_length;
        return false, read_length;
    }

    return true, bytes_to_read;
}

file_length :: (file: File) -> (length: s64, success: bool) {
    position_current := ftello64(file.handle);
    if position_current == -1 || position_current == DUMB_LINUX_FAILURE_NUMBER  return 0, false;

    fseek(file.handle, 0, SEEK_END);
    length := ftello64(file.handle);
    if length == -1 || length == DUMB_LINUX_FAILURE_NUMBER  return 0, false;

    fseek(file.handle, position_current, SEEK_SET);
    return length, true;
}

file_current_position :: (f: File) -> (position: s64, success: bool) {
    result := ftello64(f.handle);
    return result, result != -1 && result != DUMB_LINUX_FAILURE_NUMBER;
}

file_set_position :: (f: File, pos: s64) -> bool {
    return fseek(f.handle, pos, SEEK_SET) == 0;
}

read_entire_file :: (f: File, zero_terminated := false) -> (contents: string, success: bool) {
    length, success := file_length(f);
    if !success  return "", false;

    termination := cast(s32) zero_terminated;
    data := cast(*u8) alloc(length + termination);
    if data == null  return "", false;

    read_length := fread(data, 1, cast(u64) length, f.handle);
    // A short read may be the end of the file rather than an error — reading a
    // Linux HID descriptor is like that — and `fread` does not distinguish.
    if read_length != cast(u64) length && !feof(f.handle) {
        free(data);
        return "", false;
    }

    if zero_terminated  data[read_length] = 0;

    result: string = ---;
    result.data  = data;
    result.count = cast(s64) read_length;
    return result, true;
}

file_write :: (file: *File, data: *void, length: s64) -> bool {
    assert(length >= 0);
    return fwrite(data, 1, cast(u64) length, file.handle) == cast(u64) length;
}

// Removes a directory and everything in it.
delete_directory :: (dirname: string) -> bool {
    Util :: #import "File_Utilities";

    success := true;
    directory_visitor :: (info: *Util.File_Visit_Info, success_pointer: *bool) {
        if info.is_directory && !info.is_symlink {
            // Emptied first, since a directory with anything in it cannot be
            // removed.
            Util.visit_files(info.full_name, recursive = false, success_pointer, directory_visitor, visit_files = true, visit_directories = true);
        }

        if _remove(to_c_string(info.full_name, temp)) != 0 {
            error_code, description := System.get_error_value_and_string();
            log_error("Error while trying to delete \"%\": % %", info.full_name, error_code, description);
            success_pointer.* = false;
        }
    }

    Util.visit_files(dirname, recursive = false, *success, directory_visitor, visit_files = true, visit_directories = true);
    if _remove(to_c_string(dirname, temp)) != 0 {
        error_code, description := System.get_error_value_and_string();
        log_error("Error while trying to delete \"%\": % %", dirname, error_code, description);
        success = false;
    }

    return success;
}

file_seek :: (file: File, pos: s64, type: Seek_Type) -> (success: bool) {
    whence: s32;
    if type == {
        case .START;   whence = SEEK_SET;
        case .CURRENT; whence = SEEK_CUR;
        case .END;     whence = SEEK_END;
        case;          return false;
    }

    return fseek(file.handle, pos, whence) == 0;
}

file_tell :: (file: File) -> (success: bool, position: s64) {
    position := ftello64(file.handle);
    return position != -1 && position != DUMB_LINUX_FAILURE_NUMBER, position;
}

// `ftell` on Linux answers with this rather than -1 for some things — a
// directory, for one — so it has to be checked for as well.
DUMB_LINUX_FAILURE_NUMBER :: 0x7fff_ffff_ffff_ffff;

#scope_module

os_make_directory_if_it_does_not_exist :: (name: string) -> (directory_now_exists: bool) {
    RWX_ALL :: 0x1FF;  // 0o777
    return make_directory_helper(name, RWX_ALL);
}

make_directory_helper :: (dir_name: string, attribs: u32) -> (directory_now_exists: bool) {
    if mkdir(to_c_string(dir_name, temp), attribs) == 0  return true;
    // Already there is what was wanted.
    if errno() == EEXIST  return true;
    return false;
}

// Fails when the path does not exist: `realpath` resolves symlinks, which
// means looking at what is really there.
get_absolute_path_unix :: (relative_path: string) -> (absolute_path: string, success: bool) {
    PATH_MAX :: 4096;
    buffer: [PATH_MAX] u8;

    result := realpath(to_c_string(relative_path, temp), buffer.data);
    if !result  return "", false;

    return copy_temporary_string(to_string(result)), true;
}

#scope_file

System :: #import "System";
