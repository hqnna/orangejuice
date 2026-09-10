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
    short_name: string;
    full_name:  string;

    modification_time: Apollo_Time;

    // The walker could not find out everything about this entry — a directory
    // it may not read, most often. The names are still valid.
    had_error:    bool;
    is_symlink:   bool;
    is_directory: bool;

    // The callback clears this to keep the walk out of a directory it has
    // decided it does not care about.
    descend_into_directory := true;
}

// Calls `proc` for each entry. `recursive` descends into subdirectories;
// `visit_directories` decides whether the directories themselves are reported
// as well as walked into.
//
// `.` and `..` are never reported: every caller would have to skip them.
// The parameter names are the reference's, `visit_files` among them — which
// shadows this procedure's own name, so the walking is done by the helper
// below rather than by recursing here.
visit_files :: (dir_name: string, recursive: bool, user_data: $T, proc: (info: *File_Visit_Info, user_data: T), visit_files := true, visit_directories := false, visit_symlinks := true, follow_directory_symlinks := true) -> bool {
    walk: Walk_Options;
    walk.recursive = recursive;
    walk.want_files = visit_files;
    walk.want_directories = visit_directories;
    walk.want_symlinks = visit_symlinks;
    walk.follow_directory_symlinks = follow_directory_symlinks;
    return walk_directory(dir_name, user_data, proc, walk);
}

#scope_module

// Bundled so that recursing does not mean threading six parameters through
// every level.
Walk_Options :: struct {
    recursive:                 bool;
    want_files:                bool;
    want_directories:          bool;
    want_symlinks:             bool;
    follow_directory_symlinks: bool;
}

walk_directory :: (directory: string, user_data: $T, proc: (info: *File_Visit_Info, user_data: T), walk: Walk_Options) -> bool {
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

        info: File_Visit_Info;
        info.short_name = name;
        info.full_name = full;
        info.is_symlink = entry.d_type == DT_LNK;

        // A filesystem that does not fill `d_type` says UNKNOWN, and a symlink
        // says what it is rather than what it points at — so either way the
        // fact has to be asked for rather than assumed.
        if entry.d_type == DT_DIR {
            info.is_directory = true;
        } else if info.is_symlink || entry.d_type == DT_UNKNOWN {
            info.is_directory = is_directory(full);
        }

        time, time_ok := file_modification_time(full);
        if time_ok  info.modification_time = seconds_to_apollo(time) + APOLLO_TIME_FOR_JAN_1_1970;
        else        info.had_error = true;

        if info.is_symlink && !walk.want_symlinks  continue;

        if info.is_directory {
            if walk.want_directories {
                proc(*info, user_data);
            }
            if !walk.recursive                                     continue;
            if !info.descend_into_directory                        continue;
            if info.is_symlink && !walk.follow_directory_symlinks  continue;
            if !walk_directory(full, user_data, proc, walk)        return false;
        } else if walk.want_files {
            proc(*info, user_data);
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
    push_allocator(temp);
    contents, ok := read_entire_file(from);
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

// Whether the path names a directory, and whether the question could be
// answered at all — a path nobody may look at is not the same as one that
// exists and is a file.
is_directory :: (path: string) -> (is_directory: bool, success: bool) {
    c_path := to_c_string(path, temp);
    if !c_path  return false, false;
    info: stat_t;
    if stat(c_path, *info) != 0  return false, false;
    return (info.st_mode & S_IFMT) == S_IFDIR, true;
}
