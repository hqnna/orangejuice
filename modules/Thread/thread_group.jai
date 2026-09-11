//
// A pool of worker threads, each with a queue of its own.
//
// Work is handed in as a `*void` and comes back the same way once a worker has
// run the group's procedure over it. Each worker has an `available` list it
// takes from and a `completed` list it puts back onto, so the only contention
// is between the caller and one worker rather than among all of them.
//
// With work stealing on, a worker that runs out looks through the others'
// queues before going to sleep, which is what keeps one long job from leaving
// the rest of the pool idle.
//

#scope_export

Thread_Group_Proc :: #type (group: *Thread_Group, thread: *Thread, work: *void) -> Thread_Continue_Status;

// What a worker procedure returns. `STOP` retires that worker — it is for a
// shortage the work cannot go on without, not for ordinary failure.
Thread_Continue_Status :: enum {
    STOP     :: 0;
    CONTINUE :: 1;
}

Thread_Group :: struct {
    // Yours to set.
    data: *void;
    // Set by `init`; may be replaced before `start`.
    proc: Thread_Group_Proc;
    // Used in the log lines below.
    name: string;
    logging := true;

    // Where the log lines get their timestamps. Null means
    // `Basic.seconds_since_init`.
    time_proc: () -> float64;

    // Only the thread calling into this module allocates, so this need not be
    // threadsafe.
    allocator: Allocator;

    worker_info: [] Worker_Info;
    worker_info_data_to_free: *void;

    next_worker_index: s32;

    // Padded to a cache line so that two workers' counters never share one,
    // which would make every push on one queue invalidate the other's line.
    Worker_Info :: struct {
        using info: Unpadded_Worker_Info;

        DESIRED_SIZE :: #run Basic.align_forward(size_of(type_of(info)), CACHE_LINE_SIZE);
      #place info;
        padding: [DESIRED_SIZE] u8 = ---;
        // Which other workers this one may steal from, shuffled; empty when
        // stealing is off.
        work_steal_indices: [] s32;
    }

    Unpadded_Worker_Info :: struct {
        thread:    Thread;
        available: Work_List;
        completed: Work_List;

        group:        *Thread_Group;
        worker_index: s32;
    }

    // One queue. The semaphore counts what is on it, so a worker sleeps rather
    // than spins while it is empty.
    Work_List :: struct {
        semaphore: Semaphore;
        mutex:     Mutex;
        first:     *Work_Entry;
        last:      *Work_Entry;

        count: s32;
    }

    // The node the group wraps each `*void` in, so a caller never has to know
    // what the queue looks like.
    Work_Entry :: struct {
        next:         *Work_Entry;
        work:         *void;
        thread_index: Thread_Index;
        logging_name: string;

        issue_time:      float64;
        work_list_index: s32;
    }

    initted     := false;
    started     := false;
    // Set by `shutdown`, read by every worker's loop.
    should_exit := false;
}

Work_List  :: Thread_Group.Work_List;
Work_Entry :: Thread_Group.Work_Entry;

// Creates the workers, suspended. Members of the group may still be set
// afterwards; `start` is what lets them run.
init :: (using group: *Thread_Group, num_threads: s32, group_proc: Thread_Group_Proc, enable_work_stealing := false, temporary_storage_size: s32 = 16384) {
    Basic.remember_allocators(group);

    worker_info, worker_info_data_to_free = Basic.NewArray(num_threads, Worker_Info, true, alignment = CACHE_LINE_SIZE,, group.allocator);
    assert((cast(u64) worker_info.data) % cast(u64) CACHE_LINE_SIZE == 0);

    group.proc = group_proc;

    for * info: worker_info {
        thread_init(*info.thread, thread_group_run, temporary_storage_size = temporary_storage_size);
        info.thread.worker_info = info;

        init_work_list(*info.available);
        init_work_list(*info.completed);

        info.group        = group;
        info.worker_index = cast(s32) it_index;

        if enable_work_stealing && num_threads > 1 {
            // Every worker but this one, so a thief never looks at its own
            // queue twice.
            indices := Basic.NewArray(num_threads - 1, s32, false,, group.allocator);
            cursor := 0;
            for i: 0..num_threads - 1 {
                if i == it_index  continue;
                indices[cursor] = xx i;
                cursor += 1;
            }
            info.work_steal_indices = indices;
        }
    }

    group.initted = true;
}

start :: (group: *Thread_Group) {
    for * group.worker_info  thread_start(*it.thread);
    group.started = true;
}

// Tells the workers to stop and waits up to `timeout_milliseconds` for them —
// `-1` waits forever. False means some worker was still busy and nothing was
// freed; call again. After a true answer every `Thread` in the group is gone,
// so anything of yours hanging off one must be dealt with first.
shutdown :: (using group: *Thread_Group, timeout_milliseconds: s32 = -1) -> bool {
    assert(initted);

    should_exit = true;

    all_done := true;
    if started {
        // A worker asleep on its semaphore has to be woken to see the flag; a
        // worker in the middle of a job may take arbitrarily long to look.
        for * worker_info  signal(*it.available.semaphore);

        start_time: Basic.Apollo_Time;
        if timeout_milliseconds > 0  start_time = Basic.current_time_consensus();

        remaining := timeout_milliseconds;
        for * worker_info {
            // The timeout is for the whole shutdown, not for each worker, so
            // what is left of it shrinks as the loop goes.
            if remaining > 0 {
                elapsed := Basic.to_milliseconds(Basic.current_time_consensus() - start_time);
                remaining = cast(s32) (timeout_milliseconds - elapsed);
                if remaining < 0  remaining = 0;
            }

            if !thread_is_done(*it.thread, remaining)  all_done = false;
        }
    }

    if !all_done  return false;

    for * worker_info {
        thread_deinit(*it.thread);
        deinit_work_list(*it.available);
        deinit_work_list(*it.completed);
        Basic.free(it.work_steal_indices.data);
    }

    Basic.free(worker_info_data_to_free);
    return true;
}

// Queues one unit of work. Which worker gets it is a plain round robin.
add_work :: (group: *Thread_Group, work: *void, logging_name := "") {
    assert(group.worker_info.count >= 0);

    entry := Basic.New(Thread_Group.Work_Entry, true,, group.allocator);

    entry.work         = work;
    entry.logging_name = logging_name;
    entry.issue_time   = get_time(group);

    thread_index := group.next_worker_index;
    group.next_worker_index += 1;
    if group.next_worker_index >= group.worker_info.count  group.next_worker_index = 0;

    entry.work_list_index = thread_index;

    add_work(*group.worker_info[thread_index].available, entry);

    // The extra spaces line this up with the other log lines below.
    if group.logging  Basic.log("'%' added     work '%' to the available list at time %.\n", group.name, entry.logging_name, entry.issue_time);
}

// Everything finished since the last call, in an array of temporary storage.
get_completed_work :: (group: *Thread_Group) -> [] *void {
    results: [..] *void;
    results.allocator = Basic.__temporary_allocator;

    for * info: group.worker_info {
        list := *info.completed;

        // The list is taken whole under the lock and walked outside it, so a
        // worker is never kept waiting while this one frees nodes.
        count:     s32;
        completed: *Work_Entry;
        {
            lock(*list.mutex);
            count     = list.count;
            completed = list.first;

            if list.first {
                list.first = null;
                list.last  = null;
                list.count = 0;
            }

            unlock(*list.mutex);
        }

        if !completed  continue;

        Basic.array_reserve(*results, results.count + count);
        old_count := results.count;

        while completed {
            array_add(*results, completed.work);
            next := completed.next;

            if group.logging {
                now := get_time(group);
                Basic.log("'%' completed work '%' from thread % at time % (elapsed: %).\n", group.name, completed.logging_name, completed.thread_index, now, now - completed.issue_time);
            }

            Basic.free(completed,, group.allocator);
            completed = next;
        }

        assert(results.count == old_count + count);
    }

    if results.count && group.logging {
        plural := ifx results.count > 1 then "s" else "";
        Basic.log("'%' is returning % result% from get_completed_work().\n", group.name, results.count, plural);
    }

    return results;
}

#scope_file

init_work_list :: (using list: *Work_List) {
    init(*semaphore);
    init(*mutex);
}

deinit_work_list :: (using list: *Work_List) {
    destroy(*semaphore);
    destroy(*mutex);
}

// Onto the end, so the queue is first-in first-out. The semaphore is signalled
// after the lock is dropped, so a woken worker does not immediately block on
// a mutex this thread still holds.
add_work :: (using list: *Work_List, entry: *Work_Entry) {
    lock(*mutex);

    if last {
        last.next = entry;
        last      = entry;
    } else {
        first = entry;
        last  = entry;
    }

    list.count += 1;

    unlock(*mutex);

    signal(*semaphore);
}

// Null when the queue turned out to be empty after all, which happens when
// another worker stole what this one was woken for.
get_work :: (using list: *Work_List) -> *Work_Entry {
    lock(*mutex);
    defer unlock(*mutex);

    if !first  return null;

    result := first;
    first   = result.next;
    if !first  last = null;

    list.count -= 1;
    return result;
}

// What every worker runs.
thread_group_run :: (thread: *Thread) -> s64 {
    info  := thread.worker_info;
    group := info.group;

    entry: *Work_Entry;
    while !group.should_exit {
        if !entry {
            wait_for(*info.available.semaphore);
            // Checked again here: the wake may have been `shutdown`'s rather
            // than a piece of work's, and the proc could take a long time.
            if group.should_exit  break;

            entry = get_work(*info.available);
        }

        if entry {
            entry.thread_index = thread.index;
            entry.next         = null;

            if group.logging  Basic.log("'%' assigned  work '%'   to thread % at time %.\n", group.name, entry.logging_name, thread.index, get_time(group));

            should_continue := Thread_Continue_Status.CONTINUE;
            if group.proc  should_continue = group.proc(group, thread, entry.work);

            add_work(*info.completed, entry);

            if should_continue == .STOP  break;
        }

        if info.work_steal_indices.count {
            if group.should_exit  break;

            entry = get_work(*info.available);
            if entry {
                // Taken without going through the semaphore, so its count is
                // brought down by hand to keep the two in step.
                wait_for(*info.available.semaphore);
            } else {
                for i: info.work_steal_indices {
                    entry = get_work(*group.worker_info[i].available);
                    if entry {
                        if group.logging  Basic.log("'%' stole     work '%'  for thread % (worker %) at time % from worker %.\n", group.name, entry.logging_name, thread.index, info.worker_index, get_time(group), i);
                        // The other worker's semaphore is left alone: taking
                        // from it here would race with its own wait.
                        break;
                    }
                }
            }
        } else {
            entry = null;
        }
    }

    return 0;
}

get_time :: (group: *Thread_Group) -> float64 {
    if group.time_proc  return group.time_proc();
    return Basic.seconds_since_init();
}
