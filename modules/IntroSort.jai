//
// Introsort: quicksort that refuses to go quadratic.
//
// Quicksort is fast on average and O(n²) on inputs that defeat its pivot
// choice — which an adversary can construct deliberately. Introsort watches
// its own recursion depth and switches to heapsort once it exceeds 2·log2(n),
// so the worst case is O(n log n) while the common case stays quicksort.
//

#import "Basic";
#import "Bit_Operations";

intro_sort :: (array: [] $T, compare: (T, T) -> $R) {
    if array.count < 2  return;
    // 2·floor(log2(n)) is the usual limit: deep enough that a well-behaved
    // input never reaches it, shallow enough to cap the damage.
    // `bit_scan_reverse` is one past the top bit, so this is 2*floor(log2 n).
    limit := 2 * (bit_scan_reverse(cast(u64) array.count) - 1);
    sort_range(array, 0, array.count - 1, limit, compare);
}

#scope_module

SMALL :: 16;

sort_range :: (array: [] $T, first: s64, last: s64, depth_limit: s64, compare: (T, T) -> $R) {
    low := first;
    high := last;
    limit := depth_limit;

    while high - low > SMALL {
        if limit <= 0 {
            // Out of patience with quicksort; heapsort the rest.
            heap_sort_range(array, low, high, compare);
            return;
        }
        limit -= 1;

        pivot := median_of_three(array, low, high, compare);
        left := low;
        right := high;
        while left <= right {
            while compare(array[left], pivot)  < 0  left += 1;
            while compare(array[right], pivot) > 0  right -= 1;
            if left <= right {
                swap(*array[left], *array[right]);
                left += 1;
                right -= 1;
            }
        }

        // Recurse into the smaller side, loop on the larger.
        if right - low < high - left {
            sort_range(array, low, right, limit, compare);
            low = left;
        } else {
            sort_range(array, left, high, limit, compare);
            high = right;
        }
    }

    // A nearly-sorted short run is what insertion sort is best at.
    insertion_range(array, low, high, compare);
}

swap :: (a: *$T, b: *T) {
    t := a.*;
    a.* = b.*;
    b.* = t;
}

median_of_three :: (array: [] $T, low: s64, high: s64, compare: (T, T) -> $R) -> T {
    middle := low + (high - low) / 2;
    a := array[low];
    b := array[middle];
    c := array[high];
    if compare(a, b) > 0 { t := a; a = b; b = t; }
    if compare(b, c) > 0 { b = c; }
    if compare(a, b) > 0 { b = a; }
    return b;
}

insertion_range :: (array: [] $T, low: s64, high: s64, compare: (T, T) -> $R) {
    index := low + 1;
    while index <= high {
        value := array[index];
        at := index - 1;
        while at >= low && compare(array[at], value) > 0 {
            array[at + 1] = array[at];
            at -= 1;
        }
        array[at + 1] = value;
        index += 1;
    }
}

// Heapsort over [low, high]: build a max-heap, then repeatedly move the
// largest to the end. O(n log n) whatever the input looks like.
heap_sort_range :: (array: [] $T, low: s64, high: s64, compare: (T, T) -> $R) {
    count := high - low + 1;
    if count < 2  return;

    node := count / 2 - 1;
    while node >= 0 {
        sift_down(array, low, node, count, compare);
        node -= 1;
    }

    end := count - 1;
    while end > 0 {
        swap(*array[low], *array[low + end]);
        sift_down(array, low, 0, end, compare);
        end -= 1;
    }
}

sift_down :: (array: [] $T, low: s64, start: s64, count: s64, compare: (T, T) -> $R) {
    root := start;
    while true {
        child := 2 * root + 1;
        if child >= count  break;
        // Take the larger child, so the heap property is restored in one pass.
        if child + 1 < count && compare(array[low + child], array[low + child + 1]) < 0 {
            child += 1;
        }
        if compare(array[low + root], array[low + child]) >= 0  break;
        swap(*array[low + root], *array[low + child]);
        root = child;
    }
}

#scope_export

// The reference spells it without the underscore in some places; both names
// are the same procedure.
introsort :: (array: [] $T, compare: (T, T) -> $R) {
    intro_sort(array, compare);
}

// A range of the array rather than all of it, which is what a caller sorting
// part of a buffer in place wants. `end` is one past the last element.
intro_sort :: (array: [] $T, begin: s64, end: s64, compare: (T, T) -> $R) {
    first := begin;
    if first < 0  first = 0;
    last := end;
    if last > array.count  last = array.count;
    if last - first < 2  return;
    intro_sort(array_view(array, first, last - first), compare);
}

// Straight insertion sort. Quadratic, and the fastest thing there is on a
// nearly-sorted or very short run — which is why `intro_sort` finishes with it.
insertion_sort :: inline (array: [] $T, f: (T, T) -> int) {
    insertion_sort(array, 0, array.count, f);
}

insertion_sort :: (array: [] $T, begin: int, end: int, f: (T, T) -> int) {
    first := max(begin, 0);
    last := min(end, array.count);
    for index: first + 1..last - 1 {
        value := array[index];
        at := index - 1;
        while at >= first && f(array[at], value) > 0 {
            array[at + 1] = array[at];
            at -= 1;
        }
        array[at + 1] = value;
    }
}
