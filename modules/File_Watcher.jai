//
// Watching files and directories for changes.
//
// inotify rather than polling: the kernel reports what changed, so a watcher
// costs nothing while nothing is happening. The descriptor is non-blocking, so
// `process_changes` returns immediately when there is nothing to report and a
// program can call it from its own loop.
//

#import "Basic";
#import "POSIX";
#import "String";
#import "Hash_Table";

Change_Kind :: enum_flags u32 {
    NONE     :: 0;
    MODIFIED :: 0x1;
    CREATED  :: 0x2;
    DELETED  :: 0x4;
    MOVED    :: 0x8;
    ATTRIBUTES :: 0x10;
}

Change :: struct {
    // The directory that was being watched, joined with the name the kernel
    // reported — so a caller gets a usable path rather than a bare filename.
    path: string;
    kind: Change_Kind;
}

File_Watcher :: struct {
    handle: s32 = -1;
    // Which directory each watch descriptor is for, so a reported name can be
    // turned back into a path.
    watched: Table(s32, string);
    allocator: Allocator;
}

init :: (watcher: *File_Watcher, allocator := context.allocator) -> bool {
    watcher.allocator = allocator;
    init(*watcher.watched, allocator = allocator);

    handle := inotify_init1(IN_NONBLOCK);
    if handle < 0  return false;
    watcher.handle = handle;
    return true;
}

deinit :: (watcher: *File_Watcher) {
    if watcher.handle >= 0  close(watcher.handle);
    watcher.handle = -1;
    for watcher.watched  free(it.data, watcher.allocator);
    deinit(*watcher.watched);
}

// Watches one directory. Not recursive: inotify watches a directory, not a
// tree, so a caller wanting subdirectories adds each of them.
add_directory :: (watcher: *File_Watcher, path: string, kinds := Change_Kind.MODIFIED | .CREATED | .DELETED | .MOVED) -> bool {
    if watcher.handle < 0  return false;

    c_path := to_c_string(path, temp);
    if !c_path  return false;

    watch := inotify_add_watch(watcher.handle, c_path, to_inotify_mask(kinds));
    if watch < 0  return false;

    // Replacing a watch for a path the kernel already knew gives the same
    // descriptor back, so the old copy is released first.
    existing, found := table_find(*watcher.watched, watch);
    if found  free(existing.data, watcher.allocator);

    table_set(*watcher.watched, watch, copy_string(path, watcher.allocator));
    return true;
}

// Everything that has happened since the last call, in temporary storage.
// Empty when nothing has.
process_changes :: (watcher: *File_Watcher) -> [] Change {
    changes: [..] Change;
    changes.allocator = temp;
    if watcher.handle < 0  return changes;

    // The kernel packs events back to back, each followed by its name.
    buffer: [4096] u8;
    while true {
        got := read(watcher.handle, buffer.data, cast(u64) buffer.count);
        if got <= 0 {
            // EAGAIN simply means there is nothing waiting.
            break;
        }

        at := 0;
        while at + size_of(inotify_event) <= got {
            event := cast(*inotify_event) (buffer.data + at);
            at += size_of(inotify_event);

            name: string;
            if event.len {
                // The name is zero-padded to a multiple of the event size.
                name = to_string(buffer.data + at);
                at += cast(s64) event.len;
            }

            directory, found := table_find(*watcher.watched, event.wd);
            if !found  continue;

            change: Change;
            change.path = ifx name.count then path_join(directory, name, allocator = temp) else directory;
            change.kind = from_inotify_mask(event.mask);
            array_add(*changes, change);
        }
    }
    return changes;
}

#scope_module

to_inotify_mask :: (kinds: Change_Kind) -> u32 {
    mask: u32;
    if kinds & .MODIFIED    mask |= IN_MODIFY | IN_CLOSE_WRITE;
    if kinds & .CREATED     mask |= IN_CREATE;
    if kinds & .DELETED     mask |= IN_DELETE | IN_DELETE_SELF;
    if kinds & .MOVED       mask |= IN_MOVED_FROM | IN_MOVED_TO | IN_MOVE_SELF;
    if kinds & .ATTRIBUTES  mask |= IN_ATTRIB;
    return mask;
}

from_inotify_mask :: (mask: u32) -> Change_Kind {
    kind: Change_Kind;
    if mask & (IN_MODIFY | IN_CLOSE_WRITE)              kind |= .MODIFIED;
    if mask & IN_CREATE                                  kind |= .CREATED;
    if mask & (IN_DELETE | IN_DELETE_SELF)               kind |= .DELETED;
    if mask & (IN_MOVED_FROM | IN_MOVED_TO | IN_MOVE_SELF)  kind |= .MOVED;
    if mask & IN_ATTRIB                                  kind |= .ATTRIBUTES;
    return kind;
}
