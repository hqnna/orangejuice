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
// The parameter is `value_types: .. Type`: every argument the instantiation
// was given gathers into a `[N] Type` the body can index and iterate
// (**L§8.5**).
//

#import "Basic";

Tagged_Union :: struct (value_types: .. Type) {
    // The type currently held, or `void` when it holds nothing — which is
    // what the reference uses too, rather than an index.
    tag: Type = void;

    bytes: [NUM_VALUE_BYTES] u8;

    NUM_VALUE_BYTES :: #run biggest_of(value_types);
}

// Sets the tag to `type_of(value)` and copies it in. Setting a type the union
// was not declared with is a compile-time error.
set :: (using union_value: *Tagged_Union, value: $V) {
    #assert(#run holds(value_types, V), "That type is not one this union was declared with.");
    union_value.tag = V;
    memcpy(union_value.bytes.data, *value, size_of(V));
}

// The value, if that is what it currently holds. `ok` is false otherwise,
// which is the whole point of keeping the tag.
get :: (using union_value: *Tagged_Union, $V: Type) -> (value: V, ok: bool) {
    #assert(#run holds(value_types, V), "That type is not one this union was declared with.");
    empty: V;
    if union_value.tag != V  return empty, false;
    result: V;
    memcpy(*result, union_value.bytes.data, size_of(V));
    return result, true;
}

// Whether it currently holds a `V`. Named as the reference names it.
isa :: (union_value: *Tagged_Union, $V: Type) -> bool {
    return union_value.tag == V;
}

is_empty :: (union_value: *Tagged_Union) -> bool {
    return union_value.tag == void;
}

reset :: (union_value: *Tagged_Union) {
    union_value.tag = void;
}

#scope_module

// The widest of the types the union was declared with, which is how much
// storage it needs. One byte at least, so that a union of nothing still has an
// address of its own.
biggest_of :: (types: [] Type) -> s64 {
    biggest := 1;
    for types {
        info := cast(*Type_Info) it;
        if info.runtime_size > biggest  biggest = info.runtime_size;
    }
    return biggest;
}

// Whether `wanted` is one of the types the union was declared with.
holds :: (types: [] Type, wanted: Type) -> bool {
    for types  if it == wanted  return true;
    return false;
}
