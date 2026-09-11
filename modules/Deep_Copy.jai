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
#import "Hash_Table";
#import "Hash";

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

// ------------------------------------------------------ the mapped form ---
//
// The same idea with a *pointer map*, which is what makes a structure whose
// pointers alias — two fields pointing at one node, or a node pointing back at
// its parent — come out with the same shape rather than with a copy per
// pointer or an endless recursion.

// What a copy does about strings and about pointers to structs.
Deep_Copy_Config :: struct {
    // Called instead of copying a string, for a caller that interns them or
    // keeps them somewhere of its own.
    string_mapper_proc: (input: string, data: *void) -> (output: string);
    string_mapper_data: *void;

    // False stops at a `*Struct`: the pointer is given storage of its own but
    // whatever *that* points at is left shared.
    follow_struct_pointers := true;
}

// Which copy each original piece of storage was given.
Pointer_Map :: struct {
    table: Table(*void, *void);
}

// How many pieces the last copies walked, and how long they took. Kept because
// the reference keeps them; only meaningful with the debug build.
helper_count := 0;
time_total: float64;

// A copy of what `value` points at, and of everything reachable from it.
// `preallocated_root` is storage for the root the caller already has;
// `pointer_map` is one to share across several calls, so that pointers between
// the values copied still line up afterwards.
Deep_Copy :: (value: *$T, config: Deep_Copy_Config, preallocated_root: *void = null, pointer_map: *Pointer_Map = null, loc := #caller_location) -> *T {
    if !value  return null;

    result := preallocated_root;
    if !result  result = alloc(size_of(T));

    memcpy(result, value, size_of(T));

    local_pointer_map: Pointer_Map;
    local_pointer_map.table.allocator = temp;
    defer deinit(*local_pointer_map.table);

    if !pointer_map  pointer_map = *local_pointer_map;

    copy_helper(result, config, type_info(T), pointer_map);

    return cast(*T) result;
}

#scope_module

// Whether a value of this type can hold anything that needs copying. Anything
// else is bytes, and walking it would be walking for nothing.
could_need_allocation :: (info: *Type_Info) -> bool {
    if info.type == {
        case .STRING;  return true;
        case .ARRAY;   return true;
        case .POINTER; return true;
        case .STRUCT;  return true;
        case;          return false;
    }
}

// The array element stride, which is the element size rounded up to eight.
get_array_stride :: (size: s64) -> s64 {
    return (size + 7) & ~7;
}

find :: (map: *Pointer_Map, old: *void) -> *void {
    value, found := table_find(*map.table, old);
    if !found  return null;
    return value;
}

// Only to be called once the map has been searched and came back empty.
add :: (map: *Pointer_Map, old: *void, new: *void) {
    table_add(*map.table, old, new);
}

copy_helper :: (data: *void, config: Deep_Copy_Config, info: *Type_Info, pointer_map: *Pointer_Map, debug_name := "") {
    helper_count += 1;

    if info.type == {
        case .STRING;
            s := cast(*string) data;
            if !s.data  return;

            mapper := config.string_mapper_proc;
            if mapper {
                s.* = mapper(s.*, config.string_mapper_data);
                return;
            }

            // Two strings sharing bytes keep sharing them.
            if find(pointer_map, s.data)  return;

            copied := copy_string(s.*);
            add(pointer_map, s.data, copied.data);
            s.* = copied;

        case .ARRAY;
            array_info := cast(*Type_Info_Array) info;
            if !could_need_allocation(array_info.element_type)  return;

            element_size := array_info.element_type.runtime_size;
            stride := get_array_stride(element_size);

            if array_info.array_count != -1 {
                // The elements are inside the value already; only what they
                // point at needs anything.
                at := data;
                for 0..array_info.array_count - 1 {
                    copy_helper(at, config, array_info.element_type, pointer_map);
                    at += stride;
                }
                return;
            }

            // A view or a resizable array: `count`, then `data`, then for a
            // resizable one `allocated`.
            count := (cast(*s64) data).*;
            array_dest := cast(**void) (data + 8);
            allocated_dest: *s64;
            if array_info.array_type == .RESIZABLE  allocated_dest = cast(*s64) (data + 16);

            array_data := array_dest.*;
            if !array_data  return;

            mapped := find(pointer_map, array_data);
            if mapped {
                array_dest.* = mapped;
                return;
            }

            bytes := stride * count;
            fresh := alloc(bytes);
            array_dest.* = fresh;
            if allocated_dest  allocated_dest.* = count;
            add(pointer_map, array_data, fresh);

            memcpy(fresh, array_data, bytes);
            at := fresh;
            for 0..count - 1 {
                copy_helper(at, config, array_info.element_type, pointer_map);
                at += stride;
            }

        case .POINTER;
            pointer_info := cast(*Type_Info_Pointer) info;

            dest := cast(**void) data;
            old := dest.*;
            if !old  return;

            mapped := find(pointer_map, old);
            if mapped {
                dest.* = mapped;
                return;
            }

            element_size := pointer_info.pointer_to.runtime_size;
            fresh := alloc(element_size);
            memcpy(fresh, old, element_size);
            // Recorded *before* recursing, so a structure that points back at
            // itself finds the copy rather than making another.
            add(pointer_map, old, fresh);
            dest.* = fresh;

            to_type := pointer_info.pointer_to.type;
            if config.follow_struct_pointers || to_type != .STRUCT {
                copy_helper(fresh, config, pointer_info.pointer_to, pointer_map);
            }

        case .STRUCT;
            struct_info := cast(*Type_Info_Struct) info;
            for member: struct_info.members {
                if member.flags & .CONSTANT             continue;
                if !could_need_allocation(member.type)  continue;

                // A member marked `@NoDeepCopy` keeps pointing where it did.
                skip := false;
                for note: member.notes  if note == "NoDeepCopy"  skip = true;
                if skip  continue;

                copy_helper(data + member.offset_in_bytes, config, member.type, pointer_map, debug_name = member.name);
            }
    }
}

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
