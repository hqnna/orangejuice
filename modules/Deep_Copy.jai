//
// Deep copy: a value and everything it points at.
//
// Assigning a struct copies its bytes, so two copies end up sharing whatever
// their pointers and views point at. Freeing one then leaves the other holding
// freed memory. This walks the type table and copies the pointed-at storage
// too, so the result owns everything it refers to.
//
// Two things to know. Cycles are not handled: a structure that points back at
// itself recurses until the depth limit stops it. And `free_deep` frees every
// string and every view it walks, so assigning a literal — or anything else it
// does not own — into a deep copy and then freeing it will free storage the
// program does not own. A deep copy is meant to be filled by copying, not
// edited with literals afterwards.
//

#import "Basic";

MAX_DEPTH :: 64;

// A copy of `value` whose pointers and views point at storage from
// `allocator`. `free_deep` gives it all back.
deep_copy :: (value: $T, allocator := context.allocator) -> T {
    result := value;
    copy_in_place(*result, type_info(T), allocator, 0);
    return result;
}

// Frees everything a deep copy allocated. The value itself is the caller's,
// since it may be a local rather than something allocated.
free_deep :: (value: *$T, allocator := context.allocator) {
    free_in_place(value, type_info(T), allocator, 0);
}

#scope_module

// Walks `data` as a `info`-shaped value, replacing every pointer and view with
// one of its own.
copy_in_place :: (data: *void, info: *Type_Info, allocator: Allocator, depth: s64) {
    if !data || !info || depth >= MAX_DEPTH  return;

    if info.type == {
        case .STRING;
            text := cast(*string) data;
            text.* = copy_string(text.*, allocator);

        case .POINTER;
            pointer_info := cast(*Type_Info_Pointer) info;
            slot := cast(**void) data;
            if !slot.*  return;
            target := pointer_info.pointer_to;
            if !target || target.runtime_size <= 0  return;

            fresh := alloc(target.runtime_size, allocator);
            if !fresh  return;
            memcpy(fresh, slot.*, target.runtime_size);
            slot.* = fresh;
            // Whatever it points at may point at more.
            copy_in_place(fresh, target, allocator, depth + 1);

        case .STRUCT;
            structure := cast(*Type_Info_Struct) info;
            for * member: structure.members {
                if member.flags & .CONSTANT  continue;
                copy_in_place(cast(*u8) data + member.offset_in_bytes, member.type, allocator, depth + 1);
            }

        case .ARRAY;
            array_info := cast(*Type_Info_Array) info;
            element := array_info.element_type;
            if !element  return;

            if array_info.array_type == .FIXED {
                // The elements are already inside the value; only what they
                // point at needs copying.
                for index: 0..array_info.array_count - 1 {
                    copy_in_place(cast(*u8) data + index * element.runtime_size, element, allocator, depth + 1);
                }
                return;
            }

            // A view or a resizable array points at storage elsewhere, so the
            // elements themselves are copied as well.
            view := cast(*Array_View_64) data;
            if !view.count || !view.data  return;

            bytes := view.count * element.runtime_size;
            fresh := alloc(bytes, allocator);
            if !fresh  return;
            memcpy(fresh, view.data, bytes);
            view.data = cast(*u8) fresh;

            for index: 0..view.count - 1 {
                copy_in_place(cast(*u8) fresh + index * element.runtime_size, element, allocator, depth + 1);
            }

            // A resizable array now owns its storage, and must say which
            // allocator it came from or freeing it later would use the wrong
            // one.
            if array_info.array_type == .RESIZABLE {
                resizable := cast(*Resizable_Array) data;
                resizable.allocated = view.count;
                resizable.allocator = allocator;
            }

        case;
            // Everything else is bytes with nothing behind them.
    }
}

free_in_place :: (data: *void, info: *Type_Info, allocator: Allocator, depth: s64) {
    if !data || !info || depth >= MAX_DEPTH  return;

    if info.type == {
        case .STRING;
            text := cast(*string) data;
            if text.data  free(text.data, allocator);
            text.data = null;
            text.count = 0;

        case .POINTER;
            pointer_info := cast(*Type_Info_Pointer) info;
            slot := cast(**void) data;
            if !slot.*  return;
            // Deepest first, or the pointers to free would already be gone.
            free_in_place(slot.*, pointer_info.pointer_to, allocator, depth + 1);
            free(slot.*, allocator);
            slot.* = null;

        case .STRUCT;
            structure := cast(*Type_Info_Struct) info;
            for * member: structure.members {
                if member.flags & .CONSTANT  continue;
                free_in_place(cast(*u8) data + member.offset_in_bytes, member.type, allocator, depth + 1);
            }

        case .ARRAY;
            array_info := cast(*Type_Info_Array) info;
            element := array_info.element_type;
            if !element  return;

            if array_info.array_type == .FIXED {
                for index: 0..array_info.array_count - 1 {
                    free_in_place(cast(*u8) data + index * element.runtime_size, element, allocator, depth + 1);
                }
                return;
            }

            view := cast(*Array_View_64) data;
            if !view.data  return;
            for index: 0..view.count - 1 {
                free_in_place(view.data + index * element.runtime_size, element, allocator, depth + 1);
            }
            free(view.data, allocator);
            view.data = null;
            view.count = 0;

        case;
    }
}
