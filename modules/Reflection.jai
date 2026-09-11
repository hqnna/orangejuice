//
// Reflection: reading a value through its `Type_Info` (**L§17**).
//
// The type table the compiler emits describes every type the program uses, so
// a value's members can be found by name at runtime. That is how `print`
// prints a struct nobody wrote a printer for, and how a serialiser or an
// options parser works on types it has never seen.
//

#import "Basic";
#import "String";

// A value matching no member comes back as "", not as its number.
enum_value_to_name :: (value: $T) -> string {
    base := type_info(T);
    if base.type != .ENUM  return "";
    return enum_name(cast(*Type_Info_Enum) base, cast(s64) value);
}

// The member with that name, as a value of the enum itself.
enum_name_to_value :: ($T: Type, name: string) -> (value: T, found: bool) {
    base := type_info(T);
    if base.type != .ENUM  return cast,no_check(T) 0, false;

    info := cast(*Type_Info_Enum) base;
    for info.names {
        if it == name  return cast,no_check(T) info.values[it_index], true;
    }
    return cast,no_check(T) 0, false;
}

// ------------------------------------------------------------ storing ints ---

// Writes `value` through `pointer` as the integer type `info` describes, but
// only when it fits — a caller parsing text into a field it found by
// reflection has no other way to know the number is in range.
//
// `low` and `high` come back so a diagnostic can say what the range was. They
// are meaningful whether or not the store happened.
range_check_and_store :: (value: $T, info: *Type_Info_Integer, pointer: *void) -> (success: bool, low: T, high: T) {
    #assert(T == u64 || T == s64, "range_check_and_store takes a u64 or an s64.");

    size := info.runtime_size;
    low, high := integer_range(T, info);
    if value < low || value > high  return false, low, high;

    if size == {
        case 1; (cast(*s8)  pointer).* = cast,no_check(s8)  value;
        case 2; (cast(*s16) pointer).* = cast,no_check(s16) value;
        case 4; (cast(*s32) pointer).* = cast,no_check(s32) value;
        case 8; (cast(*s64) pointer).* = cast,no_check(s64) value;
        case;   return false, low, high;
    }
    return true, low, high;
}


// ------------------------------------------------------------ enum storage ---

// An enum's value read out of storage as an `s64`, whatever width and sign the
// enum's own integer type has. A 64-bit unsigned enum whose top bit is set
// comes back negative, which is the reference's behaviour too.
get_enum_value :: (data: *void, info: *Type_Info_Enum) -> s64 {
    size := ifx info.internal_type then info.internal_type.runtime_size else 8;
    signed := ifx info.internal_type then info.internal_type.signed else true;

    if signed {
        if size == {
            case 1; return (cast(*s8)  data).*;
            case 2; return (cast(*s16) data).*;
            case 4; return (cast(*s32) data).*;
            case;   return (cast(*s64) data).*;
        }
    }
    if size == {
        case 1; return cast(s64) (cast(*u8)  data).*;
        case 2; return cast(s64) (cast(*u16) data).*;
        case 4; return cast(s64) (cast(*u32) data).*;
        case;   return cast,no_check(s64) (cast(*u64) data).*;
    }
}

set_enum_value :: (data: *void, info: *Type_Info_Enum, value: s64) {
    size := ifx info.internal_type then info.internal_type.runtime_size else 8;
    if size == {
        case 1; (cast(*s8)  data).* = cast,no_check(s8)  value;
        case 2; (cast(*s16) data).* = cast,no_check(s16) value;
        case 4; (cast(*s32) data).* = cast,no_check(s32) value;
        case;   (cast(*s64) data).* = value;
    }
}

// ---------------------------------------------------------------- structs ---

// The member with that name, or null. Only the struct's own members are
// searched; one brought in by `using` belongs to the type it came from.
get_struct_field_info :: (info: *Type_Info_Struct, name: string) -> *Type_Info_Struct_Member {
    if !info || info.type != .STRUCT  return null;
    for * member: info.members {
        if member.name == name  return member;
    }
    return null;
}

// ----------------------------------------------------------------- arrays ---

// How many elements an array of any kind holds and where they start. A fixed
// array is its own storage; a view and a resizable one both begin with a count
// and a pointer, which is what lets one branch cover them.
get_array_count_and_data :: (pointer: *void, info: *Type_Info_Array) -> (count: s64, data: *void) {
    if !info || info.type != .ARRAY  return 0, null;
    if info.array_type == .FIXED     return info.array_count, pointer;

    array := cast(*Array_View_64) pointer;
    return array.count, array.data;
}

// --------------------------------------------------------- integer ranges ---

// What an integer of that many bytes can hold. The size must be 1, 2, 4 or 8.
signed_integer_range_from_size :: (size_in_bytes: int) -> (low: s64, high: s64) {
    assert(size_in_bytes == 1 || size_in_bytes == 2 || size_in_bytes == 4 || size_in_bytes == 8);
    if size_in_bytes == 8  return S64_LOW, S64_HIGH;
    high := (cast(s64) 1 << (size_in_bytes * 8 - 1)) - 1;
    return ~high, high;
}

unsigned_integer_range_from_size :: (size_in_bytes: int) -> (low: u64, high: u64) {
    assert(size_in_bytes == 1 || size_in_bytes == 2 || size_in_bytes == 4 || size_in_bytes == 8);
    // Shifting a 64-bit value by 64 is not defined, so the widest case is
    // written out rather than computed.
    if size_in_bytes == 8  return 0, 0xffff_ffff_ffff_ffff;
    return 0, (cast(u64) 1 << (size_in_bytes * 8)) - 1;
}

// --------------------------------------------------------- text to a value ---

// Reads one value of `any`'s type out of `text` and writes it through
// `any.value_pointer`. The pointer form advances the string past what it read,
// which is what a scanner walking a line needs.
set_value_from_string :: (any: Any, text: string) -> bool {
    scratch := text;
    return set_value_from_string(any, *scratch);
}

set_value_from_string :: (any: Any, text: *string) -> bool {
    if !any.type || !any.value_pointer  return false;

    if any.type.type == {
        case .INTEGER;
            value, ok, rest := string_to_int(text.*);
            if !ok  return false;
            integer := cast(*Type_Info_Integer) any.type;
            fits := range_check_and_store(value, integer, cast(*u8) any.value_pointer);
            if !fits  return false;
            text.* = rest;
            return true;

        case .FLOAT;
            value, ok, rest := string_to_float64(text.*);
            if !ok  return false;
            // Braced because a parenthesis right after the condition would
            // otherwise read as a call on it.
            if any.type.runtime_size == 4 {
                (cast(*float32) any.value_pointer).* = cast(float32) value;
            } else {
                (cast(*float64) any.value_pointer).* = value;
            }
            text.* = rest;
            return true;

        case .BOOL;
            if begins_with(text.*, "true") {
                (cast(*bool) any.value_pointer).* = true;
                advance(text, 4);
                return true;
            }
            if begins_with(text.*, "false") {
                (cast(*bool) any.value_pointer).* = false;
                advance(text, 5);
                return true;
            }
            return false;

        case .STRING;
            value, ok := read_quoted(text);
            if !ok  return false;
            (cast(*string) any.value_pointer).* = value;
            return true;

        case .ENUM;
            info := cast(*Type_Info_Enum) any.type;
            // A name first, since that is what a person writes; a bare number
            // is accepted too, which is what a flags combination needs.
            name := stop_at_any(text.*, " \t\r\n,)]}");
            for info.names {
                if it == name {
                    set_enum_value(any.value_pointer, info, info.values[it_index]);
                    advance(text, name.count);
                    return true;
                }
            }
            value, ok, rest := string_to_int(text.*);
            if !ok  return false;
            set_enum_value(any.value_pointer, info, value);
            text.* = rest;
            return true;

        case;
            return false;
    }
}

#scope_module

S64_LOW  :: -9_223_372_036_854_775_808;
S64_HIGH ::  9_223_372_036_854_775_807;

// A `"`-delimited run, without the quotes. Escape sequences are not handled,
// which is what the reference does too.
read_quoted :: (text: *string) -> (value: string, ok: bool) {
    if !text.count || text.*[0] != #char "\""  return "", false;
    advance(text, 1);

    at := 0;
    while at < text.count {
        if text.*[at] == #char "\"" {
            result := slice(text.*, 0, at);
            advance(text, at + 1);
            return result, true;
        }
        at += 1;
    }
    return "", false;
}

// -------------------------------------------------------------- members ---

// The member, or null. Only the struct's own members are searched; a member
// brought in by `using` belongs to the type it came from.
get_member :: (info: *Type_Info_Struct, name: string) -> *Type_Info_Struct_Member {
    if !info || info.type != .STRUCT  return null;
    for * member: info.members {
        if member.name == name  return member;
    }
    return null;
}

get_member :: (value: Any, name: string) -> *Type_Info_Struct_Member {
    return get_member(cast(*Type_Info_Struct) value.type, name);
}

// The member's storage inside `value`, as an `Any` so it can be read or
// written through without knowing its type at the call site.
get_member_value :: (value: Any, name: string) -> (member: Any, found: bool) {
    empty: Any;
    info := cast(*Type_Info_Struct) value.type;
    member := get_member(info, name);
    if !member  return empty, false;
    // A constant has no storage in the value (**L§17**).
    if member.flags & .CONSTANT  return empty, false;

    result: Any;
    result.type = member.type;
    result.value_pointer = cast(*u8) value.value_pointer + member.offset_in_bytes;
    return result, true;
}

// Every member that has storage, in declaration order.
get_members :: (info: *Type_Info_Struct, allocator := context.allocator) -> [] *Type_Info_Struct_Member {
    found: [..] *Type_Info_Struct_Member;
    found.allocator = allocator;
    if !info || info.type != .STRUCT  return found;
    for * member: info.members {
        if member.flags & .CONSTANT  continue;
        array_add(*found, member);
    }
    return found;
}

// --------------------------------------------------------------- typing ---

type_name :: (info: *Type_Info) -> string {
    if !info  return "(null)";
    if info.type == {
        case .STRUCT;  return (cast(*Type_Info_Struct) info).name;
        case .ENUM;    return (cast(*Type_Info_Enum) info).name;
        case .VARIANT; return (cast(*Type_Info_Variant) info).name;
        case .INTEGER;
            integer := cast(*Type_Info_Integer) info;
            if integer.signed {
                if integer.runtime_size == {
                    case 1; return "s8";
                    case 2; return "s16";
                    case 4; return "s32";
                    case;   return "s64";
                }
            }
            if integer.runtime_size == {
                case 1; return "u8";
                case 2; return "u16";
                case 4; return "u32";
                case;   return "u64";
            }
        case .FLOAT;     return ifx info.runtime_size == 4 then "float32" else "float64";
        case .BOOL;      return "bool";
        case .STRING;    return "string";
        case .VOID;      return "void";
        case .POINTER;   return "pointer";
        case .PROCEDURE; return "procedure";
        case .ARRAY;     return "array";
        case .ANY;       return "Any";
        case .TYPE;      return "Type";
        case .CODE;      return "Code";
        case;            return "(unknown)";
    }
    return "(unknown)";
}

is_struct  :: (info: *Type_Info) -> bool { return info && info.type == .STRUCT; }
is_enum    :: (info: *Type_Info) -> bool { return info && info.type == .ENUM; }
is_array   :: (info: *Type_Info) -> bool { return info && info.type == .ARRAY; }
is_pointer :: (info: *Type_Info) -> bool { return info && info.type == .POINTER; }

is_integer :: (info: *Type_Info) -> bool { return info && info.type == .INTEGER; }
is_float   :: (info: *Type_Info) -> bool { return info && info.type == .FLOAT; }

is_number :: (info: *Type_Info) -> bool {
    return is_integer(info) || is_float(info);
}

// ---------------------------------------------------------------- values ---

// Reads any integer as an `s64`, whatever its width or sign. `ok` is false
// for anything that is not an integer or an enum.
get_integer :: (value: Any) -> (result: s64, ok: bool) {
    if !value.type || !value.value_pointer  return 0, false;

    info := value.type;
    signed := true;
    if info.type == .ENUM {
        enum_info := cast(*Type_Info_Enum) info;
        if enum_info.internal_type {
            signed = enum_info.internal_type.signed;
            info = enum_info.internal_type;
        }
    } else if info.type == .INTEGER {
        signed = (cast(*Type_Info_Integer) info).signed;
    } else {
        return 0, false;
    }

    data := value.value_pointer;
    if signed {
        if info.runtime_size == {
            case 1; return (cast(*s8)  data).*, true;
            case 2; return (cast(*s16) data).*, true;
            case 4; return (cast(*s32) data).*, true;
            case;   return (cast(*s64) data).*, true;
        }
    }
    if info.runtime_size == {
        case 1; return cast(s64) (cast(*u8)  data).*, true;
        case 2; return cast(s64) (cast(*u16) data).*, true;
        case 4; return cast(s64) (cast(*u32) data).*, true;
        case;   return cast(s64) (cast(*u64) data).*, true;
    }
    return 0, false;
}

get_float :: (value: Any) -> (result: float64, ok: bool) {
    if !value.type || !value.value_pointer  return 0, false;
    if value.type.type != .FLOAT  return 0, false;
    if value.type.runtime_size == 4  return cast(float64) (cast(*float32) value.value_pointer).*, true;
    return (cast(*float64) value.value_pointer).*, true;
}

get_string :: (value: Any) -> (result: string, ok: bool) {
    empty: string;
    if !value.type || !value.value_pointer  return empty, false;
    if value.type.type != .STRING  return empty, false;
    return (cast(*string) value.value_pointer).*, true;
}

// ----------------------------------------------------------------- enums ---

// The member name for a value, or "" when nothing matches — which happens for
// a flags enum holding a combination.
enum_name :: (info: *Type_Info_Enum, value: s64) -> string {
    if !info  return "";
    for info.values  if it == value  return info.names[it_index];
    return "";
}

enum_value :: (info: *Type_Info_Enum, name: string) -> (value: s64, found: bool) {
    if !info  return 0, false;
    for info.names  if it == name  return info.values[it_index], true;
    return 0, false;
}

is_flags_enum :: (info: *Type_Info_Enum) -> bool {
    return info != null && (info.enum_type_flags & .FLAGS) != 0;
}

// ---------------------------------------------------------------- arrays ---

// The elements of any array — fixed, view or resizable — as one view, so a
// caller walking data does not have to care which it was given.
get_array :: (value: Any) -> (elements: *u8, count: s64, element_type: *Type_Info, ok: bool) {
    if !value.type || value.type.type != .ARRAY  return null, 0, null, false;
    info := cast(*Type_Info_Array) value.type;
    data := value.value_pointer;

    if info.array_type == {
        case .FIXED;
            return cast(*u8) data, info.array_count, info.element_type, true;
        case .VIEW;
            view := cast(*Array_View_64) data;
            return view.data, view.count, info.element_type, true;
        case .RESIZABLE;
            array := cast(*Resizable_Array) data;
            return cast(*u8) array.data, array.count, info.element_type, true;
    }
    return null, 0, null, false;
}

// The element at `index`, as an `Any`.
get_array_element :: (value: Any, index: s64) -> (element: Any, ok: bool) {
    empty: Any;
    elements, count, element_type, ok := get_array(value);
    if !ok || index < 0 || index >= count  return empty, false;
    if !element_type  return empty, false;

    result: Any;
    result.type = element_type;
    result.value_pointer = elements + index * element_type.runtime_size;
    return result, true;
}

// -------------------------------------------------------------- pointers ---

// What a pointer points at, as an `Any`. `ok` is false for a null one, so a
// caller does not dereference it by accident.
dereference :: (value: Any) -> (pointed_at: Any, ok: bool) {
    empty: Any;
    if !value.type || value.type.type != .POINTER  return empty, false;
    info := cast(*Type_Info_Pointer) value.type;
    target := (cast(**void) value.value_pointer).*;
    if !target  return empty, false;

    result: Any;
    result.type = info.pointer_to;
    result.value_pointer = target;
    return result, true;
}

#scope_module

// The widest and narrowest value the described integer holds, expressed in the
// caller's own type — which is why an unsigned caller sees a low of zero and a
// signed one sees the negative bound.
integer_range :: ($T: Type, info: *Type_Info_Integer) -> (low: T, high: T) {
    bits := info.runtime_size * 8;

    #if T == u64 {
        if info.signed {
            // A signed slot's top is one bit narrower; its negative half is
            // out of reach for an unsigned value anyway.
            if bits >= 64  return 0, cast(u64) 0x7fff_ffff_ffff_ffff;
            return 0, (cast(u64) 1 << (bits - 1)) - 1;
        }
        if bits >= 64  return 0, 0xffff_ffff_ffff_ffff;
        return 0, (cast(u64) 1 << bits) - 1;
    } else {
        if info.signed {
            if bits >= 64  return cast(s64) 0x8000_0000_0000_0000, 0x7fff_ffff_ffff_ffff;
            magnitude := cast(s64) 1 << (bits - 1);
            return -magnitude, magnitude - 1;
        }
        // An unsigned slot cannot take a negative value at all.
        if bits >= 64  return 0, 0x7fff_ffff_ffff_ffff;
        return 0, (cast(s64) 1 << bits) - 1;
    }
}

#scope_export
