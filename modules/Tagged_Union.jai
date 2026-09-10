//
// Tagged_Union: a value that is one of several types, and knows which.
//
// A bare `union` does not remember what was last written to it, so reading the
// wrong member is undefined. This pairs the storage with a tag naming the type
// currently held, and checks it on the way out — which turns undefined
// behaviour into a question the caller can ask.
//
//     u: Tagged_Union(int, float64, Vector3);
//     set(*u, 42);
//     value, ok := get(*u, int);
//
// The reference declares this as `struct (value_types: .. Type)`. orangejuice
// does not support variadic type parameters on a struct yet (`docs/spec.md`
// §10.1), so this takes a fixed four, defaulted to `void`. The call syntax is
// the same for up to four types; a fifth is a compile-time error rather than
// something that silently misbehaves.
//

#import "Basic";

Tagged_Union :: struct (A: Type, B: Type = void, C: Type = void, D: Type = void) {
    // The type currently held, or `void` when it holds nothing — which is
    // what the reference uses too, rather than an index.
    tag: Type = void;

    bytes: [NUM_VALUE_BYTES] u8;

    NUM_VALUE_BYTES :: #run biggest_of(A, B, C, D);
}

// Sets the tag to `type_of(value)` and copies it in. Setting a type the union
// was not declared with is a compile-time error.
set :: (union_value: *Tagged_Union($A, $B, $C, $D), value: $V) {
    #assert(V == A || V == B || V == C || V == D, "That type is not one this union was declared with.");
    union_value.tag = V;
    memcpy(union_value.bytes.data, *value, size_of(V));
}

// The value, if that is what it currently holds. `ok` is false otherwise,
// which is the whole point of keeping the tag.
get :: (union_value: *Tagged_Union($A, $B, $C, $D), $V: Type) -> (value: V, ok: bool) {
    empty: V;
    if union_value.tag != V  return empty, false;
    result: V;
    memcpy(*result, union_value.bytes.data, size_of(V));
    return result, true;
}

// Whether it currently holds a `V`. Named as the reference names it.
isa :: (union_value: *Tagged_Union($A, $B, $C, $D), $V: Type) -> bool {
    return union_value.tag == V;
}

is_empty :: (union_value: *Tagged_Union($A, $B, $C, $D)) -> bool {
    return union_value.tag == void;
}

reset :: (union_value: *Tagged_Union($A, $B, $C, $D)) {
    union_value.tag = void;
}

#scope_module

// `void` stands for "no type here", so it contributes nothing to the size.
biggest_of :: (a: Type, b: Type, c: Type, d: Type) -> s64 {
    biggest := 1;
    for t: Type.[a, b, c, d] {
        if t == void  continue;
        info := cast(*Type_Info) t;
        if info.runtime_size > biggest  biggest = info.runtime_size;
    }
    return biggest;
}
