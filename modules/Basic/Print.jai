//
// print and its formatting (**L§5.7**, **L§17**).
//
// A `%` in the format string takes the next argument; `%%` is a literal
// percent. Every argument arrives as an `Any`, so what is printed is decided
// by walking the `Type_Info` the compiler put in the type table — which is why
// a struct prints its member names without anyone writing that out.
//

// --------------------------------------------------------- the formatters ---
//
// A formatter carries the thing to print *and* how to print it, so it travels
// through a `%` slot as an ordinary argument:
//
//     print("%\n", formatInt(x, base = 16, minimum_digits = 4));
//
// `append_value` recognises one by the `Formatter` it begins with and prints
// what it describes rather than the struct's own members. The same structs,
// held in `context.print_style`, are what the settings for an *unadorned* `%`
// come from.

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
    comma_string     := "";
}

FormatFloat :: struct {
    using #as formatter: Formatter;

    // -1 for "as wide as it needs to be".
    width          := -1;
    trailing_width := -1;

    Zero_Removal :: enum { YES; NO; ONE_ZERO_AFTER_DECIMAL; }
    // Also removes the point when it would be the last character left.
    zero_removal := Zero_Removal.YES;

    Mode :: enum { DECIMAL; SCIENTIFIC; SHORTEST; }
    mode := Mode.DECIMAL;

    thousand_separators          := false;
    thousand_separator_character : u8 = #char ",";
    decimal_separator_character  : u8 = #char ".";
    exponent_character           : u8 = #char "e";

    Positive_Number_Prefix :: enum { NONE; PLUS; SPACE; }
    positive_number_prefix := Positive_Number_Prefix.NONE;
}

FormatStruct :: struct {
    using #as formatter: Formatter;

    draw_type_name := false;

    // Short form draws values with separators between them: `{1, 3, 5}`. Long
    // form names each one: `{x = 1; y = 3; z = 5}`.
    use_long_form_if_more_than_this_many_members := 5;  // -1 for always long.
    separator_between_name_and_value := " = ";

    short_form_separator_between_fields := ", ";
    long_form_separator_between_fields  := "; ";

    begin_string := "{";
    end_string   := "}";

    indentation_width: s32 = 4;

    use_newlines_if_long_form := false;
}

FormatArray :: struct {
    using #as formatter: Formatter;

    separator    := ", ";
    begin_string := "[";
    end_string   := "]";

    printing_stopped_early_string := "...";

    draw_separator_after_last_element := false;
    // So that printing a huge array does not fill the terminal. -1 never stops.
    stop_printing_after_this_many_elements := 100;
}

// What an unadorned `%` uses. The `Formatter` part of each of these is ignored
// — only the settings are read.
Print_Style :: struct {
    default_format_int:    FormatInt;
    default_format_float:  FormatFloat;
    default_format_struct: FormatStruct;
    default_format_array:  FormatArray;

    default_format_absolute_pointer: FormatInt;
    default_format_absolute_pointer.base             = 16;
    default_format_absolute_pointer.digits_per_comma = 4;
    default_format_absolute_pointer.comma_string     = "_";

    // A program can take struct printing over entirely. Returning false lets
    // the default printer handle the value after all.
    struct_printer: (builder: *String_Builder, any: Any, struct_printer_data: *void) -> bool;
    struct_printer_data: *void;

    // Print's own working state while it descends into a struct.
    indentation_depth: s32;

    log_runtime_errors := true;
}

#add_context print_style: Print_Style;

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

formatFloat :: (value: Any, width := -1, trailing_width := -1, mode := FormatFloat.Mode.DECIMAL, zero_removal := FormatFloat.Zero_Removal.YES) -> FormatFloat {
    result: FormatFloat;
    result.value          = value;
    result.width          = width;
    result.trailing_width = trailing_width;
    result.mode           = mode;
    result.zero_removal   = zero_removal;
    return result;
}

formatStruct :: (value: Any, use_long_form_if_more_than_this_many_members := 5, use_newlines_if_long_form := false) -> FormatStruct {
    result: FormatStruct;
    result.value = value;
    result.use_long_form_if_more_than_this_many_members = use_long_form_if_more_than_this_many_members;
    result.use_newlines_if_long_form = use_newlines_if_long_form;
    return result;
}

// The member that makes a struct a formatter, or null.
isFormatter :: (info: *Type_Info_Struct) -> *Type_Info_Struct_Member {
    for * info.members {
        if it.type == type_info(Formatter)  return it;
    }
    return null;
}


// --------------------------------------------------------------- the API ---

print :: (format_string: string, args: .. Any, to_standard_error := false) -> (bytes_printed: s64) {
    builder: String_Builder;
    print_to_builder(*builder, format_string, ..args);
    return write_builder(*builder, to_standard_error = to_standard_error);
}

// The formatted text in storage the caller owns, out of `context.allocator`.
sprint :: (format_string: string, args: .. Any) -> string #must {
    builder: String_Builder;
    print_to_builder(*builder, format_string, ..args);
    return builder_to_string(*builder);
}

// The formatted text in temporary storage, valid until the next
// `reset_temporary_storage` (**L§10.4**). The builder's own buffers come from
// there too, so a long `tprint` does not touch the context allocator.
tprint :: (format_string: string, args: .. Any) -> string #must {
    builder: String_Builder;
    builder.allocator = temp;
    print_to_builder(*builder, format_string, ..args);
    return builder_to_string(*builder,, allocator = temp);
}

// Appends formatted text to a builder the caller already has, which is what
// code generating source into one wants.
print_to_builder :: (builder: *String_Builder, format_string: string, args: .. Any) -> bool {
    return format_to_builder(builder, format_string, args);
}

// Several values one after another, with no format string between them.
print_sequence_to_builder :: (builder: *String_Builder, args: .. Any) {
    for args  append_any(builder, it);
}

// One value, formatted the way `print` would format it.
to_string :: (value: Any, allocator := context.allocator) -> string {
    builder: String_Builder;
    append_any(*builder, value);
    return builder_to_string(*builder,, allocator = allocator);
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

// a member and without them when it is the whole argument.
print_item_to_builder :: (builder: *String_Builder, item: Any, inside_struct_or_array := true, may_be_uninitialized := false) {
    append_any(builder, item, inside_struct_or_array);
}

// A type's name, the way `print("%", T)` writes it.
print_type_to_builder :: (builder: *String_Builder, info: *Type_Info) {
    append_type(builder, info);
}


// As many spaces as the current depth calls for.
append_indentation :: (builder: *String_Builder, format: *FormatStruct) {
    total := context.print_style.indentation_depth * format.indentation_width;
    for 1..total  append(builder, #char " ");
}


// The public spelling of each, for a caller printing one thing by hand.
print_array :: (builder: *String_Builder, any: Any, info_array: *Type_Info_Array, format: FormatArray, may_be_uninitialized := false) {
    append_array(builder, any.value_pointer, info_array, format);
}

print_integer :: (builder: *String_Builder, format: FormatInt) {
    copy := format;
    append_format_int(builder, *copy);
}

print_float :: (builder: *String_Builder, format: FormatFloat) {
    copy := format;
    append_format_float(builder, copy.value, *copy);
}

print_bytes :: (a: *void, length: int, format_string := "% ") {
    b := cast(*u8) a;
    for 0..length - 1  print(format_string, formatInt(b[it], base = 16));
    print("\n");
}

print_character :: (_c: u8) {
    c := _c;
    text: string;
    text.data = *c;
    text.count = 1;
    write_string(text);
}
#scope_module

// Walks the format string, taking one argument per `%`. An argument nobody
// asked for is appended at the end, and a `%` with nothing left prints as
// itself — neither is an error, since a diagnostic here would fire while the
// program is trying to report something else.
format_to_builder :: (builder: *String_Builder, format: string, args: [] Any) -> bool {
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
    return true;
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

    // A `#type,isa` variant is the type it is a variant of as far as printing
    // goes — `Apollo_Time` prints as the 128-bit number it is (**L§3.11**).
    while info.type == .VARIANT {
        info = (cast(*Type_Info_Variant) info).variant_of;
        if !info {
            append(builder, "(unprintable)");
            return;
        }
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
            record := cast(*Type_Info_Struct) info;
            // A formatter describes something else; printing its own members
            // would be printing the instructions rather than the value.
            if isFormatter(record) {
                append_formatter(builder, data, record);
                return;
            }
            // A program may take struct printing over entirely; returning
            // false hands the value back to the default printer.
            if context.print_style.struct_printer {
                as_any: Any;
                as_any.type = info;
                as_any.value_pointer = data;
                if context.print_style.struct_printer(builder, as_any, context.print_style.struct_printer_data)  return;
            }
            append_struct(builder, data, record, context.print_style.default_format_struct);
        case .ARRAY;     append_array(builder, data, cast(*Type_Info_Array) info, context.print_style.default_format_array);
        case .ANY;       append_any(builder, (cast(*Any) data).*, nested);
        case .VOID;      append(builder, "void");
        case .PROCEDURE; append_pointer(builder, (cast(**void) data).*);
        case .TYPE;      append_type(builder, (cast(**Type_Info) data).*);
        case;            append(builder, "(unprintable)");
    }
}

// Which kind of formatter a struct is, and printing what it describes. The
// `Formatter` member is always first, so the struct's own address is the
// formatter's.
append_formatter :: (builder: *String_Builder, data: *void, info: *Type_Info_Struct) {
    if info == type_info(FormatInt) {
        append_format_int(builder, cast(*FormatInt) data);
        return;
    }
    if info == type_info(FormatFloat) {
        format := cast(*FormatFloat) data;
        append_format_float(builder, format.value, format);
        return;
    }
    if info == type_info(FormatStruct) {
        format := cast(*FormatStruct) data;
        inner := format.value;
        if inner.type && inner.type.type == .STRUCT {
            append_struct(builder, inner.value_pointer, cast(*Type_Info_Struct) inner.type, format.*);
        } else {
            append_any(builder, inner);
        }
        return;
    }
    if info == type_info(FormatArray) {
        format := cast(*FormatArray) data;
        inner := format.value;
        if inner.type && inner.type.type == .ARRAY {
            append_array(builder, inner.value_pointer, cast(*Type_Info_Array) inner.type, format.*);
        } else {
            append_any(builder, inner);
        }
        return;
    }
    // Some other struct that begins with a Formatter: print what it holds.
    append_any(builder, (cast(*Formatter) data).value);
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

// Up to base 64; the first sixteen are the hex digits, so a hex number reads
// the way it always did.
HEX_DIGITS :: "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_.";

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
    if base < 2 || base > 64  base = 10;

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
// A float goes through the same `FormatFloat` an explicit `formatFloat` would
// use; an unadorned `%` takes the one in `context.print_style`.
append_float :: (builder: *String_Builder, data: *void, info: *Type_Info) {
    value: Any;
    value.type = info;
    value.value_pointer = data;
    format := context.print_style.default_format_float;
    append_format_float(builder, value, *format);
}

// Plain positional notation, never an exponent, to `trailing_width` decimal
// places — six when it is -1 — and then the trailing zeros come off unless
// asked to stay, so `7.25` prints as `7.25` rather than `7.250000`.
//
// The expansion is exact. A `float64` is a whole number of halves, so its
// decimal form always terminates; `Decimal` below gets there by doubling or
// halving a digit array, which is what lets `1e300` print all three hundred of
// its digits rather than an approximation of them.
append_format_float :: (builder: *String_Builder, value: Any, format: *FormatFloat) {
    if !value.type || value.type.type != .FLOAT {
        append_any(builder, value);
        return;
    }

    bits: u64;
    if value.type.runtime_size == 4 {
        // The single is widened first; its exact value is a double too, so
        // nothing is lost and one path serves both.
        wide := cast(float64) (cast(*float32) value.value_pointer).*;
        bits = (cast(*u64) *wide).*;
    } else {
        bits = (cast(*u64) value.value_pointer).*;
    }

    SIGN_BIT      :: 0x8000_0000_0000_0000;
    EXPONENT_MASK :: 0x7ff0_0000_0000_0000;
    MANTISSA_MASK :: 0x000f_ffff_ffff_ffff;
    MANTISSA_BITS :: 52;

    negative := (bits & SIGN_BIT) != 0;
    exponent := cast(s64) ((bits & EXPONENT_MASK) >> MANTISSA_BITS);
    mantissa := bits & MANTISSA_MASK;

    // What is written goes through a builder of its own first, so that `width`
    // can pad it to a column.
    inner: String_Builder;
    inner.allocator = temp;

    // The sign is printed for a NaN too, which is what the reference does.
    if negative {
        append(*inner, #char "-");
    } else if format.positive_number_prefix == {
        case .PLUS;  append(*inner, #char "+");
        case .SPACE; append(*inner, #char " ");
        case .NONE;  // Nothing.
    }

    if exponent == 0x7ff {
        append(*inner, ifx mantissa then "NaN" else "Inf");
        append_padded(builder, *inner, format.width);
        return;
    }

    if exponent == 0 {
        // Subnormal, or zero: no implicit leading one, and the exponent is one
        // above what the stored zero suggests.
        if !mantissa {
            append(*inner, "0");
            append_padded(builder, *inner, format.width);
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

    places := ifx format.trailing_width >= 0 then cast(s64) format.trailing_width else 6;
    if places > 17  places = 17;
    decimal_round(*decimal, places);
    decimal_pad(*decimal, places);

    // The column is measured before the zeros come off, so `width` lines up a
    // set of numbers by where their points sit rather than by their lengths.
    padding := format.width - (decimal_length(*decimal, format) + builder_string_length(*inner));

    if format.zero_removal == {
        case .YES;                    decimal_trim(*decimal, 0);
        case .ONE_ZERO_AFTER_DECIMAL; decimal_trim(*decimal, 1);
        case .NO;                     // Keep what was asked for.
    }

    for 1..padding  append(builder, #char " ");
    append(builder, builder_to_string(*inner,, allocator = temp));
    decimal_append(builder, *decimal, format);
}

// Right-aligns what was built into a column `width` wide. A shorter width than
// the text means the text wins; nothing is ever cut off.
append_padded :: (builder: *String_Builder, inner: *String_Builder, width: s64) {
    text := builder_to_string(inner,, allocator = temp);
    for 1..width - text.count  append(builder, #char " ");
    append(builder, text);
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

    for info.values {
        if it == value {
            append(builder, info.names[it_index]);
            return;
        }
    }
    // A flags enum is usually a combination nobody declared a name for, so it
    // is written as the names of the bits that are set.
    if (info.enum_type_flags & .FLAGS) && append_flag_names(builder, value, info)  return;

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

// Short form runs the values together — `{1, 3, 5}`; long form names each one
// — `{x = 1; y = 3; z = 5}`. Which is used depends on how many members there
// are, and long form can put each on its own line.
append_struct :: (builder: *String_Builder, data: *void, info: *Type_Info_Struct, format: FormatStruct) {
    printable := 0;
    for * member: info.members {
        // A constant has no storage in the value, so there is nothing here to
        // read for it (**L§17**).
        if member.flags & .CONSTANT  continue;
        printable += 1;
    }

    long := format.use_long_form_if_more_than_this_many_members < 0
         || printable > format.use_long_form_if_more_than_this_many_members;
    newlines := long && format.use_newlines_if_long_form;

    if format.draw_type_name  append(builder, info.name);
    append(builder, format.begin_string);

    if newlines  context.print_style.indentation_depth += 1;

    written := 0;
    for * member: info.members {
        if member.flags & .CONSTANT  continue;

        // Long form puts a separator after every field, the last one included;
        // short form only between them.
        if written && !long  append(builder, format.short_form_separator_between_fields);
        if newlines {
            append(builder, #char "\n");
            append_indentation(builder, *format);
        }
        written += 1;

        if long {
            append(builder, member.name);
            append(builder, format.separator_between_name_and_value);
        }
        append_value(builder, cast(*u8) data + member.offset_in_bytes, member.type, true);
        if long  append(builder, format.long_form_separator_between_fields);
    }

    if newlines {
        context.print_style.indentation_depth -= 1;
        append(builder, #char "\n");
        append_indentation(builder, *format);
    }
    append(builder, format.end_string);
}
// A long array is cut short rather than filling the terminal; -1 never stops.
append_array :: (builder: *String_Builder, data: *void, info: *Type_Info_Array, format: FormatArray) {
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

    limit := count;
    stopped_early := false;
    if format.stop_printing_after_this_many_elements >= 0 && count > format.stop_printing_after_this_many_elements {
        limit = format.stop_printing_after_this_many_elements;
        stopped_early = true;
    }

    append(builder, format.begin_string);
    for 0..limit - 1 {
        if it  append(builder, format.separator);
        append_value(builder, elements + it * stride, info.element_type, true);
    }
    if limit && format.draw_separator_after_last_element  append(builder, format.separator);
    if stopped_early  append(builder, format.printing_stopped_early_string);
    append(builder, format.end_string);
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
// point with nothing after it. `keep_at_least` is what leaves `1.0` looking
// like a float rather than like `1`.
decimal_trim :: (d: *Decimal, keep_at_least: s64) {
    while d.fraction > keep_at_least && d.digits[d.count - 1] == 0 {
        d.count -= 1;
        d.fraction -= 1;
    }
}

decimal_append :: (builder: *String_Builder, d: *Decimal, format: *FormatFloat) {
    integer := d.count - d.fraction;

    // Leading zeros are dropped, but a number below one keeps the single zero
    // in front of its point.
    start := 0;
    while start < integer - 1 && d.digits[start] == 0  start += 1;

    written := integer - start;
    for start..integer - 1 {
        // Thousands are grouped from the right, so the first separator falls
        // wherever the leading run leaves it.
        if format.thousand_separators && it > start && (integer - it) % 3 == 0 {
            append(builder, format.thousand_separator_character);
        }
        append(builder, cast(u8) (#char "0" + d.digits[it]));
    }
    if !written  append(builder, #char "0");
    if !d.fraction  return;

    append(builder, format.decimal_separator_character);
    for integer..d.count - 1  append(builder, cast(u8) (#char "0" + d.digits[it]));
}

// Fills the fraction out to `places` with zeros, which is what
// `zero_removal = .NO` asks for.
decimal_pad :: (d: *Decimal, places: s64) {
    while d.fraction < places {
        if d.count >= d.digits.count  return;
        d.digits[d.count] = 0;
        d.count += 1;
        d.fraction += 1;
    }
}

// How many characters `decimal_append` would write, so that a column can be
// measured before the trailing zeros come off.
decimal_length :: (d: *Decimal, format: *FormatFloat) -> s64 {
    integer := d.count - d.fraction;
    start := 0;
    while start < integer - 1 && d.digits[start] == 0  start += 1;

    total := integer - start;
    if total <= 0  total = 1;
    if format.thousand_separators && total > 3  total += (total - 1) / 3;
    if d.fraction  total += 1 + d.fraction;
    return total;
}

#scope_export

// An unsigned number in the base a `FormatInt` names, with its padding and its
// digit grouping. The sign is the caller's business, which is what lets a
// signed formatter put it outside the padding.
print_u64_base_n :: (builder: *String_Builder, v: u64, format: FormatInt) {
    base := cast(u64) format.base;
    assert(base >= 2);
    assert(base <= 64);

    MAX_OUTPUT_LENGTH :: 64;

    assert(format.minimum_digits >= 0);
    digits := format.minimum_digits;
    if digits > MAX_OUTPUT_LENGTH  digits = MAX_OUTPUT_LENGTH;

    output: [MAX_OUTPUT_LENGTH] u8;

    // Written from the right, since the digits come out lowest first.
    end := output.data + MAX_OUTPUT_LENGTH;
    p   := end;

    value := v;
    while value || digits > 0 {
        place := value % base;
        value /= base;

        p -= 1;
        // A leading position past the end of the number is padding rather than
        // a zero digit — except the units place, which is a zero.
        if place || value || p == end - 1  p.* = HEX_DIGITS[place];
        else                               p.* = format.padding;

        digits -= 1;
    }

    if p == end && digits > 0 {
        p -= 1;
        p.* = format.padding;
    }

    text: string;
    text.data  = p;
    text.count = end - p;

    if format.digits_per_comma <= 0 || !format.comma_string.count {
        append(builder, text);
        return;
    }

    // Commas fall between groups counted from the right, so the leftmost group
    // is whatever is left over.
    per := cast(s64) format.digits_per_comma;
    before := text.count % per;

    if before {
        head := text;
        head.count = before;
        append(builder, head);
        advance(*text, before);
        if text.count  append(builder, format.comma_string);
    }

    while text.count > 0 {
        piece := text;
        piece.count = per;
        append(builder, piece);
        advance(*text, per);
        if text.count  append(builder, format.comma_string);
    }
}

// A struct's members, given its type and where its bytes are. The same as what
// `%` does for a struct, for a caller doing its own formatting.
append_struct_info :: (builder: *String_Builder, info: *Type_Info_Struct, base_pointer: *void, format: *FormatStruct = null) {
    // A union's members overlap, so there is no telling which of them is the
    // one that was written.
    if info.textual_flags & .UNION {
        append(builder, "(union)");
        return;
    }

    chosen := format;
    if !chosen  chosen = *context.print_style.default_format_struct;

    append_struct(builder, base_pointer, info, chosen.*);
}
