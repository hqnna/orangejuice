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

// Who the process is running as. `USER` is what a shell sets; a process
// started without one falls back to asking the password database.
get_username :: (allocator := context.allocator) -> (name: string, success: bool) {
    value, found := get_environment_variable("USER", allocator);
    if found && value.count  return value, true;

    raw := getpwuid_name(geteuid());
    if !raw  return "", false;
    return copy_string(to_string(raw), allocator), true;
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

// glibc's password entry. Only the name is read, so the rest of the struct is
// padding as far as this is concerned.
passwd :: struct {
    pw_name:   *u8;
    pw_passwd: *u8;
    pw_uid:    u32;
    pw_gid:    u32;
    pw_gecos:  *u8;
    pw_dir:    *u8;
    pw_shell:  *u8;
}

geteuid :: () -> u32 #foreign libc_system;
getpwuid :: (uid: u32) -> *passwd #foreign libc_system;

libc_system :: #system_library "c";

getpwuid_name :: (uid: u32) -> *u8 {
    entry := getpwuid(uid);
    if !entry  return null;
    return entry.pw_name;
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
