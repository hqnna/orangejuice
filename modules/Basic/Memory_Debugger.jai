//
// The memory debugger: what `#import "Basic"()(MEMORY_DEBUGGER=true)` turns
// on (**L§10.4**). Every allocation `alloc` and `realloc` hand out is recorded
// with the stack trace it was made from, `free` forgets it, and a leak report
// is what is still recorded, grouped by where it was allocated.
//
// The bookkeeping lives in the C heap rather than behind `context.allocator`,
// so recording an allocation never allocates through the thing it records.
// Allocations from the temporary allocator are not recorded: resetting
// temporary storage is how those are given back.
//

#scope_export

Packed_Stack_Trace :: struct {
    nodes: [] Stack_Trace_Node;
    hash:  u64;
}

Leak_Summary :: struct {
    count: s64;
    bytes: s64;
    allocations_since_last_visualizer_update: s64;
    bytes_allocated_since_last_visualizer_update: s64;
    alloc_site_trace: *Packed_Stack_Trace;
    allocator_proc: Allocator_Proc;
    group_info: *void;
}

Leak_Report :: struct {
    summaries:        [] Leak_Summary;
    sorted_summaries: [] *Leak_Summary;

    total_bytes:         s64;
    total_count:         s64;
    non_leak_bytes:      s64;
    non_leak_count:      s64;
}

// Everything still allocated, one summary per allocation site, largest first.
make_leak_report :: () -> Leak_Report {
    report: Leak_Report;
    #if MEMORY_DEBUGGER {
        groups := cast(*Leak_Summary) debugger_raw_alloc(size_of(Leak_Summary) * (debugger.count + 1));
        group_count := 0;
        for i: 0..debugger.capacity - 1 {
            slot := *debugger.slots[i];
            if slot.memory == null || slot.memory == DEBUGGER_TOMBSTONE  continue;
            if slot.non_leak {
                report.non_leak_bytes += slot.size;
                report.non_leak_count += 1;
                continue;
            }
            found: *Leak_Summary;
            for g: 0..group_count - 1 {
                candidate := groups + g;
                if candidate.alloc_site_trace == slot.trace && candidate.allocator_proc == slot.allocator_proc {
                    found = candidate;
                    break;
                }
            }
            if !found {
                found = groups + group_count;
                group_count += 1;
                found.* = .{};
                found.alloc_site_trace = slot.trace;
                found.allocator_proc = slot.allocator_proc;
            }
            found.count += 1;
            found.bytes += slot.size;
            report.total_bytes += slot.size;
            report.total_count += 1;
        }

        report.summaries.data = groups;
        report.summaries.count = group_count;
        sorted := cast(**Leak_Summary) debugger_raw_alloc(size_of(*Leak_Summary) * (group_count + 1));
        for g: 0..group_count - 1  sorted[g] = groups + g;
        for a: 1..group_count - 1 {
            b := a;
            while b > 0 && sorted[b - 1].bytes < sorted[b].bytes {
                swap := sorted[b - 1];
                sorted[b - 1] = sorted[b];
                sorted[b] = swap;
                b -= 1;
            }
        }
        report.sorted_summaries.data = sorted;
        report.sorted_summaries.count = group_count;
    }
    return report;
}

log_leak_report :: (report: Leak_Report, verbose := false) {
    builder: String_Builder;
    for report.sorted_summaries {
        print_to_builder(*builder, "----- % in % -----\n", bytes_phrase(it.bytes), allocation_phrase(it.count));
        trace := it.alloc_site_trace;
        if trace {
            widest := 0;
            for node: trace.nodes  if node.info && node.info.name.count > widest  widest = node.info.name.count;
            for node: trace.nodes {
                if !node.info  continue;
                append(*builder, node.info.name);
                for 1..widest - node.info.name.count + 2  append(*builder, " ");
                print_to_builder(*builder, "%:%\n", node.info.location.fully_pathed_filename, node.line_number);
            }
        }
        append(*builder, "\n");
    }
    if report.sorted_summaries.count  append(*builder, "\n");
    print_to_builder(*builder, "Total: % in %.\n\n", bytes_phrase(report.total_bytes), allocation_phrase(report.total_count));
    print_to_builder(*builder, "Marked as non-leaks: % in %.\n", bytes_phrase(report.non_leak_bytes), allocation_phrase(report.non_leak_count));
    log("%", builder_to_string(*builder,, temp));
}

// Gives back what `make_leak_report` allocated for the report itself.
deinit :: (report: *Leak_Report) {
    #if MEMORY_DEBUGGER {
        debugger_raw_free(report.summaries.data);
        debugger_raw_free(report.sorted_summaries.data);
    }
    report.* = .{};
}

report_memory_leaks :: () {
    #if MEMORY_DEBUGGER {
        report := make_leak_report();
        log_leak_report(report);
        deinit(*report);
    }
}

// An allocation that is meant to live for the rest of the program, and so is
// not a leak: it is counted apart rather than reported.
mark_as_non_leak :: (memory: *void) {
    #if MEMORY_DEBUGGER {
        slot := debugger_find(memory);
        if slot  slot.non_leak = true;
    }
}

#scope_module

bytes_phrase :: (bytes: s64) -> string {
    return tprint("% byte%", bytes, ifx bytes == 1 then "" else "s");
}

allocation_phrase :: (count: s64) -> string {
    return tprint("% allocation%", count, ifx count == 1 then "" else "s");
}

Debugger_Slot :: struct {
    memory:         *void;
    size:           s64;
    trace:          *Packed_Stack_Trace;
    allocator_proc: Allocator_Proc;
    non_leak:       bool;
}

Debugger_State :: struct {
    slots:    *Debugger_Slot;
    capacity: s64;
    count:    s64;
    used:     s64;

    traces:         **Packed_Stack_Trace;
    trace_capacity: s64;
    trace_count:    s64;
}

debugger: Debugger_State;

DEBUGGER_TOMBSTONE :: cast(*void) 1;

debugger_raw_alloc :: (size: s64) -> *void {
    memory := Libc.malloc(cast(u64) size);
    memset(memory, 0, size);
    return memory;
}

debugger_raw_free :: (memory: *void) {
    if memory  Libc.free(memory);
}

debugger_hash_pointer :: (memory: *void) -> u64 {
    x := cast(u64) memory;
    x ^= x >> 33;
    x *= 0xff51_afd7_ed55_8ccd;
    x ^= x >> 33;
    return x;
}

debugger_find :: (memory: *void) -> *Debugger_Slot {
    if !debugger.capacity  return null;
    mask := cast(u64) (debugger.capacity - 1);
    index := debugger_hash_pointer(memory) & mask;
    while true {
        slot := *debugger.slots[index];
        if slot.memory == null    return null;
        if slot.memory == memory  return slot;
        index = (index + 1) & mask;
    }
    return null;
}

debugger_grow :: () {
    old := debugger.slots;
    old_capacity := debugger.capacity;
    debugger.capacity = ifx old_capacity then old_capacity * 2 else 1024;
    debugger.slots = cast(*Debugger_Slot) debugger_raw_alloc(size_of(Debugger_Slot) * debugger.capacity);
    debugger.count = 0;
    debugger.used = 0;
    for i: 0..old_capacity - 1 {
        slot := old[i];
        if slot.memory == null || slot.memory == DEBUGGER_TOMBSTONE  continue;
        debugger_insert(slot);
    }
    debugger_raw_free(old);
}

debugger_insert :: (entry: Debugger_Slot) {
    if (debugger.used + 1) * 4 >= debugger.capacity * 3  debugger_grow();
    mask := cast(u64) (debugger.capacity - 1);
    index := debugger_hash_pointer(entry.memory) & mask;
    while true {
        slot := *debugger.slots[index];
        if slot.memory == null || slot.memory == DEBUGGER_TOMBSTONE {
            if slot.memory == null  debugger.used += 1;
            slot.* = entry;
            debugger.count += 1;
            return;
        }
        if slot.memory == entry.memory {
            slot.* = entry;
            return;
        }
        index = (index + 1) & mask;
    }
}

// One copy of each distinct trace, so that a summary can tell two allocation
// sites apart by comparing pointers.
debugger_intern_trace :: (node: *Stack_Trace_Node) -> *Packed_Stack_Trace {
    hash: u64 = 0xcbf2_9ce4_8422_2325;
    count := 0;
    walk := node;
    while walk {
        hash = (hash ^ cast(u64) walk.info) * 0x100_0000_01b3;
        hash = (hash ^ cast(u64) walk.line_number) * 0x100_0000_01b3;
        count += 1;
        walk = walk.next;
    }

    for i: 0..debugger.trace_count - 1 {
        existing := debugger.traces[i];
        if existing.hash != hash || existing.nodes.count != count  continue;
        same := true;
        walk = node;
        for existing.nodes {
            if it.info != walk.info || it.line_number != walk.line_number { same = false; break; }
            walk = walk.next;
        }
        if same  return existing;
    }

    trace := cast(*Packed_Stack_Trace) debugger_raw_alloc(size_of(Packed_Stack_Trace));
    trace.hash = hash;
    trace.nodes.count = count;
    trace.nodes.data = cast(*Stack_Trace_Node) debugger_raw_alloc(size_of(Stack_Trace_Node) * (count + 1));
    walk = node;
    for i: 0..count - 1 {
        trace.nodes[i] = walk.*;
        trace.nodes[i].next = ifx i + 1 < count then *trace.nodes[i + 1] else null;
        walk = walk.next;
    }

    if debugger.trace_count == debugger.trace_capacity {
        capacity := ifx debugger.trace_capacity then debugger.trace_capacity * 2 else 256;
        grown := cast(**Packed_Stack_Trace) debugger_raw_alloc(size_of(*Packed_Stack_Trace) * capacity);
        if debugger.trace_count  memcpy(grown, debugger.traces, size_of(*Packed_Stack_Trace) * debugger.trace_count);
        debugger_raw_free(debugger.traces);
        debugger.traces = grown;
        debugger.trace_capacity = capacity;
    }
    debugger.traces[debugger.trace_count] = trace;
    debugger.trace_count += 1;
    return trace;
}

debugger_note_allocation :: (memory: *void, size: s64, proc: Allocator_Proc, trace: *Stack_Trace_Node) {
    if !memory || proc == temporary_allocator_proc  return;
    entry: Debugger_Slot;
    entry.memory = memory;
    entry.size = size;
    entry.trace = debugger_intern_trace(trace);
    entry.allocator_proc = proc;
    debugger_insert(entry);
}

debugger_note_free :: (memory: *void) {
    slot := debugger_find(memory);
    if !slot  return;
    slot.memory = DEBUGGER_TOMBSTONE;
    debugger.count -= 1;
}
