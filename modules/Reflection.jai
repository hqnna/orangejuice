//
// Reflection: reading a value through its `Type_Info` (**L§17**).
//
// The type table the compiler emits describes every type the program uses, so
// a value's members can be found by name at runtime. That is how `print`
// prints a struct nobody wrote a printer for, and how a serialiser or an
// options parser works on types it has never seen.
//

#import "Basic";

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
