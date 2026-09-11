//
// `$` marks a parameter whose *value* or *type* the call site decides, and the
// header is solved again with what it found (**L§9**).
//

#import "Basic";

// `$T` is bound by the argument's type. One body, one procedure generated per
// type actually used.
largest :: (values: [] $T) -> T {
    best := values[0];
    for values  if it > best  best = it;
    return best;
}

// `$N` binds to a *length*, read off the argument's own type.
middle :: (array: [$N] $T) -> T {
    #assert N > 0;
    return array[N / 2];
}

// A polymorphic struct bakes down to one type per argument set.
Pair :: struct (First: Type, Second: Type) {
    first:  First;
    second: Second;
}

swap :: (pair: Pair($A, $B)) -> Pair(B, A) {
    result: Pair(B, A);
    result.first = pair.second;
    result.second = pair.first;
    return result;
}

// `$$x` bakes a *value* the call site passed as a constant, so `count` is
// known while the body is compiled.
repeat :: (text: string, $$count: int) -> string {
    builder: String_Builder;
    for 1..count  append(*builder, text);
    return builder_to_string(*builder,, allocator = temp);
}

main :: () {
    print("% %\n", largest(int.[3, 9, 4]), largest(float.[0.5, 0.25]));

    print("%\n", middle(int.[1, 2, 3, 4, 5]));

    p: Pair(int, string);
    p.first = 1;
    p.second = "one";
    print("% -> %\n", p, swap(p));

    print("%\n", repeat("ab", 3));

    // The type a polymorph was solved with is a value like any other.
    print("largest takes %\n", type_of(largest(int.[1])));

    reset_temporary_storage();
}
