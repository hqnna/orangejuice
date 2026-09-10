//
// Command_Line: arguments into a struct.
//
// A struct's members are the options: `parse_arguments(*Options)` fills them
// from the command line by walking the type table, so adding an option is
// adding a member rather than adding a case to a parser.
//
// `-name value` for anything that takes one, `-name` alone for a bool, and
// everything after `--` or not starting with `-` is left in `arguments`.
//

#import "Basic";
#import "String";

Argument_Flags :: enum_flags {
    NONE                  :: 0;
    // Stop at the first thing that is not an option, rather than gathering
    // the rest — what a program that wraps another one wants.
    STOP_AT_FIRST_LOOSE   :: 0x1;
    // Say nothing about options the struct has no member for.
    IGNORE_UNKNOWN        :: 0x2;
}

Parse_Result :: struct {
    ok := true;
    // What was not an option: filenames and the like, in order.
    arguments: [..] string;
    // Why it failed, when it did.
    error_message: string;
}

// Fills `options` from the program's own command line, skipping argv[0].
parse_arguments :: (options: *$T, flags := Argument_Flags.NONE) -> Parse_Result {
    args: [..] string;
    args.allocator = temp;
    for 1..__command_line_arguments.count - 1 {
        array_add(*args, to_string(__command_line_arguments[it]));
    }
    return parse_arguments(options, args, flags);
}

parse_arguments :: (options: *$T, args: [] string, flags := Argument_Flags.NONE) -> Parse_Result {
    result: Parse_Result;

    info := cast(*Type_Info_Struct) type_info(T);
    if !info || info.type != .STRUCT {
        result.ok = false;
        result.error_message = "Options must be a struct.";
        return result;
    }

    at := 0;
    while at < args.count {
        argument := args[at];

        // `--` ends the options; whatever follows is a plain argument even if
        // it looks like one.
        if argument == "--" {
            at += 1;
            while at < args.count {
                array_add(*result.arguments, args[at]);
                at += 1;
            }
            break;
        }

        if !begins_with(argument, "-") || argument.count == 1 {
            array_add(*result.arguments, argument);
            at += 1;
            if flags & .STOP_AT_FIRST_LOOSE {
                while at < args.count {
                    array_add(*result.arguments, args[at]);
                    at += 1;
                }
                break;
            }
            continue;
        }

        // One or two leading dashes both name the same option, and `-name=v`
        // is the same as `-name v`.
        name := trim_left(argument, "-");
        inline_value: string;
        has_inline := false;
        equals := find_index_from_left(name, #char "=");
        if equals >= 0 {
            inline_value = slice(name, equals + 1, name.count);
            name = slice(name, 0, equals);
            has_inline = true;
        }

        member := find_member(info, name);
        if !member {
            if !(flags & .IGNORE_UNKNOWN) {
                result.ok = false;
                result.error_message = sprint("Unknown option '-%'.", name, allocator = temp);
                return result;
            }
            at += 1;
            continue;
        }

        storage := cast(*u8) options + member.offset_in_bytes;

        // A bool is set by being mentioned; everything else takes the next
        // argument.
        if member.type.type == .BOOL && !has_inline {
            (cast(*bool) storage).* = true;
            at += 1;
            continue;
        }

        value := inline_value;
        if !has_inline {
            if at + 1 >= args.count {
                result.ok = false;
                result.error_message = sprint("Option '-%' wants a value.", name, allocator = temp);
                return result;
            }
            value = args[at + 1];
            at += 1;
        }
        at += 1;

        if !assign_member(storage, member.type, value) {
            result.ok = false;
            result.error_message = sprint("Option '-%' cannot take '%'.", name, value, allocator = temp);
            return result;
        }
    }

    return result;
}

// What the options are, printed from the struct itself so that the help text
// cannot drift from what is accepted.
print_usage :: (options: *$T, program_name := "") {
    info := cast(*Type_Info_Struct) type_info(T);
    if program_name.count  print("Usage: % [options]\n\n", program_name);
    print("Options:\n");
    for * member: info.members {
        if member.flags & .CONSTANT  continue;
        print("    -%", member.name);
        if member.type.type != .BOOL  print(" <%>", type_name(member.type));
        print("\n");
    }
}

#scope_module

find_member :: (info: *Type_Info_Struct, name: string) -> *Type_Info_Struct_Member {
    for * member: info.members {
        if member.flags & .CONSTANT  continue;
        // A member is named with underscores and an option usually with
        // dashes, so the two spellings mean the same option.
        if member.name == name  return member;
        if names_match(member.name, name)  return member;
    }
    return null;
}

names_match :: (member: string, given: string) -> bool {
    if member.count != given.count  return false;
    for 0..member.count - 1 {
        a := member[it];
        b := given[it];
        if a == b  continue;
        if a == #char "_" && b == #char "-"  continue;
        return false;
    }
    return true;
}

assign_member :: (storage: *u8, type: *Type_Info, value: string) -> bool {
    if type.type == {
        case .STRING;
            (cast(*string) storage).* = value;
            return true;

        case .BOOL;
            if value == "true"  || value == "1" { (cast(*bool) storage).* = true;  return true; }
            if value == "false" || value == "0" { (cast(*bool) storage).* = false; return true; }
            return false;

        case .INTEGER;
            number, ok := string_to_int(value);
            if !ok  return false;
            integer := cast(*Type_Info_Integer) type;
            if integer.runtime_size == {
                case 1; (cast(*s8)  storage).* = cast(s8)  number;
                case 2; (cast(*s16) storage).* = cast(s16) number;
                case 4; (cast(*s32) storage).* = cast(s32) number;
                case;   (cast(*s64) storage).* = number;
            }
            return true;

        case .ENUM;
            // By member name, which is how an option reads: `-mode fast`.
            enum_info := cast(*Type_Info_Enum) type;
            for enum_info.names {
                if !equal_nocase(it, value)  continue;
                chosen := enum_info.values[it_index];
                size := ifx enum_info.internal_type then enum_info.internal_type.runtime_size else 8;
                if size == {
                    case 1; (cast(*s8)  storage).* = cast(s8)  chosen;
                    case 2; (cast(*s16) storage).* = cast(s16) chosen;
                    case 4; (cast(*s32) storage).* = cast(s32) chosen;
                    case;   (cast(*s64) storage).* = chosen;
                }
                return true;
            }
            return false;

        case;
            return false;
    }
}

type_name :: (type: *Type_Info) -> string {
    if type.type == {
        case .STRING;  return "string";
        case .INTEGER; return "number";
        case .BOOL;    return "bool";
        case .ENUM;    return (cast(*Type_Info_Enum) type).name;
        case;          return "value";
    }
}
