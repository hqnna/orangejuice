//
// Asynchronous file IO: ask for a read, get on with something else, collect
// the result later.
//
// A pool of worker threads does the blocking calls. The reference also has an
// io_uring backend, which is faster for very many small requests because it
// avoids a syscall per operation; a thread pool is what works on any kernel
// and is not the bottleneck for the file-sized reads programs usually do.
//
// Requests are handed out as handles rather than pointers, so a caller cannot
// accidentally read a result while a worker is still writing it.
//

#import "Basic";
#import "File";
#import "POSIX";
#import "Thread";

Request_Status :: enum u8 {
    // Never issued, or the handle is stale.
    UNKNOWN   :: 0;
    PENDING   :: 1;
    COMPLETE  :: 2;
    FAILED    :: 3;
}

Request_Kind :: enum u8 {
    READ  :: 0;
    WRITE :: 1;
}

Request_Handle :: struct {
    index:      s64 = -1;
    // Bumped every time a slot is reused, so a handle kept past its request
    // does not read somebody else's result.
    generation: s64;
}

Request :: struct {
    kind:       Request_Kind;
    path:       string;
    // For a write, what to write. For a read, what was read.
    data:       string;
    status:     Request_Status;
    generation: s64;
    // Whether the slot is free for a new request.
    in_use:     bool;
}

File_Async :: struct {
    mutex:     Mutex;
    // Woken when a request is queued, and when one finishes.
    work:      Condition;
    done:      Condition;

    requests:  [..] Request;
    // Indices of requests not yet picked up.
    queue:     [..] s64;

    threads:   [] Thread;
    running:   bool;
    allocator: Allocator;
}

// Starts `thread_count` workers. Zero asks for one per processor, which is
// what a program reading many files at once wants.
init :: (async: *File_Async, thread_count := 0, allocator := context.allocator) -> bool {
    async.allocator = allocator;
    async.requests.allocator = allocator;
    async.queue.allocator = allocator;

    if !mutex_init(*async.mutex)      return false;
    if !condition_init(*async.work)   return false;
    if !condition_init(*async.done)   return false;

    count := thread_count;
    if count <= 0  count = get_number_of_processors();
    if count <= 0  count = 1;

    async.running = true;
    async.threads = NewArray(Thread, count, allocator);
    for * async.threads {
        if !thread_init(it, worker, async, cast(u32) it_index) {
            // Whatever started already is shut down rather than left running.
            deinit(async);
            return false;
        }
    }
    return true;
}

// Stops the workers and gives everything back. Requests still pending are
// abandoned: a caller wanting them should wait first.
deinit :: (async: *File_Async) {
    lock(*async.mutex);
    async.running = false;
    unlock(*async.mutex);
    condition_broadcast(*async.work);

    for * async.threads  thread_deinit(it);
    if async.threads.data  free(async.threads.data, async.allocator);
    async.threads.count = 0;

    for * async.requests  free_string(*it.data, async.allocator);
    array_free(*async.requests);
    array_free(*async.queue);

    condition_destroy(*async.work);
    condition_destroy(*async.done);
    mutex_destroy(*async.mutex);
}

// Asks for a file to be read. The handle is how the result is collected.
read_file :: (async: *File_Async, path: string) -> Request_Handle {
    return submit(async, .READ, path, "");
}

// Asks for a file to be written. `contents` is copied, so the caller may free
// or change theirs immediately.
write_file :: (async: *File_Async, path: string, contents: string) -> Request_Handle {
    return submit(async, .WRITE, path, contents);
}

// Whether a request has finished, without waiting.
poll :: (async: *File_Async, handle: Request_Handle) -> Request_Status {
    lock(*async.mutex);
    defer unlock(*async.mutex);

    request := lookup(async, handle);
    if !request  return .UNKNOWN;
    return request.status;
}

// Waits for one request and hands back what it produced. The result is copied
// into `allocator`, and the slot is released for reuse.
wait :: (async: *File_Async, handle: Request_Handle, allocator := context.allocator) -> (data: string, ok: bool) {
    lock(*async.mutex);
    defer unlock(*async.mutex);

    while true {
        request := lookup(async, handle);
        if !request  return "", false;

        if request.status == .PENDING {
            condition_wait(*async.done, *async.mutex);
            continue;
        }

        ok := request.status == .COMPLETE;
        result: string;
        if ok && request.data.count  result = copy_string(request.data, allocator);

        // The slot is free now, and its generation moves on so the handle
        // cannot be used again.
        free_string(*request.data, async.allocator);
        free_string(*request.path, async.allocator);
        request.in_use = false;
        request.generation += 1;
        request.status = .UNKNOWN;
        return result, ok;
    }
    return "", false;
}

#scope_module

submit :: (async: *File_Async, kind: Request_Kind, path: string, contents: string) -> Request_Handle {
    handle: Request_Handle;

    lock(*async.mutex);
    defer unlock(*async.mutex);

    index := free_slot(async);
    if index < 0  return handle;

    request := *async.requests[index];
    request.kind = kind;
    request.path = copy_string(path, async.allocator);
    // Only ever storage this owns: assigning `""` here would put a literal in
    // the slot, and releasing the request would then free static bytes.
    request.data.data = null;
    request.data.count = 0;
    if contents.count  request.data = copy_string(contents, async.allocator);
    request.status = .PENDING;
    request.in_use = true;

    array_add(*async.queue, index);

    handle.index = index;
    handle.generation = request.generation;

    condition_signal(*async.work);
    return handle;
}

// A slot nobody is using, or a fresh one.
free_slot :: (async: *File_Async) -> s64 {
    for * async.requests  if !it.in_use  return it_index;
    slot: Request;
    array_add(*async.requests, slot);
    return async.requests.count - 1;
}

// The request a handle names, or null if the slot has moved on.
lookup :: (async: *File_Async, handle: Request_Handle) -> *Request {
    if handle.index < 0 || handle.index >= async.requests.count  return null;
    request := *async.requests[handle.index];
    if request.generation != handle.generation  return null;
    return request;
}

// One worker. Takes a request, does the blocking call *without* the lock held
// — the whole point is that other threads keep working — then takes the lock
// again to record what happened.
worker :: (thread: *Thread) -> s64 {
    async := cast(*File_Async) thread.data;

    while true {
        lock(*async.mutex);
        while async.running && !async.queue.count {
            condition_wait(*async.work, *async.mutex);
        }
        if !async.running {
            unlock(*async.mutex);
            break;
        }

        index := async.queue[0];
        array_ordered_remove_by_index(*async.queue, 0);

        // Copied out under the lock rather than held as a pointer: `submit`
        // may grow `requests` while this thread is doing its blocking call,
        // and growing it moves every element.
        kind: Request_Kind;
        generation: s64;
        path: string;
        contents: string;

        {
            request := *async.requests[index];
            kind = request.kind;
            generation = request.generation;
            path = copy_string(request.path, temp);
            if kind == .WRITE && request.data.count  contents = copy_string(request.data, temp);
        }
        unlock(*async.mutex);

        data: string;
        ok: bool;
        if kind == .READ {
            data, ok = read_entire_file(path, async.allocator);
        } else {
            ok = write_entire_file(path, contents);
        }

        lock(*async.mutex);
        // Looked up again: the array may have moved, and the slot may have
        // been released and reused while this thread was working.
        request := *async.requests[index];
        if request.in_use && request.generation == generation {
            if kind == .READ {
                free_string(*request.data, async.allocator);
                request.data = data;
            } else if data.count {
                free_string(*data, async.allocator);
            }
            request.status = ifx ok then Request_Status.COMPLETE else .FAILED;
        } else if data.count {
            free_string(*data, async.allocator);
        }
        unlock(*async.mutex);

        condition_broadcast(*async.done);
        reset_temporary_storage();
    }
    return 0;
}
