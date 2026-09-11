//
// Pointers stored as an offset from where they sit.
//
// A structure built out of these can be written to a file or mapped somewhere
// else and still be walkable: nothing in it holds an absolute address, so
// nothing has to be fixed up when the base moves. The cost is that a pointer
// only reaches as far as its offset type allows, and that it cannot be copied
// on its own — moving one without what it points at breaks it.
//
// The stored value is the offset with its *sign bit flipped*. That is what
// leaves zero free to mean null: an offset of zero would otherwise be a
// pointer to the field itself, which is never what anyone means, while a
// genuine zero offset becomes the sign bit and reads back correctly.
//

#import "Basic";

Relative_Pointer :: struct (Storage_Type: Type, Pointer_To_Type: Type) {
    value: Storage_Type;

    #assert (Storage_Type == s8) || (Storage_Type == s16) || (Storage_Type == s32) || (Storage_Type == s64);
}

// A string whose bytes are reached the same way. Both members are offsets from
// their own address, so the count is stored as one too rather than as a
// separate integer type.
Relative_String :: struct (Storage_Type: Type) {
    count: Storage_Type;
    data:  Storage_Type;

    #assert (Storage_Type == s8) || (Storage_Type == s16) || (Storage_Type == s32) || (Storage_Type == s64);
}

decode :: inline (p: *Relative_Pointer) -> (result: *p.Pointer_To_Type) {
    return cast(*p.Pointer_To_Type) inline decode_relative_pointer_static(p.Storage_Type, *p.value);
}

encode :: (p: *Relative_Pointer, absolute: *p.Pointer_To_Type) {
    inline write_relative_pointer(p.Storage_Type, *p.value, absolute);
}

decode :: (p: *Relative_String) -> string {
    s: string;
    s.count = p.count;
    s.data = cast(*u8) inline decode_relative_pointer_static(p.Storage_Type, *p.data);
    return s;
}

// ------------------------------------------------------- the raw encoding ---

// Reads an offset of a width known at compile time. The offset is relative to
// where it is stored.
decode_relative_pointer_static :: ($type: Type, address: *void) -> (pointer: *void) {
    #assert (type == s8) || (type == s16) || (type == s32) || (type == s64);
    BITS :: size_of(type) * 8;

    stored := (cast(*type) address).*;
    if !stored  return null;

    offset: s64 = stored ^ (cast(type) 1 << (BITS - 1));
    return cast(*u8) address + offset;
}

// The same with the width known only at runtime, for a walker reading a format
// whose pointer size is in its header. The offset comes back too, since a
// caller checking a file's integrity wants it.
decode_relative_pointer :: (relative_pointer_size: s32, pointer_address: *void, base_address: *void) -> (pointer: *void, offset: s64) {
    offset: s64;
    if relative_pointer_size == {
        case 1;
            stored := (cast(*s8) pointer_address).*;
            if !stored  return null, 0;
            offset = stored ^ 0x80;
        case 2;
            stored := (cast(*s16) pointer_address).*;
            if !stored  return null, 0;
            offset = stored ^ 0x8000;
        case 4;
            stored := (cast(*s32) pointer_address).*;
            if !stored  return null, 0;
            offset = stored ^ 0x8000_0000;
        case 8;
            stored := (cast(*s64) pointer_address).*;
            if !stored  return null, 0;
            offset = stored ^ cast,no_check(s64) 0x8000_0000_0000_0000;
        case;
            return null, 0;
    }
    return cast(*u8) base_address + offset, offset;
}

// The older form, where the offset is relative to the pointer's own address.
decode_relative_pointer :: (relative_pointer_size: s32, address: *void) -> (pointer: *void, offset: s64) {
    return decode_relative_pointer(relative_pointer_size, address, address);
}

// Writes the offset from `base_address` to `target_address` at
// `pointer_address`. A null target is stored as a plain zero.
write_relative_pointer :: ($type: Type, pointer_address: *void, base_address: *void, target_address: *void) {
    #assert (type == s8) || (type == s16) || (type == s32) || (type == s64);
    BITS :: size_of(type) * 8;

    if target_address == null {
        (cast(*type) pointer_address).* = 0;
        return;
    }
    stored := cast(type) (target_address - base_address) ^ (cast(type) 1 << (BITS - 1));
    (cast(*type) pointer_address).* = stored;
}

write_relative_pointer :: ($type: Type, pointer_address: *void, target_address: *void) {
    inline write_relative_pointer(type, pointer_address, pointer_address, target_address);
}
