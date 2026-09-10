//
// Relative pointers: an offset from the pointer's own address.
//
// A structure holding absolute pointers cannot be written to disk, mapped
// somewhere else, or memcpy'd to another address — every pointer in it would
// have to be found and fixed. A relative pointer stores the distance from its
// own location instead, so the whole block stays valid wherever it lands.
//
// The cost is a smaller reach: a `Relative_Pointer(T, s32)` can address ±2GB
// around itself, which is why the offset type is a parameter.
//

#import "Basic";

// `Offset_Type` is usually `s32`; `s16` is worth it for a structure with a
// great many small links, and `s64` gives up the space saving for full reach.
Relative_Pointer :: struct (Points_To: Type, Offset_Type: Type = s32) {
    // 0 means null. That costs the ability to point at the field itself,
    // which nothing wants to do.
    offset: Offset_Type;
}

// Points `pointer` at `target`. Fails if the distance does not fit the offset
// type — silently truncating would produce a pointer into nowhere.
set :: (pointer: *Relative_Pointer($T, $O), target: *T) -> bool {
    if !target {
        pointer.offset = 0;
        return true;
    }

    distance := cast(s64) target - cast(s64) pointer;
    if distance == 0  return false;

    narrowed := cast(O) distance;
    if cast(s64) narrowed != distance  return false;
    pointer.offset = narrowed;
    return true;
}

get :: (pointer: *Relative_Pointer($T, $O)) -> *T {
    if pointer.offset == 0  return null;
    return cast(*T) (cast(s64) pointer + cast(s64) pointer.offset);
}

is_null :: (pointer: *Relative_Pointer($T, $O)) -> bool {
    return pointer.offset == 0;
}

set_null :: (pointer: *Relative_Pointer($T, $O)) {
    pointer.offset = 0;
}

// A relative view: a count and a relative pointer at the elements, so an array
// inside a relocatable block stays valid too.
Relative_Array :: struct (Element: Type, Offset_Type: Type = s32) {
    count: s64;
    data:  Relative_Pointer(Element, Offset_Type);
}

set :: (array: *Relative_Array($T, $O), elements: [] T) -> bool {
    array.count = elements.count;
    if !elements.count {
        set_null(*array.data);
        return true;
    }
    return set(*array.data, elements.data);
}

get :: (array: *Relative_Array($T, $O)) -> [] T {
    result: [] T;
    elements := get(*array.data);
    if !elements  return result;
    result.data = elements;
    result.count = array.count;
    return result;
}
