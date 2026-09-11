//
// Radix sort: no comparisons, eleven bits at a time.
//
// A comparison sort cannot beat O(n log n); a radix sort is O(n · passes) and
// for 64-bit keys that is six passes of counting and scattering. It wins on
// large arrays of numbers and loses on small ones, where the extra buffer and
// the counting passes cost more than a quicksort.
//
// Stable, which is what makes sorting by one key and then another work.
//

#import "Basic";

BITS_PER_PASS :: 11;
BUCKETS       :: 1 << BITS_PER_PASS;
PASSES        :: 6;   // 6 * 11 = 66 >= 64

// Sorts unsigned keys ascending, using `scratch` as the second buffer. The
// result is in `values` either way: an odd number of passes copies back.
radix_sort :: (values: [] u64, scratch: [] u64) -> bool {
    if values.count < 2  return true;
    if scratch.count < values.count  return false;

    counts: [BUCKETS] s64;
    source := values;
    target := scratch;

    for pass: 0..PASSES - 1 {
        shift := cast(u64) (pass * BITS_PER_PASS);
        if shift >= 64  break;

        for * counts  it.* = 0;
        for source  counts[(it >> shift) & (BUCKETS - 1)] += 1;

        // A pass whose keys all land in one bucket changes nothing, so it is
        // skipped rather than copied back and forth.
        if counts[(source[0] >> shift) & (BUCKETS - 1)] == source.count  continue;

        // Prefix sums turn counts into where each bucket starts.
        running := 0;
        for * counts {
            here := it.*;
            it.* = running;
            running += here;
        }

        for source {
            bucket := (it >> shift) & (BUCKETS - 1);
            target[counts[bucket]] = it;
            counts[bucket] += 1;
        }

        temporary := source;
        source = target;
        target = temporary;
    }

    // If the last pass left the data in the scratch buffer, bring it home.
    if source.data != values.data {
        memcpy(values.data, source.data, values.count * size_of(u64));
    }
    return true;
}

// Signed keys, by flipping the sign bit so that the unsigned order matches the
// signed one: negatives have the top bit set and must sort first.
radix_sort :: (values: [] s64, scratch: [] u64) -> bool {
    if values.count < 2  return true;
    if scratch.count < values.count  return false;

    keys: [] u64;
    keys.data = cast(*u64) values.data;
    keys.count = values.count;

    SIGN :: 0x8000_0000_0000_0000;
    for * keys  it.* ^= SIGN;
    ok := radix_sort(keys, scratch);
    for * keys  it.* ^= SIGN;
    return ok;
}

// Allocates its own scratch buffer, for a caller that does not want to.
radix_sort :: (values: [] u64) -> bool {
    scratch := NewArray(values.count, u64,, temp);
    if !scratch.data && values.count  return false;
    return radix_sort(values, scratch);
}
