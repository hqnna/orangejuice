//
// Calling C, and being called by it (**L§12**).
//

#import "Basic";
#import "POSIX";

// A `#foreign` header is a declaration with no body: the linker supplies it.
// The library it comes from is named by the `#foreign` that binds it.
libc :: #library,system "libc";

strlen  :: (s: *u8) -> u64 #foreign libc;
abs_c   :: (n: s32) -> s32 #foreign libc "abs";
qsort_c :: (base: *void, count: u64, size: u64, compare: (*void, *void) -> s32 #c_call) #foreign libc "qsort";

// A `#c_call` procedure takes no context, because C has none to give it. One
// that needs to allocate or print pushes a context of its own first.
compare_ints :: (left: *void, right: *void) -> s32 #c_call {
    a := (cast(*s32) left).*;
    b := (cast(*s32) right).*;
    if a < b  return -1;
    if a > b  return 1;
    return 0;
}

main :: () {
    // A Jai `string` is a count and a pointer and is *not* NUL-terminated, so
    // one handed to C is terminated on the way out.
    text := "orange";
    c_text := temp_c_string(text);
    print("strlen(\"%\") = %\n", text, strlen(c_text));

    print("abs(-17) = %\n", abs_c(-17));

    numbers := s32.[5, 3, 9, 1, 7];
    qsort_c(numbers.data, cast(u64) numbers.count, size_of(s32), compare_ints);
    print("%\n", numbers);

    reset_temporary_storage();
}
