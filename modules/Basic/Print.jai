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

// --------------------------------------------------------- the formatters ---
//
// A formatter is a value that carries the thing to print *and* how to print
// it, so it travels through a `%` slot as an ordinary argument:
//
//     print("%\n", formatInt(x, base = 16, minimum_digits = 4));
//
// `append_value` recognises the struct and prints what it describes rather
// than the struct's own members.

Formatter :: struct {
    value: Any;
}

FormatInt :: struct {
    using #as formatter: Formatter;

    base           := 10;
    minimum_digits := 1;
    // What the digits are padded out to `minimum_digits` with.
    padding: u8 = #char "0";

    // Zero means no commas at all.
    digits_per_comma: u16 = 0;
    comma_string := "";
}

formatInt :: (value: Any, base := 10, minimum_digits := 1, padding: u8 = #char "0", digits_per_comma: u16 = 0, comma_string := "") -> FormatInt {
    result: FormatInt;
    result.value            = value;
    result.base             = base;
    result.minimum_digits   = minimum_digits;
    result.padding          = padding;
    result.digits_per_comma = digits_per_comma;
    result.comma_string     = comma_string;
    return result;
}

// ------------------------------------------------------ printing one thing ---

// One value into a builder, formatted the way a `%` slot would format it.
// `inside_struct_or_array` is what makes a string print with quotes when it is
// a member and without them when it is the whole argument.
print_item_to_builder :: (builder: *String_Builder, item: Any, inside_struct_or_array := false, may_be_uninitialized := false) {
    append_any(builder, item, inside_struct_or_array);
}

// A type's name, the way `print("%", T)` writes it.
print_type_to_builder :: (builder: *String_Builder, info: *Type_Info) {
    append_type(builder, info);
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

// `nested` says the value is a member or an element rather than a whole
// argument, which is what decides whether a string is quoted: `print("%", s)`
// writes the text, and the same string inside a struct writes `"text"`, so
// that a comma in it cannot be read as the next member.

append_any :: (builder: *String_Builder, value: Any, nested := false) {
    // An `Any` missing either half is the empty one — there is nothing to read
    // and nothing to read it as — and it is written as the literal it would
    // be, measured against the reference.
    if !value.type || !value.value_pointer {
        append(builder, "Any.{}");
        return;
    }
    append_value(builder, value.value_pointer, value.type, nested);
}

append_value :: (builder: *String_Builder, data: *void, info: *Type_Info, nested := false) {
    if !info {
        append(builder, "Any.{}");
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
        case .STRING;
            if nested  append(builder, #char "\"");
            append(builder, (cast(*string) data).*);
            if nested  append(builder, #char "\"");
        case .POINTER;   append_pointer(builder, (cast(**void) data).*);
        case .ENUM;      append_enum(builder, data, cast(*Type_Info_Enum) info);
        case .STRUCT;
            // A formatter describes something else; printing its own members
            // would be printing the instructions rather than the value.
            if info == type_info(FormatInt) {
                append_format_int(builder, cast(*FormatInt) data);
                return;
            }
            append_struct(builder, data, cast(*Type_Info_Struct) info);
        case .ARRAY;     append_array(builder, data, cast(*Type_Info_Array) info);
        case .ANY;       append_any(builder, (cast(*Any) data).*, nested);
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

// A `FormatInt` prints the integer it holds, in the base it names, padded and
// grouped as it asks. A negative value keeps its sign outside the padding, so
// `-7` at four digits is `-007` rather than `0-07`.
append_format_int :: (builder: *String_Builder, format: *FormatInt) {
    value := format.value;
    if !value.type || value.type.type != .INTEGER {
        append_any(builder, value);
        return;
    }

    base := format.base;
    if base < 2 || base > 16  base = 10;

    info := cast(*Type_Info_Integer) value.type;
    negative := false;
    magnitude: u64;
    if info.signed {
        signed := read_signed(value.value_pointer, info.runtime_size);
        negative = signed < 0;
        // Negating S64_MIN overflows, so the magnitude is taken unsigned.
        if negative  magnitude = (~cast(u64) signed) + 1;
        else         magnitude = cast(u64) signed;
    } else {
        magnitude = read_unsigned(value.value_pointer, info.runtime_size);
    }

    // 64 digits is the most any base down to 2 can need.
    buffer: [64] u8;
    at := buffer.count;
    if magnitude == 0 {
        at -= 1;
        buffer[at] = #char "0";
    }
    while magnitude > 0 {
        at -= 1;
        buffer[at] = HEX_DIGITS[magnitude % cast(u64) base];
        magnitude /= cast(u64) base;
    }

    digits: string;
    digits.data = buffer.data + at;
    digits.count = buffer.count - at;

    if negative  append(builder, #char "-");
    for 1..format.minimum_digits - digits.count  append(builder, format.padding);

    if format.digits_per_comma <= 0 || !format.comma_string.count {
        append(builder, digits);
        return;
    }

    // Commas fall between groups counted from the right, so the leftmost group
    // is whatever is left over.
    group := cast(s64) format.digits_per_comma;
    first := digits.count % group;
    if first == 0  first = group;
    written := 0;
    while written < digits.count {
        take := ifx written == 0 then first else group;
        if written  append(builder, format.comma_string);
        piece: string;
        piece.data = digits.data + written;
        piece.count = take;
        append(builder, piece);
        written += take;
    }
}

read_signed :: (data: *void, size: s64) -> s64 {
    if size == {
        case 1; return (cast(*s8)  data).*;
        case 2; return (cast(*s16) data).*;
        case 4; return (cast(*s32) data).*;
        case;   return (cast(*s64) data).*;
    }
}

read_unsigned :: (data: *void, size: s64) -> u64 {
    if size == {
        case 1; return (cast(*u8)  data).*;
        case 2; return (cast(*u16) data).*;
        case 4; return (cast(*u32) data).*;
        case;   return (cast(*u64) data).*;
    }
}

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
// A float is written in plain positional notation, never an exponent, to
// `context.print_style.default_float_digits` decimal places — and then the
// trailing zeros come off, so `7.25` prints as `7.25` rather than `7.250000`
// and `100.0` as `100`.
//
// The expansion is exact. A `float64` is a whole number of halves, so its
// decimal form always terminates; `Decimal` below gets there by doubling or
// halving a digit array, which is what lets `1e300` print all three hundred of
// its digits rather than an approximation of them.
append_float :: (builder: *String_Builder, data: *void, info: *Type_Info) {
    bits: u64;
    if info.runtime_size == 4 {
        // The single is widened first; its exact value is a double too, so
        // nothing is lost and one path serves both.
        wide := cast(float64) (cast(*float32) data).*;
        bits = (cast(*u64) *wide).*;
    } else {
        bits = (cast(*u64) data).*;
    }

    SIGN_BIT      :: 0x8000_0000_0000_0000;
    EXPONENT_MASK :: 0x7ff0_0000_0000_0000;
    MANTISSA_MASK :: 0x000f_ffff_ffff_ffff;
    MANTISSA_BITS :: 52;

    negative := (bits & SIGN_BIT) != 0;
    exponent := cast(s64) ((bits & EXPONENT_MASK) >> MANTISSA_BITS);
    mantissa := bits & MANTISSA_MASK;

    // The sign is printed for a NaN too, which is what the reference does.
    if negative  append(builder, #char "-");

    if exponent == 0x7ff {
        append(builder, ifx mantissa then "NaN" else "Inf");
        return;
    }

    if exponent == 0 {
        // Subnormal, or zero: no implicit leading one, and the exponent is one
        // above what the stored zero suggests.
        if !mantissa {
            append(builder, "0");
            return;
        }
        exponent = 1;
    } else {
        mantissa |= cast(u64) 1 << MANTISSA_BITS;
    }
    // value == mantissa * 2^power
    power := exponent - 1023 - MANTISSA_BITS;

    decimal: Decimal;
    decimal_set(*decimal, mantissa);
    if power > 0 {
        for 1..power  decimal_double(*decimal);
    } else {
        for 1..-power  decimal_halve(*decimal);
    }

    places := context.print_style.default_float_digits;
    if places < 0   places = 0;
    if places > 17  places = 17;
    decimal_round(*decimal, places);
    decimal_trim(*decimal);
    decimal_append(builder, *decimal);
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
        // A flags enum is usually a combination nobody declared a name for, so
        // it is written as the names of the bits that are set.
        if (info.enum_type_flags & .FLAGS) && append_flag_names(builder, value, info)  return;
    }
    append_s64(builder, value);
}

// The set bits as `A | B`. False when a bit is set that no member accounts
// for, so the caller falls back to the number rather than printing a name
// that leaves part of the value out.
append_flag_names :: (builder: *String_Builder, value: s64, info: *Type_Info_Enum) -> bool {
    if value == 0  return false;

    covered: s64 = 0;
    for info.values {
        // A member that is itself a combination would double-count, and one
        // that is zero is the "none" name rather than a bit.
        if it == 0  continue;
        if (value & it) == it  covered |= it;
    }
    if covered != value  return false;

    written := 0;
    remaining := value;
    for info.values {
        if it == 0             continue;
        if (remaining & it) != it  continue;
        if written  append(builder, " | ");
        append(builder, info.names[it_index]);
        remaining &= ~it;
        written += 1;
    }
    return written > 0;
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
        append_value(builder, cast(*u8) data + member.offset_in_bytes, member.type, true);
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
        append_value(builder, elements + index * stride, info.element_type, true);
        index += 1;
    }
    append(builder, "]");
}

// ------------------------------------------------------- exact decimals ---
//
// A digit array with a decimal point in it, and the three operations that turn
// a binary float into its exact decimal form: set it to a whole number, double
// it, halve it. Doubling never adds a fraction digit and halving adds at most
// one, so the buffer bounds are known: a `float64`'s exponent reaches 971 up
// and 1074 down, which is 309 integer digits and 1074 fraction digits at worst.

DECIMAL_INTEGER_MAX  :: 320;
DECIMAL_FRACTION_MAX :: 1080;

Decimal :: struct {
    // Most significant first. `count` digits in all, the last `fraction` of
    // them after the point.
    digits:   [DECIMAL_INTEGER_MAX + DECIMAL_FRACTION_MAX] u8;
    count:    s64;
    fraction: s64;
}

decimal_set :: (d: *Decimal, value: u64) {
    d.count = 0;
    d.fraction = 0;
    if value == 0 {
        d.digits[0] = 0;
        d.count = 1;
        return;
    }

    // Written most significant first, so the digits come out backwards and are
    // reversed into place.
    scratch: [20] u8;
    n := 0;
    remaining := value;
    while remaining > 0 {
        scratch[n] = cast(u8) (remaining % 10);
        remaining /= 10;
        n += 1;
    }
    for 0..n - 1  d.digits[it] = scratch[n - 1 - it];
    d.count = n;
}

decimal_double :: (d: *Decimal) {
    carry: u8 = 0;
    at := d.count - 1;
    while at >= 0 {
        sum := d.digits[at] * 2 + carry;
        d.digits[at] = sum % 10;
        carry = sum / 10;
        at -= 1;
    }
    if !carry  return;

    // The carry needs a new digit at the front, so everything shifts along.
    if d.count >= d.digits.count  return;
    at = d.count;
    while at > 0 {
        d.digits[at] = d.digits[at - 1];
        at -= 1;
    }
    d.digits[0] = carry;
    d.count += 1;
}

decimal_halve :: (d: *Decimal) {
    remainder: u8 = 0;
    for 0..d.count - 1 {
        current := remainder * 10 + d.digits[it];
        d.digits[it] = current / 2;
        remainder = current % 2;
    }
    // A remainder of one is exactly half, which is one more fraction digit.
    if !remainder  return;
    if d.count >= d.digits.count  return;
    d.digits[d.count] = 5;
    d.count += 1;
    d.fraction += 1;
}

// Keeps `places` fraction digits, rounding half away from zero.
decimal_round :: (d: *Decimal, places: s64) {
    if d.fraction <= places  return;

    keep := d.count - d.fraction + places;
    round_up := d.digits[keep] >= 5;
    d.count = keep;
    d.fraction = places;

    if !round_up  return;
    at := d.count - 1;
    while at >= 0 {
        if d.digits[at] < 9 {
            d.digits[at] += 1;
            return;
        }
        d.digits[at] = 0;
        at -= 1;
    }
    // Every digit carried, so the number grew a place: 9.99 -> 10.00.
    if d.count >= d.digits.count  return;
    at = d.count;
    while at > 0 {
        d.digits[at] = d.digits[at - 1];
        at -= 1;
    }
    d.digits[0] = 1;
    d.count += 1;
}

// Trailing zeros after the point carry no information, and neither does a
// point with nothing after it.
decimal_trim :: (d: *Decimal) {
    while d.fraction > 0 && d.digits[d.count - 1] == 0 {
        d.count -= 1;
        d.fraction -= 1;
    }
}

decimal_append :: (builder: *String_Builder, d: *Decimal) {
    integer := d.count - d.fraction;

    // Leading zeros are dropped, but a number below one keeps the single zero
    // in front of its point.
    start := 0;
    while start < integer - 1 && d.digits[start] == 0  start += 1;

    for start..integer - 1  append(builder, cast(u8) (#char "0" + d.digits[it]));
    if !d.fraction  return;

    append(builder, #char ".");
    for integer..d.count - 1  append(builder, cast(u8) (#char "0" + d.digits[it]));
}
