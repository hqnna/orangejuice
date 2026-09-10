//
// Threads, mutexes and condition variables.
//
// Each thread gets a `#Context` of its own (**L§10.1**), built the way the
// main one was: its own temporary storage, and the allocator and logger the
// creating thread had. A thread that shared the creator's context would be
// sharing its temporary arena, and two threads bumping the same pointer is
// exactly the bug this avoids.
//

#import "Basic";
#import "POSIX";

Thread_Proc :: #type (thread: *Thread) -> s64;

Thread :: struct {
    handle: pthread_t;
    proc:   Thread_Proc;
    // The caller's, for the thread to find.
    data:   *void;
    index:  u32;
    // What `proc` returned, once `thread_join` has collected it.
    result: s64;

    started := false;
    // Storage the thread's own context and arena live in.
    thread_context: #Context;
    temporary_storage: Temporary_Storage;
    temporary_data: [] u8;
}

TEMPORARY_STORAGE_SIZE :: 32768;

// Starts `proc` on a thread of its own. `ok` is false if the thread could not
// be created, in which case nothing is running.
thread_init :: (thread: *Thread, proc: Thread_Proc, data: *void = null, index: u32 = 0) -> (ok: bool) {
    if !proc  return false;

    thread.proc = proc;
    thread.data = data;
    thread.index = index;

    // The new thread's context is a copy of this one's — same allocator, same
    // logger — with an arena of its own.
    thread.thread_context = context;
    thread.thread_context.thread_index = index;

    thread.temporary_data = NewArray(TEMPORARY_STORAGE_SIZE, u8);
    if !thread.temporary_data.data  return false;

    set_initial_data(*thread.temporary_storage, TEMPORARY_STORAGE_SIZE, thread.temporary_data.data);
    thread.thread_context.temporary_storage = *thread.temporary_storage;

    if pthread_create(*thread.handle, null, thread_entry, thread) != 0 {
        free(thread.temporary_data.data);
        thread.temporary_data.count = 0;
        return false;
    }
    thread.started = true;
    return true;
}

// Waits for it to finish and collects what it returned.
thread_join :: (thread: *Thread) -> s64 {
    if !thread.started  return 0;
    pthread_join(thread.handle, null);
    thread.started = false;
    return thread.result;
}

// Lets it run unattended. Nothing may touch the `Thread` afterwards: it owns
// its own storage and nobody is left to say when it is finished with it.
thread_detach :: (thread: *Thread) {
    if !thread.started  return;
    pthread_detach(thread.handle);
    thread.started = false;
}

thread_deinit :: (thread: *Thread) {
    if thread.started  thread_join(thread);
    free(thread.temporary_data.data);
    thread.temporary_data.count = 0;
}

// How many threads the machine will actually run at once.
get_number_of_processors :: () -> s64 {
    count := sysconf(_SC_NPROCESSORS_ONLN);
    return ifx count > 0 then count else 1;
}

// --------------------------------------------------------------- mutexes ---

Mutex :: struct {
    handle: pthread_mutex_t;
    ready := false;
}

mutex_init :: (mutex: *Mutex) -> bool {
    if pthread_mutex_init(*mutex.handle, null) != 0  return false;
    mutex.ready = true;
    return true;
}

mutex_destroy :: (mutex: *Mutex) {
    if !mutex.ready  return;
    pthread_mutex_destroy(*mutex.handle);
    mutex.ready = false;
}

lock :: (mutex: *Mutex) {
    if mutex.ready  pthread_mutex_lock(*mutex.handle);
}

try_lock :: (mutex: *Mutex) -> bool {
    if !mutex.ready  return false;
    return pthread_mutex_trylock(*mutex.handle) == 0;
}

unlock :: (mutex: *Mutex) {
    if mutex.ready  pthread_mutex_unlock(*mutex.handle);
}

// ---------------------------------------------------- condition variables ---

Condition :: struct {
    handle: pthread_cond_t;
    ready := false;
}

condition_init :: (condition: *Condition) -> bool {
    if pthread_cond_init(*condition.handle, null) != 0  return false;
    condition.ready = true;
    return true;
}

condition_destroy :: (condition: *Condition) {
    if !condition.ready  return;
    pthread_cond_destroy(*condition.handle);
    condition.ready = false;
}

// Releases the mutex, waits, and takes it again before returning. A wakeup
// can happen without a signal, so the caller must re-test whatever it is
// waiting for rather than assume.
condition_wait :: (condition: *Condition, mutex: *Mutex) {
    if condition.ready && mutex.ready  pthread_cond_wait(*condition.handle, *mutex.handle);
}

condition_signal :: (condition: *Condition) {
    if condition.ready  pthread_cond_signal(*condition.handle);
}

condition_broadcast :: (condition: *Condition) {
    if condition.ready  pthread_cond_broadcast(*condition.handle);
}
// ------------------------------------------------------- the short names ---
//
// A caller writes `init(*mutex)` and `destroy(*mutex)`; the `mutex_`-prefixed
// spellings above are what the bodies here are, kept because a caller holding
// both a mutex and a condition sometimes wants to say which it means.

init :: (mutex: *Mutex) -> bool {
    return mutex_init(mutex);
}

destroy :: (mutex: *Mutex) {
    mutex_destroy(mutex);
}

// The reference spells the type this way; both names are the same struct.
Condition_Variable :: Condition;

init :: (condition: *Condition) -> bool {
    return condition_init(condition);
}

destroy :: (condition: *Condition) {
    condition_destroy(condition);
}

wait :: (condition: *Condition, mutex: *Mutex) {
    condition_wait(condition, mutex);
}

signal :: (condition: *Condition) {
    condition_signal(condition);
}

broadcast :: (condition: *Condition) {
    condition_broadcast(condition);
}


#scope_module

// The C entry point. It pushes the context the thread was given and calls
// what the caller asked for; `#c_call` because pthreads calls it, so there is
// no context until this puts one in place.
thread_entry :: (argument: *void) -> *void #c_call {
    thread := cast(*Thread) argument;
    push_context thread.thread_context {
        thread.result = thread.proc(thread);
    }
    return null;
}
