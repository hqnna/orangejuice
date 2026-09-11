//
// System: what the program can ask about the machine it is running on.
//

#import "Basic";
#import "POSIX";
#import "String";

// The environment variable, and whether it was set at all — which is not the
// same as being set to nothing.
get_environment_variable :: (name: string, allocator := context.allocator) -> (value: string, found: bool) {
    empty: string;
    c_name := to_c_string(name, temp);
    if !c_name  return empty, false;

    raw := getenv(c_name);
    if !raw  return empty, false;
    return copy_string(to_string(raw), allocator), true;
}

// The directories `PATH` names, in order.
get_path_directories :: (allocator := context.allocator) -> [] string {
    value, found := get_environment_variable("PATH", temp);
    if !found  return .[];
    return split(value, ":", allocator);
}

// Where an executable named `name` would be found on `PATH`, or "".
find_executable :: (name: string, allocator := context.allocator) -> string {
    // A name with a separator in it is a path already, not something to look
    // up.
    if find_index_from_left(name, #char "/") >= 0 {
        if file_exists_and_is_executable(name)  return copy_string(name, allocator);
        return "";
    }

    for get_path_directories(temp) {
        candidate := tprint("%/%", it, name);
        if file_exists_and_is_executable(candidate)  return copy_string(candidate, allocator);
    }
    return "";
}

// Who the process is running as, in temporary storage.
get_username :: () -> (name: string, success: bool) {
    success, entry := get_passwd_for_user();
    if !success  return "", false;
    return to_string(entry.pw_name), true;
}

// The password-database entry for a user, or for the one this process is
// running as. The strings in it point into a temporary buffer.
get_passwd_for_user :: (username := "") -> (success: bool, p: passwd) {
    buffer_size := sysconf(_SC_GETPW_R_SIZE_MAX);
    // The system would not say; a kilobyte is enough for almost every entry,
    // and the loop below grows it when it is not.
    if buffer_size == -1  buffer_size = 1024;

    while true {
        buffer := cast(*u8) alloc(buffer_size,, temp);

        found: *passwd;
        entry: passwd;
        result: s32;
        if username {
            result = getpwnam_r(to_c_string(username, temp), *entry, buffer, cast(u64) buffer_size, *found);
        } else {
            result = getpwuid_r(getuid(), *entry, buffer, cast(u64) buffer_size, *found);
        }

        if result != 0 {
            error_code := errno();
            // Not unbounded: an entry this big is a broken database rather
            // than a long one.
            MAX_BUFFER_SIZE :: 16384;
            if error_code == ERANGE && buffer_size < MAX_BUFFER_SIZE {
                buffer_size *= 2;
                continue;
            }

            // Some systems say "no such user" this way and some by leaving
            // `found` null, which is why both are checked.
            if error_code == ENOENT  return false, .{};

            description := get_error_string(error_code);
            if username  log_error("Could not get passwd details for user \"%\": % %", username, error_code, description);
            else         log_error("Could not get passwd details for current user: % %", error_code, description);
        }

        if !found  return false, entry;
        return true, entry;
    }
    return false, .{};
}

// The user's home directory. `$HOME` wins for the current user, since that is
// what a shell lets someone override it with.
get_home_directory :: (username := "") -> (home: string, success: bool) {
    if !username {
        result := getenv("HOME");
        if result  return to_string(result), true;
    }

    success, entry := get_passwd_for_user(username);
    if !success  return "", false;

    return to_string(entry.pw_dir), true;
}

// The machine's name, in temporary storage.
get_machinename :: () -> string {
    hostname: [1024] u8;
    if gethostname(hostname.data, cast(u64) hostname.count) < 0  return "unknown";
    return copy_temporary_string(to_string(hostname.data));
}

// Where this executable was loaded from, in temporary storage.
get_path_of_running_executable :: () -> string {
    buffer: [4096] u8;

    length := readlink("/proc/self/exe", buffer.data, cast(u64) buffer.count);
    if length < 0  return "";

    result: string = ---;
    result.data  = alloc(length,, temp);
    result.count = length;
    memcpy(result.data, buffer.data, length);
    return result;
}

// Which count of processors is wanted. Linux has no way to tell a performance
// core from an efficiency one, so the performance queries answer as the
// corresponding `ALL_` one does.
Processor_Query_Type :: enum u8 {
    ALL_PHYSICAL;
    ALL_LOGICAL;
    PERFORMANCE_PHYSICAL;
    PERFORMANCE_LOGICAL;
}

get_number_of_processors :: ($type: Processor_Query_Type = .PERFORMANCE_LOGICAL) -> (count: s32) #modify {
    // Folding the two pairs together means one instantiation rather than two
    // identical ones.
    if type == .PERFORMANCE_PHYSICAL  type = .ALL_PHYSICAL;
    if type == .PERFORMANCE_LOGICAL   type = .ALL_LOGICAL;
    return true;
} {
    #if type == .ALL_LOGICAL {
        return get_nprocs();
    } else {
        // Physical cores have to be counted out of `/proc/cpuinfo`: one entry
        // per logical processor, with a `core id` that repeats across the
        // hyperthreads of one core.
        text, ok := read_special_file("/proc/cpuinfo");
        if !ok || !text.count  return get_number_of_processors(.ALL_LOGICAL);
        defer free(text.data);

        count: s32 = 0;
        last: string;
        cpu_info := text;
        while cpu_info.count {
            line_end := find_index_from_left(cpu_info, #char "\n");

            if starts_with(cpu_info, "core id") {
                id := cpu_info;
                if line_end != -1  id.count = line_end;
                last_space := find_index_from_right(id, #char " ");
                if last_space != -1 {
                    id.data  += last_space;
                    id.count -= last_space;
                }
                if id != last {
                    count += 1;
                    last = id;
                }
            }

            if line_end == -1  break;
            cpu_info.count -= line_end + 1;
            cpu_info.data  += line_end + 1;
        }

        if !count  return get_number_of_processors(.ALL_LOGICAL);
        return count;
    }
}

// What the last failing call set. `errno` is thread-local, so this is about
// the calling thread and nothing else.
get_error_value :: errno;

// The system's description of an error code, in temporary storage.
get_error_string :: (error_code: OS_Error_Code) -> string {
    buffer: [1024] u8;

    result := strerror_r(cast(s32) error_code, buffer.data, cast(s32) buffer.count);
    if !result {
        // glibc's `strerror_r` sets `errno` itself when it fails, so the code
        // reported here is that one rather than the one asked about.
        return sprint("OS did not return us a valid string for error %. Returned error was %", error_code, errno());
    }

    return copy_temporary_string(to_string(result));
}

get_error_value_and_string :: () -> (error_code: OS_Error_Code, description: string) {
    error_code := errno();
    return error_code, get_error_string(error_code);
}

get_process_id :: () -> s32 {
    return getpid();
}

// Seconds and nanoseconds since the epoch.
get_time :: () -> (seconds: s64, nanoseconds: s64) {
    now: timespec;
    if clock_gettime(.REALTIME, *now) != 0  return 0, 0;
    return now.tv_sec, now.tv_nsec;
}

// Seconds since an unspecified point, which is the one to measure elapsed
// time with: unlike the wall clock it never steps backwards.
get_monotonic_time :: () -> float64 {
    now: timespec;
    if clock_gettime(.MONOTONIC, *now) != 0  return 0;
    return cast(float64) now.tv_sec + cast(float64) now.tv_nsec * 1.0e-9;
}

sleep_milliseconds :: (milliseconds: s64) {
    if milliseconds <= 0  return;
    request: timespec;
    request.tv_sec = milliseconds / 1000;
    request.tv_nsec = (milliseconds % 1000) * 1_000_000;
    remaining: timespec;
    // A signal can cut the sleep short, and then what is left is slept again.
    while nanosleep(*request, *remaining) != 0 {
        if errno() != EINTR  break;
        request = remaining;
    }
}

#scope_module

// `stat` says whether it exists; the execute bits say whether it could be
// run. Checking the owner/group/other bits together is close enough — the
// caller finds out for certain by trying.
EXECUTABLE_BITS :: 0x49;   // 0o111

file_exists_and_is_executable :: (path: string) -> bool {
    c_path := to_c_string(path, temp);
    if !c_path  return false;
    info: stat_t;
    if stat(c_path, *info) != 0  return false;
    if (info.st_mode & S_IFMT) != S_IFREG  return false;
    return (info.st_mode & EXECUTABLE_BITS) != 0;
}

// A file in `/proc` cannot be sized in advance: it is generated as it is read,
// so `fstat` says zero and seeking does not work. The only way to know how big
// it is is to read until it stops.
read_special_file :: (path: string) -> (contents: string, ok: bool) {
    descriptor := open(to_c_string(path, temp), O_RDONLY | O_CLOEXEC);
    if descriptor == -1  return "", false;
    defer close(descriptor);

    builder: [..] u8;
    BLOCK_SIZE :: 4096;
    block: [BLOCK_SIZE] u8;
    while true {
        moved := read(descriptor, block.data, BLOCK_SIZE);
        if moved < 0 {
            array_free(builder);
            return "", false;
        }
        if moved == 0  break;

        piece: [] u8;
        piece.data  = block.data;
        piece.count = moved;
        array_add(*builder, ..piece);
    }

    result: string = ---;
    result.data  = builder.data;
    result.count = builder.count;
    return result, true;
}

#scope_module

libc_nprocs :: #system_library "c";

get_nprocs :: () -> s32 #foreign libc_nprocs;
strerror_r :: (errnum: s32, buffer: *u8, length: s32) -> *u8 #foreign libc_nprocs;

#scope_export
