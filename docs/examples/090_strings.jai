//
// Strings: a count and a pointer at bytes nobody owns (**L§3.4**).
//

#import "Basic";
String :: #import "String";

main :: () {
    s := "orange juice";
    print("% is % bytes\n", s, s.count);

    // Indexing gives a byte, not a character.
    print("first byte: % (%)\n", s[0], cast(s64) s[0]);

    // Slicing makes another string over the same bytes: no copy, no
    // allocation.
    print("%|%\n", String.slice(s, 0, 6), String.slice(s, 7, 5));

    print("% % %\n",
        String.contains(s, "juice"),
        String.starts_with(s, "orange"),
        String.ends_with(s, "juice"));

    // Splitting and joining allocate, so they say where from.
    parts := String.split(s, " ",, allocator = temp);
    print("% parts: %\n", parts.count, parts);
    print("%\n", String.join(..parts, separator = "-",, allocator = temp));

    print("%\n", String.to_upper_copy(s,, allocator = temp));

    // A builder is how a string is grown without reallocating at every step.
    builder: String_Builder;
    for 1..5  print_to_builder(*builder, "%,", it);
    print("%\n", builder_to_string(*builder,, allocator = temp));

    // Comparing strings compares their bytes.
    print("% %\n", "abc" == "abc", "abc" == "abd");

    reset_temporary_storage();
}
