//
// Walking the file tree, and the operations that work on whole directories.
//
// `visit_files` is the one worth knowing: it walks a directory, optionally
// into subdirectories, and calls back for each file it finds. Everything else
// here is built on it or on `File`.
//

#import "Basic";
#import "File";
#import "POSIX";
#import "String";

File_Visit_Info :: struct {
    full_name:  string;
    short_name: string;
    // Set by the callback to stop the walk early.
    had_error := false;
}

// Calls `proc` for each entry. `recursive` descends into subdirectories;
// `visit_directories` decides whether the directories themselves are reported
// as well as walked into.
//
// `.` and `..` are never reported: every caller would have to skip them.
// The parameter names are the reference's, `visit_files` among them — which
// shadows this procedure's own name, so the walking is done by the helper
// below rather than by recursing here.
visit_files :: (dir_name: string, recursive: bool, user_data: $T, proc: (info: *File_Visit_Info, user_data: T), visit_files := true, visit_directories := false) -> bool {
    return walk_directory(dir_name, recursive, user_data, proc, visit_files, visit_directories);
}

#scope_module

walk_directory :: (directory: string, recursive: bool, user_data: $T, proc: (info: *File_Visit_Info, user_data: T), want_files: bool, want_directories: bool) -> bool {
    c_path := to_c_string(directory, temp);
    if !c_path  return false;

    dir := opendir(c_path);
    if !dir  return false;
    defer closedir(dir);

    while true {
        entry := readdir(dir);
        if !entry  break;

        name := to_string(entry.d_name.data);
        if name == "." || name == ".."  continue;

        full := path_join(directory, name, allocator = temp);
        is_dir := entry.d_type == DT_DIR;
        // A filesystem that does not fill `d_type` says UNKNOWN, so the fact
        // has to be asked for rather than assumed.
        if entry.d_type == DT_UNKNOWN  is_dir = is_directory(full);

        if is_dir {
            if want_directories {
                info: File_Visit_Info;
                info.full_name = full;
                info.short_name = name;
                proc(*info, user_data);
                if info.had_error  return false;
            }
            if recursive {
                if !walk_directory(full, recursive, user_data, proc, want_files, want_directories) {
                    return false;
                }
            }
        } else if want_files {
            info: File_Visit_Info;
            info.full_name = full;
            info.short_name = name;
            proc(*info, user_data);
            if info.had_error  return false;
        }
    }
    return true;
}

#scope_export

// Every file under `directory`, as paths the caller owns.
file_list :: (directory: string, recursive := false, allocator := context.allocator) -> [] string {
    found: [..] string;
    found.allocator = allocator;

    collect :: (info: *File_Visit_Info, found: *[..] string) {
        array_add(found, copy_string(info.full_name, found.allocator));
    }
    visit_files(directory, recursive, *found, collect);
    return found;
}

copy_file :: (from: string, to: string) -> bool {
    contents, ok := read_entire_file(from, temp);
    if !ok  return false;
    return write_entire_file(to, contents);
}

// Removes a directory and everything under it. Reports false if anything
// could not be removed, having removed what it could.
delete_directory_recursively :: (directory: string) -> bool {
    if !is_directory(directory)  return false;

    c_path := to_c_string(directory, temp);
    if !c_path  return false;
    dir := opendir(c_path);
    if !dir  return false;

    ok := true;
    while true {
        entry := readdir(dir);
        if !entry  break;
        name := to_string(entry.d_name.data);
        if name == "." || name == ".."  continue;

        full := path_join(directory, name, allocator = temp);
        if is_directory(full) {
            if !delete_directory_recursively(full)  ok = false;
        } else {
            if !file_delete(full)  ok = false;
        }
    }
    closedir(dir);

    if !delete_directory(directory)  ok = false;
    return ok;
}

// ------------------------------------------------------------- the paths ---

// The path with `.` and `..` resolved and symlinks followed. Fails for a path
// that does not exist, which is what `realpath` promises.
get_absolute_path :: (path: string, allocator := context.allocator) -> (result: string, ok: bool) {
    empty: string;
    c_path := to_c_string(path, temp);
    if !c_path  return empty, false;

    // PATH_MAX on Linux.
    buffer: [4096] u8;
    if !realpath(c_path, buffer.data)  return empty, false;
    return copy_string(to_string(buffer.data), allocator), true;
}

get_working_directory :: (allocator := context.allocator) -> string {
    buffer: [4096] u8;
    if !getcwd(buffer.data, cast(u64) buffer.count)  return "";
    return copy_string(to_string(buffer.data), allocator);
}

set_working_directory :: (path: string) -> bool {
    c_path := to_c_string(path, temp);
    if !c_path  return false;
    return chdir(c_path) == 0;
}

// Where this program's own executable lives, read out of /proc.
get_path_of_running_executable :: (allocator := context.allocator) -> string {
    buffer: [4096] u8;
    c_link := to_c_string("/proc/self/exe", temp);
    if !c_link  return "";
    n := readlink(c_link, buffer.data, cast(u64) buffer.count);
    if n <= 0  return "";
    return copy_string(to_string(buffer.data, n), allocator);
}
