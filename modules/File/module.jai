//
// File: reading and writing whole files, and the handle for doing it piecemeal.
//
// Every procedure reports failure rather than asserting: a file not being
// there is an ordinary thing for a program to discover, not a bug in it.
//

#import "Basic";
#import "POSIX";

File :: struct {
    handle: s32 = -1;
}

File_Error :: enum {
    NONE          :: 0;
    NOT_FOUND     :: 1;
    PERMISSION    :: 2;
    ALREADY_EXISTS :: 3;
    IS_DIRECTORY  :: 4;
    OTHER         :: 5;
}

// ---------------------------------------------------------- whole files ---

// The whole file as one string the caller owns. `ok` is false when it could
// not be read, and the string is then empty.
read_entire_file :: (path: string, allocator := context.allocator) -> (contents: string, ok: bool) {
    empty: string;

    c_path := to_c_string(path, temp);
    if !c_path  return empty, false;

    fd := open(c_path, O_RDONLY);
    if fd < 0  return empty, false;
    defer close(fd);

    info: stat_t;
    if fstat(fd, *info) != 0  return empty, false;
    if (info.st_mode & S_IFMT) == S_IFDIR  return empty, false;

    if info.st_size <= 0 {
        // Zero-length, or something whose size the kernel does not know —
        // /proc entries report zero and still have contents.
        streamed, streamed_ok := read_until_end(fd, allocator);
        return streamed, streamed_ok;
    }

    contents: string;
    contents.data = cast(*u8) alloc(info.st_size, allocator);
    if !contents.data  return empty, false;

    got := 0;
    while got < info.st_size {
        n := read(fd, contents.data + got, cast(u64) (info.st_size - got));
        if n < 0 {
            if errno() == EINTR  continue;
            free(contents.data, allocator);
            return empty, false;
        }
        if n == 0  break;
        got += n;
    }
    contents.count = got;
    return contents, true;
}

write_entire_file :: (path: string, contents: string) -> bool {
    c_path := to_c_string(path, temp);
    if !c_path  return false;

    fd := open(c_path, O_WRONLY | O_CREAT | O_TRUNC, 0x1A4)  ;
    if fd < 0  return false;
    defer close(fd);

    written := 0;
    while written < contents.count {
        n := write(fd, contents.data + written, cast(u64) (contents.count - written));
        if n < 0 {
            if errno() == EINTR  continue;
            return false;
        }
        written += n;
    }
    return true;
}

append_to_file :: (path: string, contents: string) -> bool {
    c_path := to_c_string(path, temp);
    if !c_path  return false;

    fd := open(c_path, O_WRONLY | O_CREAT | O_APPEND, 0x1A4)  ;
    if fd < 0  return false;
    defer close(fd);

    written := 0;
    while written < contents.count {
        n := write(fd, contents.data + written, cast(u64) (contents.count - written));
        if n < 0 {
            if errno() == EINTR  continue;
            return false;
        }
        written += n;
    }
    return true;
}

// ------------------------------------------------------------- handles ---

file_open :: (path: string, for_writing := false, keep_existing_content := false) -> (file: File, ok: bool) {
    result: File;

    c_path := to_c_string(path, temp);
    if !c_path  return result, false;

    flags: s32 = O_RDONLY;
    if for_writing {
        flags = O_WRONLY | O_CREAT;
        if !keep_existing_content  flags |= O_TRUNC;
    }

    fd := open(c_path, flags, 0x1A4)  ;
    if fd < 0  return result, false;
    result.handle = fd;
    return result, true;
}

file_close :: (file: *File) {
    if file.handle >= 0  close(file.handle);
    file.handle = -1;
}

// How many bytes were read, which is fewer than asked for at the end of the
// file. Zero means there is nothing more.
file_read :: (file: File, buffer: *void, count: s64) -> (read_count: s64, ok: bool) {
    if file.handle < 0  return 0, false;
    got := 0;
    while got < count {
        n := read(file.handle, cast(*u8) buffer + got, cast(u64) (count - got));
        if n < 0 {
            if errno() == EINTR  continue;
            return got, false;
        }
        if n == 0  break;
        got += n;
    }
    return got, true;
}

file_write :: (file: File, buffer: *void, count: s64) -> bool {
    if file.handle < 0  return false;
    written := 0;
    while written < count {
        n := write(file.handle, cast(*u8) buffer + written, cast(u64) (count - written));
        if n < 0 {
            if errno() == EINTR  continue;
            return false;
        }
        written += n;
    }
    return true;
}

file_write :: (file: File, s: string) -> bool {
    return file_write(file, s.data, s.count);
}

file_length :: (file: File) -> (length: s64, ok: bool) {
    if file.handle < 0  return 0, false;
    info: stat_t;
    if fstat(file.handle, *info) != 0  return 0, false;
    return info.st_size, true;
}

file_seek :: (file: File, offset: s64, whence := SEEK_SET) -> (position: s64, ok: bool) {
    if file.handle < 0  return 0, false;
    where := lseek(file.handle, offset, whence);
    if where < 0  return 0, false;
    return where, true;
}

// --------------------------------------------------------------- facts ---

file_exists :: (path: string) -> bool {
    c_path := to_c_string(path, temp);
    if !c_path  return false;
    info: stat_t;
    return stat(c_path, *info) == 0;
}

is_directory :: (path: string) -> bool {
    c_path := to_c_string(path, temp);
    if !c_path  return false;
    info: stat_t;
    if stat(c_path, *info) != 0  return false;
    return (info.st_mode & S_IFMT) == S_IFDIR;
}

is_regular_file :: (path: string) -> bool {
    c_path := to_c_string(path, temp);
    if !c_path  return false;
    info: stat_t;
    if stat(c_path, *info) != 0  return false;
    return (info.st_mode & S_IFMT) == S_IFREG;
}

file_size :: (path: string) -> (size: s64, ok: bool) {
    c_path := to_c_string(path, temp);
    if !c_path  return 0, false;
    info: stat_t;
    if stat(c_path, *info) != 0  return 0, false;
    return info.st_size, true;
}

// Seconds since the epoch.
file_modification_time :: (path: string) -> (time: s64, ok: bool) {
    c_path := to_c_string(path, temp);
    if !c_path  return 0, false;
    info: stat_t;
    if stat(c_path, *info) != 0  return 0, false;
    return info.st_mtime, true;
}

// ------------------------------------------------------------- changing ---

file_delete :: (path: string) -> bool {
    c_path := to_c_string(path, temp);
    if !c_path  return false;
    return unlink(c_path) == 0;
}

file_move :: (from: string, to: string) -> bool {
    c_from := to_c_string(from, temp);
    c_to := to_c_string(to, temp);
    if !c_from || !c_to  return false;
    return rename(c_from, c_to) == 0;
}

make_directory_if_it_does_not_exist :: (path: string, recursive := false) -> bool {
    if is_directory(path)  return true;

    if recursive {
        // Make each prefix in turn. The root is skipped: a leading separator
        // names a directory that already exists.
        at := 0;
        while at < path.count {
            if path[at] == #char "/" && at > 0 {
                prefix := slice(path, 0, at);
                if !is_directory(prefix)  make_one_directory(prefix);
            }
            at += 1;
        }
    }
    return make_one_directory(path);
}

delete_directory :: (path: string) -> bool {
    c_path := to_c_string(path, temp);
    if !c_path  return false;
    return rmdir(c_path) == 0;
}

#scope_module

make_one_directory :: (path: string) -> bool {
    c_path := to_c_string(path, temp);
    if !c_path  return false;
    if mkdir(c_path, 0x1ED) == 0  return true;
    // Something else may have made it in the meantime, which is success as
    // far as the caller is concerned.
    return errno() == EEXIST;
}

// For a file whose size the kernel does not report — a pipe, or something
// under /proc — read until it stops giving anything.
read_until_end :: (fd: s32, allocator: Allocator) -> (contents: string, ok: bool) {
    builder: String_Builder;
    init_string_builder(*builder, temp);

    buffer: [4096] u8;
    while true {
        n := read(fd, buffer.data, cast(u64) buffer.count);
        if n < 0 {
            if errno() == EINTR  continue;
            empty: string;
            return empty, false;
        }
        if n == 0  break;
        piece: string;
        piece.data = buffer.data;
        piece.count = n;
        append(*builder, piece);
    }
    return builder_to_string(*builder, allocator), true;
}
