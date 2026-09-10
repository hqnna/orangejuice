//
// print and its formatting (**L§5.7**, **L§17**).
//
// A `%` in the format string takes the next argument; `%%` is a literal
// percent. Every argument arrives as an `Any`, so what is printed is decided
// by walking the `Type_Info` the compiler put in the type table — which is why
// a struct prints its member names without anyone writing that out.
//

Print_Style :: struct {
    // How many digits after the point a float gets. Shortest round-tripping
    // output is what this should eventually be; see the note on
    // `append_float`.
    default_float_digits: s64 = 6;
    // Print an enum's member name rather than its number where one matches.
    enum_names := true;
}

#add_context print_style: Print_Style;

// --------------------------------------------------------------- the API ---

print :: (format: string, args: ..Any, to_standard_error := false) -> s64 {
    builder: String_Builder;
    init_string_builder(*builder, temp);
    count := format_to_builder(*builder, format, args);
    write_builder(*builder, to_standard_error);
    return count;
}

// The formatted text in storage the caller owns.
sprint :: (format: string, args: ..Any, allocator := context.allocator) -> string {
    builder: String_Builder;
    init_string_builder(*builder, temp);
    format_to_builder(*builder, format, args);
    return builder_to_string(*builder, allocator);
}

// The formatted text in temporary storage, valid until the next
// `reset_temporary_storage` (**L§10.4**).
tprint :: (format: string, args: ..Any) -> string {
    return sprint(format, ..args, allocator = temp);
}

// Appends formatted text to a builder the caller already has, which is what
// code generating source into one wants.
print_to_builder :: (builder: *String_Builder, format: string, args: ..Any) -> s64 {
    return format_to_builder(builder, format, args);
}

// One value, formatted the way `print` would format it.
to_string :: (value: Any, allocator := context.allocator) -> string {
    builder: String_Builder;
    init_string_builder(*builder, temp);
    append_any(*builder, value);
    return builder_to_string(*builder, allocator);
}

#scope_module

// Walks the format string, taking one argument per `%`. An argument nobody
// asked for is appended at the end, and a `%` with nothing left prints as
// itself — neither is an error, since a diagnostic here would fire while the
// program is trying to report something else.
format_to_builder :: (builder: *String_Builder, format: string, args: [] Any) -> s64 {
    next := 0;
    at := 0;
    while at < format.count {
        c := format[at];
        if c != #char "%" {
            start := at;
            while at < format.count && format[at] != #char "%"  at += 1;
            run: string;
            run.data = format.data + start;
            run.count = at - start;
            append(builder, run);
            continue;
        }

        at += 1;
        if at < format.count && format[at] == #char "%" {
            append(builder, #char "%");
            at += 1;
            continue;
        }
        if next < args.count {
            append_any(builder, args[next]);
            next += 1;
        } else {
            append(builder, #char "%");
        }
    }

    // Arguments past the last `%` still get printed, which is what makes
    // `print("x")` with a stray argument visible rather than silent.
    while next < args.count {
        append_any(builder, args[next]);
        next += 1;
    }
    return builder_string_length(builder);
}

// ------------------------------------------------------------ dispatching ---

append_any :: (builder: *String_Builder, value: Any) {
    if !value.type {
        append(builder, "(null type)");
        return;
    }
    append_value(builder, value.value_pointer, value.type);
}

append_value :: (builder: *String_Builder, data: *void, info: *Type_Info) {
    if !info {
        append(builder, "(null type)");
        return;
    }
    if !data {
        append(builder, "(null)");
        return;
    }

    if info.type == {
        case .INTEGER;   append_integer(builder, data, cast(*Type_Info_Integer) info);
        case .FLOAT;     append_float(builder, data, info);
        case .BOOL;      append(builder, ifx (cast(*bool) data).* then "true" else "false");
        case .STRING;    append(builder, (cast(*string) data).*);
        case .POINTER;   append_pointer(builder, (cast(**void) data).*);
        case .ENUM;      append_enum(builder, data, cast(*Type_Info_Enum) info);
        case .STRUCT;    append_struct(builder, data, cast(*Type_Info_Struct) info);
        case .ARRAY;     append_array(builder, data, cast(*Type_Info_Array) info);
        case .ANY;       append_any(builder, (cast(*Any) data).*);
        case .VOID;      append(builder, "void");
        case .PROCEDURE; append_pointer(builder, (cast(**void) data).*);
        case .TYPE;      append_type(builder, (cast(**Type_Info) data).*);
        case;            append(builder, "(unprintable)");
    }
}

// ------------------------------------------------------------- the leaves ---

append_integer :: (builder: *String_Builder, data: *void, info: *Type_Info_Integer) {
    size := info.runtime_size;
    if info.signed {
        value: s64;
        if size == {
            case 1; value = (cast(*s8)  data).*;
            case 2; value = (cast(*s16) data).*;
            case 4; value = (cast(*s32) data).*;
            case;   value = (cast(*s64) data).*;
        }
        append_s64(builder, value);
    } else {
        value: u64;
        if size == {
            case 1; value = (cast(*u8)  data).*;
            case 2; value = (cast(*u16) data).*;
            case 4; value = (cast(*u32) data).*;
            case;   value = (cast(*u64) data).*;
        }
        append_u64(builder, value);
    }
}

append_s64 :: (builder: *String_Builder, value: s64) {
    if value < 0 {
        append(builder, #char "-");
        // Negating S64_MIN overflows, so the magnitude is taken unsigned.
        append_u64(builder, (~cast(u64) value) + 1);
    } else {
        append_u64(builder, cast(u64) value);
    }
}

append_u64 :: (builder: *String_Builder, value: u64) {
    buffer: [20] u8;
    at := buffer.count;
    remaining := value;
    if remaining == 0 {
        at -= 1;
        buffer[at] = #char "0";
    }
    while remaining > 0 {
        at -= 1;
        buffer[at] = cast(u8) (#char "0" + remaining % 10);
        remaining /= 10;
    }
    text: string;
    text.data = buffer.data + at;
    text.count = buffer.count - at;
    append(builder, text);
}

HEX_DIGITS :: "0123456789abcdef";

append_pointer :: (builder: *String_Builder, pointer: *void) {
    if !pointer {
        append(builder, "null");
        return;
    }
    value := cast(u64) pointer;
    buffer: [16] u8;
    at := buffer.count;
    while value > 0 {
        at -= 1;
        buffer[at] = HEX_DIGITS[value & 0xf];
        value >>= 4;
    }
    text: string;
    text.data = buffer.data + at;
    text.count = buffer.count - at;
    append(builder, text);
}

// @Incomplete: this is a fixed-precision conversion, not the shortest string
// that reads back as the same float. Printing a float and parsing it again
// does not always give what you started with; Ryū is the intended replacement
// and is why `float_to_string` is its own file in the reference.
append_float :: (builder: *String_Builder, data: *void, info: *Type_Info) {
    value: float64;
    if info.runtime_size == 4  value = cast(float64) (cast(*float32) data).*;
    else                       value = (cast(*float64) data).*;

    if value != value {
        append(builder, "NaN");
        return;
    }

    if value < 0 {
        append(builder, #char "-");
        value = -value;
    }

    INFINITY :: 0h7FF0000000000000;
    if value == INFINITY {
        append(builder, "Infinity");
        return;
    }

    digits := context.print_style.default_float_digits;
    if digits < 0  digits = 0;
    if digits > 17 digits = 17;

    scale: float64 = 1;
    count := 0;
    while count < digits {
        scale *= 10;
        count += 1;
    }

    // Round half away from zero at the last digit kept.
    scaled := value * scale + 0.5;
    // Anything past what a u64 holds is printed in whole units only.
    if scaled >= 18_446_744_073_709_551_000.0 {
        append_u64(builder, cast(u64) value);
        return;
    }

    whole := cast(u64) scaled;
    integral := whole;
    fraction := cast(u64) 0;
    divisor := cast(u64) scale;
    if divisor > 0 {
        integral = whole / divisor;
        fraction = whole % divisor;
    }

    append_u64(builder, integral);
    if digits <= 0  return;

    append(builder, #char ".");
    // Leading zeroes of the fraction are significant, so they are written out
    // rather than lost by printing the number alone.
    place := divisor / 10;
    while place > 0 {
        digit := (fraction / place) % 10;
        append(builder, cast(u8) (#char "0" + digit));
        place /= 10;
    }
}

append_type :: (builder: *String_Builder, info: *Type_Info) {
    if !info {
        append(builder, "(null type)");
        return;
    }
    if info.type == {
        case .STRUCT; append(builder, (cast(*Type_Info_Struct) info).name);
        case .ENUM;   append(builder, (cast(*Type_Info_Enum) info).name);
        case;         append(builder, type_tag_name(info.type));
    }
}

type_tag_name :: (tag: Type_Info_Tag) -> string {
    if tag == {
        case .INTEGER;   return "integer";
        case .FLOAT;     return "float";
        case .BOOL;      return "bool";
        case .STRING;    return "string";
        case .POINTER;   return "pointer";
        case .PROCEDURE; return "procedure";
        case .VOID;      return "void";
        case .ARRAY;     return "array";
        case .ANY;       return "Any";
        case .TYPE;      return "Type";
        case .CODE;      return "Code";
        case;            return "type";
    }
}

// ---------------------------------------------------------- the composites ---

append_enum :: (builder: *String_Builder, data: *void, info: *Type_Info_Enum) {
    value: s64;
    size := ifx info.internal_type then info.internal_type.runtime_size else 8;
    signed := ifx info.internal_type then info.internal_type.signed else true;
    if signed {
        if size == {
            case 1; value = (cast(*s8)  data).*;
            case 2; value = (cast(*s16) data).*;
            case 4; value = (cast(*s32) data).*;
            case;   value = (cast(*s64) data).*;
        }
    } else {
        if size == {
            case 1; value = cast(s64) (cast(*u8)  data).*;
            case 2; value = cast(s64) (cast(*u16) data).*;
            case 4; value = cast(s64) (cast(*u32) data).*;
            case;   value = cast(s64) (cast(*u64) data).*;
        }
    }

    if context.print_style.enum_names {
        for info.values {
            if it == value {
                append(builder, info.names[it_index]);
                return;
            }
        }
    }
    append_s64(builder, value);
}

append_struct :: (builder: *String_Builder, data: *void, info: *Type_Info_Struct) {
    append(builder, "{");
    written := 0;
    for * member: info.members {
        // A constant has no storage in the value, so there is nothing here to
        // read for it (**L§17**).
        if member.flags & .CONSTANT  continue;
        if written  append(builder, ", ");
        written += 1;
        append_value(builder, cast(*u8) data + member.offset_in_bytes, member.type);
    }
    append(builder, "}");
}

append_array :: (builder: *String_Builder, data: *void, info: *Type_Info_Array) {
    count: s64;
    elements: *u8;

    if info.array_type == {
        case .FIXED;
            count = info.array_count;
            elements = cast(*u8) data;
        case .VIEW;
            view := cast(*Array_View_64) data;
            count = view.count;
            elements = view.data;
        case .RESIZABLE;
            array := cast(*Resizable_Array) data;
            count = array.count;
            elements = cast(*u8) array.data;
    }

    if !elements  count = 0;
    stride := ifx info.element_type then info.element_type.runtime_size else 0;

    append(builder, "[");
    index := 0;
    while index < count {
        if index  append(builder, ", ");
        append_value(builder, elements + index * stride, info.element_type);
        index += 1;
    }
    append(builder, "]");
}
