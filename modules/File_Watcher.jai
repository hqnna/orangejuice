//
// Watching directories for changes.
//
// `init` sets a watcher up with the callback that will hear about changes;
// `add_directories` starts watching, recursively unless told otherwise; and
// `process_changes` is called periodically to deliver whatever has happened.
//
// Changes are *merged*: a file written in several pieces produces one event
// rather than a dozen, because a change is held for `merge_window_seconds`
// after the last thing that happened to it. That is what
// `process_changes`'s `needs_wait`/`wait_seconds` are for — a program sleeping
// on the watcher's descriptor has to wake up again to deliver a change whose
// window is still open even though nothing new has arrived.
//
// inotify underneath, non-blocking, so `process_changes` returns at once when
// there is nothing to report.
//

#import "Basic";
#import "File";
#import "File_Utilities";
#import "Hash_Table";
#import "Pool";
#import "POSIX";
#import "String";

Linux :: #import "Linux";

File_Change :: struct {
    // Fully pathed.
    full_path: string;
    events: File_Change_Event;
    // From `seconds_since_init`.
    time_of_last_change := 0.0;
}

File_Change_Event :: enum_flags {
    ADDED;
    MODIFIED;

    // A rename produces two `MOVED` events, one for each path. `MOVED_FROM`
    // and `MOVED_TO` say which end this one is, where the system says.
    MOVED;
    MOVED_FROM;
    MOVED_TO;

    ATTRIBUTES_CHANGED;
    REMOVED;

    // The children of this directory may have changed without an event of
    // their own — a directory that appeared already full, say.
    SCAN_CHILDREN;

    ALL_EVENTS :: ADDED | MODIFIED | MOVED | ATTRIBUTES_CHANGED | REMOVED;
}

DEFAULT_BUFFER_SIZE :: 8000;

File_Watcher :: struct (User_Data: Type = void) {
    File_Watcher_Callback :: #type (watcher: *File_Watcher(User_Data), change: *File_Change, user_data: *User_Data);

    callback: File_Watcher_Callback;
    user_data: *User_Data;
    events_to_watch: File_Change_Event;
    merge_window_seconds: float;
    watch_recursively: bool;
    verbose: bool;
    buffer_size: s32;

    // Allocated rather than held inline: a watcher that is copied — into an
    // array, say — would otherwise leave `changes` pointing into the old
    // copy's pool.
    pool: *Pool;
    changes: [..] File_Change;

    inotify_instance: s32 = -1;
    inotify_data: *void;
    inotify_bytes_read: s64;
    inotify_cursor: s32;
    // A second pool, so the directory tree costs one allocation rather than
    // one per directory.
    path_pool: *Pool;
    watch_paths: Table(s32, string);
}

// `events_to_watch` says what is wanted; the system may report more.
// `event_buffer_size` is allocated once here, not once per directory.
init :: (watcher: *$T/File_Watcher, callback: T.File_Watcher_Callback, user_data: *T.User_Data = null, events_to_watch: File_Change_Event = .ALL_EVENTS, merge_window_seconds: float = 0.1, watch_recursively := true, verbose := false, event_buffer_size: s32 = DEFAULT_BUFFER_SIZE) -> (success: bool) {
    assert(merge_window_seconds >= 0);

    watcher.callback = callback;
    watcher.user_data = user_data;
    watcher.events_to_watch = events_to_watch;
    watcher.merge_window_seconds = merge_window_seconds;
    watcher.watch_recursively = watch_recursively;
    watcher.verbose = verbose;
    watcher.buffer_size = event_buffer_size;

    watcher.pool = New(Pool);
    set_allocators(watcher.pool);

    watcher.changes.allocator.proc = pool_allocator_proc;
    watcher.changes.allocator.data = watcher.pool;

    watcher.inotify_instance = Linux.inotify_init(Linux.IN_NONBLOCK);
    if watcher.inotify_instance == -1 {
        log_error("Could not init inotify: %", errno());
        free(watcher.pool);
        return false;
    }

    watcher.inotify_data = alloc(event_buffer_size);
    watcher.path_pool = New(Pool);
    set_allocators(watcher.path_pool);
    init(*watcher.watch_paths);

    return true;
}

// Starts watching. Each directory's subdirectories are watched too unless
// `watch_recursively` was false: inotify watches a directory, not a tree.
add_directories :: (using watcher: *$T/File_Watcher, dir_paths: ..string) -> bool {
    assert(callback != null, "You must call init() on the File_Watcher before add_directories()!");

    for dir_paths  if !add_single_directory(watcher, it)  return false;

    if !watcher.watch_recursively  return true;

    data: Walk_Data(T);
    data.watcher = watcher;
    data.success = true;

    for path: dir_paths {
        is_dir, success := is_directory(path);
        if !success {
            if watcher.verbose  log("Couldn’t check if \"%\" is a directory", path);
            continue;
        }
        if !is_dir  continue;

        walked := visit_files(path, true, *data, visit_one_directory, visit_files = false, visit_directories = true, visit_symlinks = false, follow_directory_symlinks = false);

        if !walked || !data.success {
            log_error("Couldn’t enumerate dirs in \"%\"", path);
            return false;
        }
    }

    return true;
}

deinit :: (using watcher: $T/File_Watcher) {
    if inotify_instance != -1  close(inotify_instance);
    free(inotify_data);
    deinit(*watch_paths);
    release(path_pool);
    free(path_pool);

    release(pool);
    free(pool);
}

// Delivers whatever has happened since the last call, through the callback.
//
// Calling this periodically is enough on its own. A program instead waiting on
// `inotify_instance` with `poll` has to look at `needs_wait`: when it is true
// there is a change whose merge window has not closed, and this must be called
// again after `wait_seconds` even if nothing else arrives.
process_changes :: (using watcher: *File_Watcher) -> (changed: bool, needs_wait: bool, wait_seconds: float64) {
    assert(callback != null, "You must call init() on the File_Watcher before process_changes()!");
    assert(inotify_instance != -1);

    inotify_bytes_read = read(inotify_instance, inotify_data, cast(u64) watcher.buffer_size);
    if inotify_bytes_read == -1 {
        // Nothing pending is the ordinary case for a non-blocking descriptor.
        if errno() != EAGAIN  log_error("Could not read inotify events: %", errno());
    } else if inotify_bytes_read >= 0 {
        inotify_cursor = 0;
        notify := cast(*Linux.inotify_event) inotify_data;
        while notify != null {
            defer notify = move_data_forward(watcher, notify);

            // `len` counts the padding after the name as well, so the name's
            // real length has to be measured rather than taken from it.
            name: string;
            name.data = notify.name.data;
            if notify.len  name.count = c_style_strlen(notify.name.data);
            else           name.count = 0;

            events: File_Change_Event;
            if notify.mask & Linux.IN_CREATE                                   events |= .ADDED;
            if notify.mask & (Linux.IN_MODIFY | Linux.IN_CLOSE_WRITE)          events |= .MODIFIED;
            if notify.mask & Linux.IN_MOVED_FROM                               events |= File_Change_Event.MOVED | .MOVED_FROM;
            if notify.mask & (Linux.IN_MOVED_TO | Linux.IN_MOVE_SELF)          events |= File_Change_Event.MOVED | .MOVED_TO;
            if notify.mask & Linux.IN_ATTRIB                                   events |= .ATTRIBUTES_CHANGED;
            if notify.mask & (Linux.IN_DELETE | Linux.IN_DELETE_SELF)          events |= .REMOVED;

            if !events {
                if watcher.verbose  log("discarded mask % for \"%\"", notify.mask, name);
                continue;
            }

            if watcher.verbose  log("Action '%' on name '%':", events, name);

            full_path, found := get_full_name(watcher, notify.wd, name);
            if !found {
                log_error("Could not assemble full name for \"%\"", name);
                continue;
            }

            was_created_or_moved := events & (File_Change_Event.ADDED | File_Change_Event.MOVED);
            if watch_recursively && was_created_or_moved && is_directory(full_path) {
                add_directories(watcher, full_path);
                // Whatever was already inside it never produced an event.
                events |= .SCAN_CHILDREN;
            }

            add_change(watcher, full_path, events);
        }
    }

    return emit_changes(watcher);
}

#scope_file

// What the recursive walk carries: the watcher, and whether every directory it
// reached could be watched.
Walk_Data :: struct (W: Type) {
    watcher: *W;
    success: bool;
}

visit_one_directory :: (info: *File_Visit_Info, data: *Walk_Data($W)) {
    if !add_single_directory(data.watcher, info.full_name)  data.success = false;
}

add_single_directory ::
 (using watcher: *File_Watcher, dir_path: string) -> bool {
    push_allocator(pool_allocator_proc, path_pool);
    c_path := to_c_string(dir_path, temp);

    flags: u32;
    if events_to_watch & .ADDED               flags |= Linux.IN_CREATE;
    if events_to_watch & .MODIFIED            flags |= Linux.IN_MODIFY | Linux.IN_CLOSE_WRITE;
    if events_to_watch & .MOVED               flags |= Linux.IN_MOVED_TO | Linux.IN_MOVE_SELF;
    if events_to_watch & .ATTRIBUTES_CHANGED  flags |= Linux.IN_ATTRIB;
    if events_to_watch & .REMOVED             flags |= Linux.IN_DELETE | Linux.IN_DELETE_SELF | Linux.IN_MOVED_FROM;

    watch := Linux.inotify_add_watch(inotify_instance, c_path, flags);
    if watch == -1 {
        log_error("Could not watch path \"%\": %", dir_path, errno());
        return false;
    }

    // A directory already watched gets the same descriptor back, so the path
    // is replaced rather than added again.
    previous := table_find_pointer(*watch_paths, watch);
    if previous {
        if watcher.verbose  log("Watch % for \"%\" already exists", watch, previous.*);
        if compare(previous.*, dir_path) != 0 {
            if watcher.verbose  log("Replacing path for watch %: \"%\" -> \"%\"", watch, previous.*, dir_path);
            free(previous.*);
            previous.* = copy_string(dir_path);
        }
    } else {
        if watcher.verbose  log("Adding watch % for \"%\"", watch, dir_path);
        table_add(*watch_paths, watch, copy_string(dir_path));
    }

    return true;
}

// The kernel packs the events back to back, each one as long as its name
// makes it.
move_data_forward :: (using watcher: *File_Watcher, event: *Linux.inotify_event) -> *Linux.inotify_event {
    // The binding's `name` is one byte where C has none, so the header is
    // that much shorter than the struct.
    INOTIFY_EVENT_OVERSIZE :: 4;
    step: s32 = cast(s32) (size_of(Linux.inotify_event) - INOTIFY_EVENT_OVERSIZE + event.len);
    if inotify_cursor + step >= inotify_bytes_read  return null;
    inotify_cursor += step;
    return cast(*Linux.inotify_event) ((cast(*u8) inotify_data) + inotify_cursor);
}

get_full_name :: (using watcher: *File_Watcher, watch: s32, name: string) -> (full_path: string, success: bool) {
    dir_path := table_find_pointer(*watch_paths, watch);
    if !dir_path  return "", false;

    if name  return tprint("%/%", dir_path.*, name), true;
    return dir_path.*, true;
}

// A change to a path already waiting is folded into it, and its window starts
// again — which is what turns a file written in pieces into one event.
add_change :: (using watcher: *File_Watcher, full_path: string, events: File_Change_Event) {
    push_allocator(pool_allocator_proc, pool);

    now := seconds_since_init();

    for * change: changes {
        if change.full_path == full_path {
            if watcher.verbose  log("Deduplicating changes for %: % + %", full_path, change.events, events);
            change.events |= events;
            change.time_of_last_change = xx now;
            return;
        }
    }

    change: File_Change;
    change.full_path = copy_string(full_path);
    change.events = events;
    change.time_of_last_change = xx now;

    array_add(*changes, change);

    if watcher.verbose  log("Change for '%' issued at time %.", full_path, now);
}

emit_changes :: (using watcher: *$T/File_Watcher) -> (changed: bool, needs_wait: bool, wait_seconds: float64) {
    assert(watcher.merge_window_seconds >= 0);
    if !changes.count  return false, false, 0;

    now := seconds_since_init();

    notified := false;
    earliest_wait_time: float64 = merge_window_seconds;
    for * change: changes {
        wait_time := merge_window_seconds - (now - change.time_of_last_change);

        if wait_time <= 0 {
            if callback  callback(watcher, change, user_data);
            remove change;
            notified = true;
        } else if wait_time < earliest_wait_time {
            earliest_wait_time = wait_time;
        }
    }

    // With nothing left waiting the pool can go back to the start. It could
    // grow without bound if changes never stopped arriving, but this is for a
    // development loop rather than for an adversary.
    if changes.count == 0 {
        array_reset(*changes);
        reset(pool);
    }

    return notified, changes.count > 0, earliest_wait_time;
}
