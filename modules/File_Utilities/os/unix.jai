#scope_export

copy_file :: (source: string, dest: string) -> bool {
    data, success := read_entire_file(source);
    if !success {
        log_error("Unable to open file '%' for reading.\n", source);
        return false;
    }
    defer free(data);

    success = write_entire_file(dest, data);
    if !success {
        log_error("Unable to write file '%'.\n", dest);
        return false;
    }

    return success;
}

// Walks `dir_name`, calling `proc` for each entry. The walk is breadth-first
// over a queue rather than recursive, so a deep tree costs no stack.
visit_files :: (dir_name: string, recursive: bool, user_data: $T, proc: (info: *File_Visit_Info, user_data: T), visit_files := true, visit_directories := false, visit_symlinks := true, follow_directory_symlinks := true) -> bool {
    if !proc  return true;

    directories: [..] string;
    directories.allocator = temp;
    array_add(*directories, dir_name);

    info: File_Visit_Info = ---;

    cursor := 0;
    while cursor < directories.count {
        current := directories[cursor];
        cursor += 1;

        dirp := opendir(to_c_string(current, temp));
        if !dirp {
            // A subdirectory that cannot be opened is skipped; the one the
            // caller named is an error.
            if cursor != 1  continue;

            error_value, description := get_error_value_and_string();
            log_error("Unable to open directory '%': % %", current, error_value, description);
            return false;
        }
        defer closedir(dirp);

        entry := readdir(dirp);
        while entry != null {
            name := to_string(entry.d_name.data);

            full_name := join(current, "/", name,, temp);

            info.had_error = false;
            stats: stat_t;
            if stat(to_c_string(full_name, temp), *stats) == 0 {
                info.is_directory = S_ISDIR(stats.st_mode);
            } else {
                error_value := get_error_value();
                // ENOENT is a symlink pointing at nothing, or a file deleted
                // between the read and the stat; EACCES is a file this process
                // may not look at; ELOOP is a symlink cycle. All three are
                // reported to the visitor rather than failing the walk.
                if error_value != ENOENT && error_value != EACCES && error_value != ELOOP {
                    description := get_error_string(error_value);
                    log_error("Unable to stat file '%': % %", full_name, error_value, description);
                    return false;
                }

                info.had_error = true;
                info.is_directory = false;
            }

            info.short_name = name;
            info.full_name  = full_name;
            info.descend_into_directory = false;
            info.is_symlink = entry.d_type == .LNK;

            if info.is_directory {
                if name != "." && name != ".." {
                    info.descend_into_directory = recursive && (follow_directory_symlinks || !info.is_symlink);
                    if visit_directories && (visit_symlinks || !info.is_symlink) {
                        info.modification_time = timespec_to_apollo(stats.st_mtime);
                        proc(*info, user_data);
                    }
                }

                if info.descend_into_directory  array_add(*directories, full_name);
            } else {
                if visit_files && (visit_symlinks || !info.is_symlink) {
                    info.modification_time = timespec_to_apollo(stats.st_mtime);
                    proc(*info, user_data);
                }
            }

            entry = readdir(dirp);
        }
    }

    return true;
}

file_exists :: (path: string) -> bool {
    return access(to_c_string(path, temp), 0) == 0;
}

// Both together, because a modification time on its own is not much evidence:
// it can change without the contents changing, and change without being
// noticed.
file_modtime_and_size :: (file_path: string) -> (modtime: Apollo_Time, size: s64, file_exists: bool) {
    fs: stat_t;
    if stat(to_c_string(file_path, temp), *fs) != 0  return .{}, 0, false;

    return timespec_to_apollo(fs.st_mtime), fs.st_size, true;
}

// For saving state: write the new file somewhere else first, then call this,
// so a crash part way through cannot leave neither version behind.
atomic_file_replace_or_just_rename_if_destination_file_does_not_exist :: (old_filename: string, new_filename: string) -> bool {
    return rename(to_c_string(old_filename, temp), to_c_string(new_filename, temp)) == 0;
}

// Whether it is a directory, and whether the question could be answered at all
// — a path nobody may look at is not the same as one that is a file.
is_directory :: (path: string) -> (is_directory: bool, success: bool) {
    stats: stat_t;
    if stat(to_c_string(path, temp), *stats) != 0  return false, false;
    return S_ISDIR(stats.st_mode), true;
}

#scope_file

#import "POSIX";
#import "System";
