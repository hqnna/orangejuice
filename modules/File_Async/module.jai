//
// Asynchronous file IO.
//
// A program submits reads and writes to a `Queue` and asks it later which of
// them finished, rather than stalling on each in turn the way the `File`
// module does. Every submission carries a `User_Data` of the caller's own
// choosing, which is what comes back with the completion — the queue has no
// idea what a request *means*, so the caller does not have to keep a table of
// what it asked for.
//
// The queue is typed by that user data, so `wait_for_completion` hands back a
// `T` rather than a `*void` the caller has to cast.
//
// Opening and truncating still block: those are metadata operations, and the
// interfaces that do them asynchronously are not the same anywhere.
//

// What went wrong, if anything. `platform_code` is `errno` where there was
// one; it may be zero even for a real error, since not every failure here
// comes from a syscall.
Error :: struct {
    code: enum {
        Success :: 0;
        // `check_only` was given and nothing had finished.
        DidNotWait;
        // The submission queue is full; wait or drain completions and retry.
        FullQueue;
        // No such file, no permission, or the like.
        FileAccessFailed;
        // Some of the data moved, but not all of it.
        Incomplete;
        // Should not happen; a caller may reasonably assert on it.
        Catastrophic;
    };
    platform_code: s64;
}

#load "thread_pool.jai";

#scope_module

error :: (code: type_of(Error.code), platform_code: $T = 0) -> Error {
    result: Error = ---;
    result.code = code;
    result.platform_code = xx platform_code;
    return result;
}
