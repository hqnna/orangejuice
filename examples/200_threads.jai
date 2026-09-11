//
// Threads, and the atomics that make what they share safe.
//
// Each thread gets a context of its own (**L§10.3**), which is why two threads
// allocating at once do not have to agree about anything.
//

#import "Basic";
#import "Thread";
#import "Atomics";

Work :: struct {
    from:  int;
    to:    int;
    total: int;
}

// The shared counter every worker adds into, with an atomic add so that two
// threads adding at once cannot lose one of the additions.
shared_total: s64;

sum_range :: (thread: *Thread) -> s64 {
    work := cast(*Work) thread.data;
    for work.from..work.to  work.total += it;
    atomic_add(*shared_total, cast(s64) work.total);
    return 0;
}

main :: () {
    WORKERS :: 4;
    PER_WORKER :: 250;

    threads: [WORKERS] Thread;
    work:    [WORKERS] Work;

    for * work {
        it.from = it_index * PER_WORKER + 1;
        it.to = (it_index + 1) * PER_WORKER;
    }

    for * threads {
        thread_init(it, sum_range);
        it.data = *work[it_index];
        thread_start(it);
    }
    for * threads {
        while !thread_is_done(it)  sleep_milliseconds(1);
        thread_deinit(it);
    }

    // 1 + 2 + … + 1000, arrived at four ways at once.
    print("shared total: %\n", shared_total);
    print("expected:     %\n", 1000 * 1001 / 2);

    for work  print("worker % summed %..%: %\n", it_index, it.from, it.to, it.total);
}
