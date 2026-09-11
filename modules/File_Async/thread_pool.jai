//
// The queue, backed by a pool of worker threads doing the blocking calls.
//
// A submission goes onto one worker's list, round robin; the worker performs
// it and puts the result onto the queue's shared completion list.
// `wait_for_completion` takes from that list, sleeping on a condition variable
// until something arrives.
//

#import "Basic";
#import "POSIX";
#import "Thread";
#import "Atomics";

Queue :: struct(User_Data: Type) {
    THREADS            :: 4;
    SUBMISSION_ENTRIES :: 32;
    COMPLETION_ENTRIES :: 32;

    Operation :: struct {
        next: *Operation;

        kind: enum u8 {
            READ;
            WRITE;
            READ_ENTIRE_FILE;
            WRITE_ENTIRE_FILE;
        };

        descriptor: s32;
        // Owned by the queue for the whole-file forms, which have to open the
        // file themselves.
        owns_descriptor: bool;
        // Freed with the operation, since the caller's string need only live
        // until the submitting call returns.
        path: string;

        position: s64;
        // For a read, where the bytes go; for a write, where they come from.
        // A whole-file read fills this in itself.
        data: [] u8;

        user_data: User_Data;
        result: Error;

        unfinished_operations: *s64;
    };

    Worker :: struct {
        thread: Thread;

        mutex:     Mutex;
        condition: Condition_Variable;
        first: *Operation;
        last:  *Operation;
        count: s32;
    };

    allocator: Allocator;

    workers: [THREADS] Worker;
    next_worker: u64;

    // Where every worker puts what it finished.
    completion_mutex:     Mutex;
    completion_condition: Condition_Variable;
    completed_first: *Operation;
    completed_last:  *Operation;
    completed_count: s32;

    should_exit: bool;
    queue_data: *void;
}

// A file the queue may be given operations on. `unfinished_operations` is what
// keeps the descriptor open until the last of them is through, so a caller may
// close the file while its reads are still in flight.
File :: struct(User_Data: Type) {
    descriptor: s32;
    unfinished_operations: *s64;
}

initialize_queue :: ($User_Data: Type) -> (queue: *Queue(User_Data), result := Error.{}) {
    queue := New(Queue(User_Data));
    remember_allocators(queue);

    init(*queue.completion_mutex);
    init(*queue.completion_condition);

    // Declared here so that it is one procedure per `User_Data`: the worker
    // has to know which `Queue` it was handed, and the thread proc's own
    // signature has nowhere to say so.
    worker_proc :: (thread: *Thread) -> s64 {
        run_worker(cast(*Queue(User_Data)) thread.data, thread);
        return 0;
    }

    for * worker: queue.workers {
        init(*worker.mutex);
        init(*worker.condition);
        if !thread_init(*worker.thread, worker_proc) {
            return queue, error(.Catastrophic);
        }
        worker.thread.data = queue;
        thread_start(*worker.thread);
    }

    return queue;
}

// Stops the workers and gives the queue's storage back. Anything still in
// flight is waited for, since a worker writing into a caller's buffer must not
// outlive the queue.
destroy_queue :: (queue: *Queue) {
    if !queue  return;

    queue.should_exit = true;
    for * worker: queue.workers {
        lock(*worker.mutex);
        wake_all(*worker.condition);
        unlock(*worker.mutex);
    }

    for * worker: queue.workers {
        while !thread_is_done(*worker.thread, -1) {}
        thread_deinit(*worker.thread);
        destroy(*worker.condition);
        destroy(*worker.mutex);
    }

    destroy(*queue.completion_condition);
    destroy(*queue.completion_mutex);

    free(queue,, queue.allocator);
}

// Opens, blocking, and hands back a handle the read and write calls take.
// `keep_existing_content` false truncates it.
open_file :: (queue: *Queue($T), file_path: string, keep_existing_content := true) -> (file: File(T), result := Error.{}) {
    file: File(T);
    flags: s32 = O_RDWR | O_CREAT | O_CLOEXEC;
    if !keep_existing_content  flags |= O_TRUNC;

    file.descriptor = open(temp_c_string(file_path), flags, 0x1B4);
    if file.descriptor == -1  return file, error(.FileAccessFailed, errno());

    // One count for the caller's own handle, dropped by `close_file`.
    file.unfinished_operations = New(s64, false,, queue.allocator);
    file.unfinished_operations.* = 1;
    return file;
}

// Drops the caller's own reference. The descriptor closes once the last
// operation on it is through, which may be after this returns.
close_file :: (queue: *Queue, file: File) {
    finish_operation(queue, file.descriptor, file.unfinished_operations, queue.allocator);
}

// The bytes read come back with the completion, allocated out of the context
// allocator of the thread that submitted.
read_entire_file :: (queue: *Queue($T), file_path: string, user_data: T) -> (result := Error.{}) {
    operation := make_operation(queue, .READ_ENTIRE_FILE, user_data);
    operation.path = copy_string(file_path,, queue.allocator);
    return submit(queue, operation);
}

write_entire_file :: (queue: *Queue($T), file_path: string, data: [] u8, user_data: T) -> (result := Error.{}) {
    operation := make_operation(queue, .WRITE_ENTIRE_FILE, user_data);
    operation.path = copy_string(file_path,, queue.allocator);
    operation.data = data;
    return submit(queue, operation);
}

// `data` is the caller's buffer, and must stay put until the completion for
// this operation comes back.
read_file :: (queue: *Queue($T), file: File, position: s64, data: [] u8, user_data: T) -> (result := Error.{}) {
    operation := make_operation(queue, .READ, user_data);
    operation.descriptor = file.descriptor;
    operation.unfinished_operations = file.unfinished_operations;
    operation.position = position;
    operation.data = data;
    atomic_increment(file.unfinished_operations);
    return submit(queue, operation);
}

write_file :: (queue: *Queue($T), file: File, position: s64, data: [] u8, user_data: T) -> (result := Error.{}) {
    operation := make_operation(queue, .WRITE, user_data);
    operation.descriptor = file.descriptor;
    operation.unfinished_operations = file.unfinished_operations;
    operation.position = position;
    operation.data = data;
    atomic_increment(file.unfinished_operations);
    return submit(queue, operation);
}

// The next finished operation: its user data, whatever bytes it produced, and
// how it went. `check_only` returns `DidNotWait` rather than sleeping.
wait_for_completion :: (queue: *Queue($T), check_only := false) -> (user_data: T, data: [] u8, result := Error.{}) {
    empty: T;

    lock(*queue.completion_mutex);
    while !queue.completed_first {
        if check_only {
            unlock(*queue.completion_mutex);
            return empty, .[], error(.DidNotWait);
        }
        if queue.should_exit {
            unlock(*queue.completion_mutex);
            return empty, .[], error(.Catastrophic);
        }
        wait(*queue.completion_condition, *queue.completion_mutex);
    }

    operation := queue.completed_first;
    queue.completed_first = operation.next;
    if !queue.completed_first  queue.completed_last = null;
    queue.completed_count -= 1;
    unlock(*queue.completion_mutex);

    user_data := operation.user_data;
    data      := operation.data;
    result    := operation.result;

    if operation.path  free(operation.path,, queue.allocator);
    free(operation,, queue.allocator);

    return user_data, data, result;
}

#scope_file

make_operation :: (queue: *Queue($T), kind: type_of(Queue(T).Operation.kind), user_data: T) -> *Queue(T).Operation {
    operation := New(Queue(T).Operation,, queue.allocator);
    operation.kind = kind;
    operation.user_data = user_data;
    return operation;
}

// Round robin over the workers. A worker's list has a bound, so a caller that
// submits faster than the disk keeps up is told rather than allowed to grow
// the queue without limit.
submit :: (queue: *Queue($T), operation: *Queue(T).Operation) -> Error {
    index := queue.next_worker % Queue(T).THREADS;
    queue.next_worker += 1;
    worker := *queue.workers[index];

    lock(*worker.mutex);
    if worker.count >= Queue(T).SUBMISSION_ENTRIES {
        unlock(*worker.mutex);
        free(operation,, queue.allocator);
        return error(.FullQueue);
    }

    if worker.last {
        worker.last.next = operation;
        worker.last = operation;
    } else {
        worker.first = operation;
        worker.last  = operation;
    }
    worker.count += 1;
    wake(*worker.condition);
    unlock(*worker.mutex);

    return .{};
}

// Drops one reference to a descriptor, closing it when the last goes.
finish_operation :: (queue: *Queue, descriptor: s32, unfinished: *s64, allocator: Allocator) {
    if !unfinished  return;
    if atomic_decrement(unfinished) {
        close(descriptor);
        free(unfinished,, allocator);
    }
}

run_worker :: (queue: *Queue($T), thread: *Thread) {
    worker: *Queue(T).Worker;
    for * queue.workers  if *it.thread == thread  worker = it;
    if !worker  return;

    while !queue.should_exit {
        lock(*worker.mutex);
        while !worker.first && !queue.should_exit {
            wait(*worker.condition, *worker.mutex);
        }
        operation := worker.first;
        if operation {
            worker.first = operation.next;
            if !worker.first  worker.last = null;
            worker.count -= 1;
            operation.next = null;
        }
        unlock(*worker.mutex);

        if !operation  continue;

        perform(queue, operation);

        lock(*queue.completion_mutex);
        if queue.completed_last {
            queue.completed_last.next = operation;
            queue.completed_last = operation;
        } else {
            queue.completed_first = operation;
            queue.completed_last  = operation;
        }
        queue.completed_count += 1;
        wake(*queue.completion_condition);
        unlock(*queue.completion_mutex);
    }
}

perform :: (queue: *Queue($T), operation: *Queue(T).Operation) {
    if operation.kind == {
        case .READ;
            moved := pread(operation.descriptor, operation.data.data, cast(u64) operation.data.count, operation.position);
            if moved < 0                            operation.result = error(.Incomplete, errno());
            else if moved != operation.data.count   operation.result = error(.Incomplete);
            finish_operation(queue, operation.descriptor, operation.unfinished_operations, queue.allocator);

        case .WRITE;
            moved := pwrite(operation.descriptor, operation.data.data, cast(u64) operation.data.count, operation.position);
            if moved < 0                            operation.result = error(.Incomplete, errno());
            else if moved != operation.data.count   operation.result = error(.Incomplete);
            finish_operation(queue, operation.descriptor, operation.unfinished_operations, queue.allocator);

        case .READ_ENTIRE_FILE;
            descriptor := open(temp_c_string(operation.path), O_RDONLY | O_CLOEXEC);
            if descriptor == -1 {
                operation.result = error(.FileAccessFailed, errno());
                return;
            }
            defer close(descriptor);

            length := lseek(descriptor, 0, SEEK_END);
            if length < 0 {
                operation.result = error(.Catastrophic, errno());
                return;
            }

            buffer := NewArray(length, u8, false);
            moved := pread(descriptor, buffer.data, cast(u64) length, 0);
            if moved < 0 {
                operation.result = error(.Incomplete, errno());
                free(buffer.data);
                return;
            }
            buffer.count = moved;
            operation.data = buffer;
            if moved != length  operation.result = error(.Incomplete);

        case .WRITE_ENTIRE_FILE;
            descriptor := open(temp_c_string(operation.path), O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0x1B4);
            if descriptor == -1 {
                operation.result = error(.FileAccessFailed, errno());
                return;
            }
            defer close(descriptor);

            moved := pwrite(descriptor, operation.data.data, cast(u64) operation.data.count, 0);
            if moved < 0                            operation.result = error(.Incomplete, errno());
            else if moved != operation.data.count   operation.result = error(.Incomplete);
    }
}
