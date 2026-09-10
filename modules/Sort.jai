//
// Sorting.
//
// `quick_sort` is the one to reach for; it takes either a comparison procedure
// returning a negative/zero/positive number, or a key procedure whose results
// are compared with `<`. Neither allocates.
//
// The pivot is the median of the first, middle and last element, which keeps
// already-sorted input off the quadratic path, and the smaller side is
// recursed into first so the stack stays logarithmic.
//

quick_sort :: (array: [] $T, compare: (T, T) -> $R) {
    if array.count < 2  return;
    sort_range(array, 0, array.count - 1, compare);
}

// A key rather than a comparison: `quick_sort(people, x => x.age)`.
quick_sort :: (array: [] $T, key: (T) -> $K) {
    compare :: (a: T, b: T) -> s64 #expand {
        ka := key(a);
        kb := key(b);
        if ka < kb  return -1;
        if kb < ka  return  1;
        return 0;
    }
    quick_sort(array, compare);
}

// Stable, and useful where the comparison is expensive enough that the
// quadratic worst case matters less than not moving equal elements past one
// another. Sorts in place using `scratch` when one is given.
insertion_sort :: (array: [] $T, compare: (T, T) -> $R) {
    index := 1;
    while index < array.count {
        value := array[index];
        at := index - 1;
        while at >= 0 && compare(array[at], value) > 0 {
            array[at + 1] = array[at];
            at -= 1;
        }
        array[at + 1] = value;
        index += 1;
    }
}

// Where `value` sits, or -1. The array must already be sorted by the same
// comparison.
binary_search :: (array: [] $T, value: T, compare: (T, T) -> $R) -> s64 {
    low := 0;
    high := array.count - 1;
    while low <= high {
        middle := low + (high - low) / 2;
        order := compare(array[middle], value);
        if order == 0  return middle;
        if order < 0   low = middle + 1;
        else           high = middle - 1;
    }
    return -1;
}

// Where `value` would be inserted to keep the array sorted: the first index
// whose element does not compare less than it.
lower_bound :: (array: [] $T, value: T, compare: (T, T) -> $R) -> s64 {
    low := 0;
    high := array.count;
    while low < high {
        middle := low + (high - low) / 2;
        if compare(array[middle], value) < 0  low = middle + 1;
        else                                  high = middle;
    }
    return low;
}

compare_integers :: (a: $T, b: T) -> s64 {
    if a < b  return -1;
    if b < a  return  1;
    return 0;
}

#scope_module

// Below this a partition is finished off by insertion sort, which beats
// quicksort's bookkeeping on short runs.
SMALL :: 12;

sort_range :: (array: [] $T, first: s64, last: s64, compare: (T, T) -> $R) {
    low := first;
    high := last;
    while low < high {
        if high - low < SMALL {
            // A view of the partition, built here rather than through Basic:
            // sorting should not drag a module in behind it.
            run: [] T;
            run.data = array.data + low;
            run.count = high - low + 1;
            insertion_sort(run, compare);
            return;
        }

        pivot := median_of_three(array, low, high, compare);
        left := low;
        right := high;
        while left <= right {
            while compare(array[left], pivot)  < 0  left += 1;
            while compare(array[right], pivot) > 0  right -= 1;
            if left <= right {
                temporary := array[left];
                array[left] = array[right];
                array[right] = temporary;
                left += 1;
                right -= 1;
            }
        }

        // Recurse into the smaller side and loop on the larger, so the depth
        // is logarithmic however the pivots fall.
        if right - low < high - left {
            sort_range(array, low, right, compare);
            low = left;
        } else {
            sort_range(array, left, high, compare);
            high = right;
        }
    }
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
