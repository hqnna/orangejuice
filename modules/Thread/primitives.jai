//
// Threads, mutexes, condition variables and semaphores, on pthreads.
//
// A thread is created suspended and only runs once `thread_start` is called,
// which is what lets the caller finish filling in the `Thread` — its `data`,
// its group's worker info — before the body can see it.
//

Thread_Index :: s64;
Thread_Proc  :: #type (*Thread) -> s64;

Thread :: struct {
    index: Thread_Index;
    proc:  Thread_Proc;
    data:  *void;
    // Zero in a program that is running; the workspace it belongs to inside
    // the compiler.
    workspace: Workspace;

    // The context the body starts with, and the temporary storage that context
    // points at. Both live here rather than on the new thread's stack, since
    // the thread is created before it runs.
    starting_context: #Context;
    starting_temporary_storage: Temporary_Storage;

    allocator_used_for_temporary_storage: Allocator;

    #if LOAD_THREAD_GROUP  worker_info: *Thread_Group.Worker_Info;

    #if _STACK_TRACE  stack_trace_sentinel: Stack_Trace_Node;

    using specific: Thread_Os_Specific;
}

POSIX_THREADS :: (OS == .LINUX || OS == .MACOS || OS == .IOS || OS == .ANDROID);

// Sets a thread up and creates it, suspended. `temporary_storage_size` bytes
// are taken from the *calling* thread's allocator for the new thread's
// temporary storage, unless `starting_storage` hands over one already built.
thread_init :: (thread: *Thread, proc: Thread_Proc, temporary_storage_size: s32 = 16384, starting_storage: *Temporary_Storage = null) -> bool {
    thread.workspace = get_current_workspace();

    entry_proc :: (parameter: *void) -> *void #c_call {
        thread := cast(*Thread) parameter;
        // Created suspended: the body waits here until `thread_start` signals.
        wait_for(*thread.suspended_semaphore);

        new_context := thread.starting_context;
        new_context.thread_index = xx thread.index;

        #if _STACK_TRACE  new_context.stack_trace = *thread.stack_trace_sentinel;

        result: s32;
        push_context new_context {
            result = cast(s32) thread.proc(thread);

            // `is_done` is the fast path of `thread_is_done`; the semaphore is
            // what a caller with a timeout waits on.
            thread.is_done = true;
            signal(*thread.is_alive_semaphore);
            return cast(*void) result;
        }
    }

    init(*thread.suspended_semaphore);
    init(*thread.is_alive_semaphore);

    if pthread_create(*thread.thread_handle, null, entry_proc, thread) != 0 {
        destroy(*thread.suspended_semaphore);
        destroy(*thread.is_alive_semaphore);
        return false;
    }

    thread.proc = proc;

    thread.starting_context.allocator   = context.allocator;
    thread.starting_context.logger      = context.logger;
    thread.starting_context.logger_data = context.logger_data;

    storage := *thread.starting_temporary_storage;
    thread.starting_context.temporary_storage = storage;
    if starting_storage {
        storage.* = starting_storage.*;
    } else {
        set_initial_data(storage, temporary_storage_size, Basic.alloc(temporary_storage_size));
        storage.current_page_bytes_occupied = 0;
        storage.total_bytes_occupied = 0;
        storage.high_water_mark = 0;

        thread.allocator_used_for_temporary_storage = context.allocator;
    }

    thread.index = atomic_add(*next_thread_index, 1);
    return true;
}

// Joins the thread and gives back everything `thread_init` took. The thread
// must have finished: its temporary storage is freed here, and the body is
// still using it until it returns.
thread_deinit :: (thread: *Thread) {
    if thread.thread_handle {
        pthread_join(thread.thread_handle, null);
        thread.thread_handle = 0;
    }

    destroy(*thread.suspended_semaphore);
    destroy(*thread.is_alive_semaphore);

    assert(thread.index > 0);

    storage := *thread.starting_temporary_storage;
    if thread.allocator_used_for_temporary_storage.proc {
        Basic.free(storage.original_data,, thread.allocator_used_for_temporary_storage);
    }

    overflow := storage.overflow_pages;
    while overflow {
        next := overflow.next;
        Basic.free(overflow,, overflow.allocator);
        overflow = next;
    }

    thread.index = 0;
}

// ---------------------------------------------------------------- the types ---

Per_OS_Critical_Section_Type   :: pthread_mutex_t;
Per_OS_Condition_Variable_Type :: pthread_cond_t;

Mutex :: struct {
    csection: Per_OS_Critical_Section_Type;
}

// A mutex may carry an order, so that locking two of them out of order can be
// caught rather than deadlocking. Ordering is off in this build.
Mutex_Order :: s32;

Wait_For_Result :: enum {
    SUCCESS;
    TIMEOUT;
    ERROR;
}

Condition_Variable :: struct {
    condition_variable: Per_OS_Condition_Variable_Type;
}

Thread_Os_Specific :: struct {
    thread_handle: pthread_t;
    // Signalled once when the body returns; `is_done` is the same answer
    // without the syscall.
    is_alive_semaphore:  Semaphore;
    suspended_semaphore: Semaphore;
    is_done: bool;
}

// Darwin has no unnamed POSIX semaphores: `sem_init` answers ENOSYS and
// `sem_timedwait` is not in libSystem at all. A count behind a mutex and a
// condition variable is the same object, built out of what is there.
#if OS == .MACOS {
    Semaphore :: struct {
        mutex:     pthread_mutex_t;
        condition: pthread_cond_t;
        count:     s32;
    }
} else {
    Semaphore :: struct {
        semaphore: sem_t;
    }
}

// ------------------------------------------------------------ running them ---

// Lets a thread created by `thread_init` begin.
thread_start :: (thread: *Thread) {
    signal(*thread.suspended_semaphore);
}

// Whether the thread's body has returned. `-1` waits forever, `0` only looks.
// Asking again after a true answer gives true again.
thread_is_done :: (thread: *Thread, milliseconds: s32 = 0) -> bool {
    if milliseconds < -1  Basic.log_error("Invalid value % for 'milliseconds' in thread_is_done().\n", milliseconds);

    if thread.is_done  return true;

    // pthreads has no way to ask a sibling whether it is still running, so the
    // body signals a semaphore on its way out and this waits on that.
    if wait_for(*thread.is_alive_semaphore, milliseconds) != .SUCCESS  return false;

    // Put the count back, so that a second call answers the same way.
    signal(*thread.is_alive_semaphore);

    assert(thread.is_done);
    return true;
}

// ------------------------------------------------------------------ mutexes ---

init :: (using m: *Mutex, desired_name := "", desired_order: Mutex_Order = -1) {
    attr: pthread_mutexattr_t;
    pthread_mutexattr_init(*attr);
    // Recursive: locking one twice on the same thread is counted rather than
    // deadlocking, which is what the condition-variable dance below needs.
    pthread_mutexattr_settype(*attr, xx PTHREAD_MUTEX.RECURSIVE_NP);

    result := pthread_mutex_init(*csection, *attr);
    assert(result == 0);
}

destroy :: (using m: *Mutex) {
    pthread_mutex_destroy(*csection);
}

lock :: (using m: *Mutex) {
    csection_lock(*m.csection);
}

unlock :: (using m: *Mutex) {
    csection_unlock(*m.csection);
}

// The raw critical section, for code holding one of its own rather than a
// `Mutex`.
csection_lock :: (csection: *Per_OS_Critical_Section_Type) #expand {
    pthread_mutex_lock(csection);
}

csection_unlock :: (csection: *Per_OS_Critical_Section_Type) #expand {
    pthread_mutex_unlock(csection);
}

// ------------------------------------------------- condition variables ---

init :: (using cv: *Condition_Variable, desired_name := "") {
    pthread_cond_init(*cv.condition_variable, null);
}

destroy :: (using cv: *Condition_Variable) {
    pthread_cond_destroy(*cv.condition_variable);
}

// Releases the mutex and sleeps until woken, holding the mutex again on the
// way out. The two halves are one step, which is what keeps a wake from
// arriving between them and being missed.
wait :: (c: *Condition_Variable, m: *Mutex) {
    cvariable_wait(*c.condition_variable, *m.csection);
}

wake :: (c: *Condition_Variable) {
    cvariable_wake(*c.condition_variable);
}

wake_all :: (c: *Condition_Variable) {
    cvariable_wake_all(*c.condition_variable);
}

cvariable_wait :: (condition_variable: *Per_OS_Condition_Variable_Type, csection: *Per_OS_Critical_Section_Type, wait_time_ms: s32 = -1) {
    if wait_time_ms == -1 {
        pthread_cond_wait(condition_variable, csection);
        return;
    }

    // `pthread_cond_timedwait` takes an absolute deadline, so the wait is
    // added onto the current time.
    seconds := wait_time_ms / 1000;
    milliseconds := wait_time_ms - seconds * 1000;

    time: timespec;
    clock_gettime(.REALTIME, *time);
    time.tv_sec  += seconds;
    time.tv_nsec += milliseconds * 1_000_000;
    pthread_cond_timedwait(condition_variable, csection, *time);
}

cvariable_wake :: (condition_variable: *Per_OS_Condition_Variable_Type) {
    pthread_cond_signal(condition_variable);
}

cvariable_wake_all :: (condition_variable: *Per_OS_Condition_Variable_Type) {
    pthread_cond_broadcast(condition_variable);
}

// ---------------------------------------------------------------- semaphores ---

init :: (s: *Semaphore, initial_value: s32 = 0) {
    assert(initial_value >= 0);
    #if OS == .MACOS {
        assert(pthread_mutex_init(*s.mutex, null) == 0);
        assert(pthread_cond_init(*s.condition, null) == 0);
        s.count = initial_value;
    } else {
        result := sem_init(*s.semaphore, 0, cast(u32) initial_value);
        assert(result == 0);
    }
}

destroy :: (s: *Semaphore) {
    #if OS == .MACOS {
        pthread_cond_destroy(*s.condition);
        pthread_mutex_destroy(*s.mutex);
    } else {
        sem_destroy(*s.semaphore);
    }
}

signal :: (s: *Semaphore) {
    #if OS == .MACOS {
        pthread_mutex_lock(*s.mutex);
        s.count += 1;
        pthread_cond_signal(*s.condition);
        pthread_mutex_unlock(*s.mutex);
    } else {
        sem_post(*s.semaphore);
    }
}

// Waits for a count. Negative waits forever; otherwise the answer says whether
// the count arrived or the time ran out.
wait_for :: (s: *Semaphore, milliseconds: s32 = -1) -> Wait_For_Result #no_context {
#if OS == .MACOS {
    pthread_mutex_lock(*s.mutex);
    answer := Wait_For_Result.SUCCESS;
    if milliseconds < 0 {
        // A condition variable may wake without the count having arrived, so
        // the test is a loop rather than an `if` whichever way it waits.
        while s.count <= 0  pthread_cond_wait(*s.condition, *s.mutex);
    } else {
        now: timespec;
        if clock_gettime(.REALTIME, *now) == -1 {
            pthread_mutex_unlock(*s.mutex);
            return .ERROR;
        }
        nanoseconds := cast(s64) now.tv_nsec + 1_000_000 * cast(s64) milliseconds;
        deadline := timespec.{
            tv_sec  = now.tv_sec + nanoseconds / 1_000_000_000,
            tv_nsec = nanoseconds % 1_000_000_000,
        };
        while s.count <= 0 {
            if pthread_cond_timedwait(*s.condition, *s.mutex, *deadline) != 0 {
                // The deadline passed. The count may still have arrived on the
                // way out, which is a wait that succeeded.
                if s.count <= 0  answer = .TIMEOUT;
                break;
            }
        }
    }
    if answer == .SUCCESS  s.count -= 1;
    pthread_mutex_unlock(*s.mutex);
    return answer;
} else {
    result: s32 = ---;
    if milliseconds < 0 {
        result = sem_wait(*s.semaphore);
        // A signal delivered to this thread interrupts the wait without the
        // count having arrived, which is not an answer.
        while result == -1 && errno() == EINTR {
            result = sem_wait(*s.semaphore);
        }
    } else {
        now: timespec;
        if clock_gettime(.REALTIME, *now) == -1  return .ERROR;

        // `sem_timedwait` takes an absolute deadline too.
        nanoseconds := cast(s64) now.tv_nsec + 1_000_000 * cast(s64) milliseconds;
        deadline := timespec.{
            tv_sec  = now.tv_sec + nanoseconds / 1_000_000_000,
            tv_nsec = nanoseconds % 1_000_000_000,
        };

        result = sem_timedwait(*s.semaphore, *deadline);
    }

    if result == -1 {
        if errno() == ETIMEDOUT  return .TIMEOUT;
        return .ERROR;
    }

    return .SUCCESS;
}
}
