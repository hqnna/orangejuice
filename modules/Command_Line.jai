//
// Command_Line: the argument list filled into a struct the program declared.
//
//     Arguments :: struct {
//         verbose:  bool;      @"?Say more about what is happening."
//         threads:  int;       @"?How many workers to run."
//         out_path: string;
//     }
//
//     success, args, is_set, free_arguments := parse_arguments(Arguments);
//
// The struct's members *are* the option names, found by reflection, so adding
// an option means adding a member and nothing else. A `bool` member is a flag
// and takes no value; everything else takes the argument that follows it.
//
// `is_set` says which options the caller actually wrote, which is not the same
// as which differ from their defaults — a caller who typed the default value
// on purpose is indistinguishable otherwise.
//
// A member's note beginning with `?` is its help text, printed for `-help`.
//

#import "Basic";
#import "Reflection";

// A parallel struct with one `bool` per member of `T`, generated from `T`'s
// own type info: `is_set.threads` answers for `args.threads`.
Is_Set :: struct (T: Type) {
    #insert -> string {
        info := type_info(T);

        builder: String_Builder;
        init_string_builder(*builder, temp);
        for * member: info.members {
            if should_skip(member)  continue;
            print_to_builder(*builder, "    % := false;\n", member.name);
        }
        return builder_to_string(*builder);
    };
}

Argument_Flags :: enum_flags u64 {
    // An argument with no dash is collected rather than rejected.
    FREE_ARGUMENTS_ALLOWED :: 0x1;
    // Options are written `--name` rather than `-name`.
    DOUBLE_DASH_REQUIRED   :: 0x2;
    SHOW_HELP_ON_ERROR     :: 0x4;
    // Help lists the options in name order rather than declaration order.
    SORT_HELP              :: 0x8;
    // Help lines up the descriptions in a column.
    ALIGN_HELP             :: 0x10;
}

Default_Argument_Flags :: Argument_Flags.FREE_ARGUMENTS_ALLOWED | .SORT_HELP | .ALIGN_HELP;

parse_arguments :: ($T: Type, flags := Default_Argument_Flags, help_triggers: [] string = .["help", "HELP", "?"]) -> (success: bool, result: T, is_set: Is_Set(T), free_arguments: [..] string) {
    result: T;
    is_set: Is_Set(T);
    free_arguments: [..] string;

    info := type_info(T);
    args := get_command_line_arguments();

    // `Is_Set`'s members are `bool`s in the same order as `T`'s, so the flag
    // for member `n` is the `n`th byte.
    set_flags := cast(*bool) *is_set;

    success := true;
    only_free := false;
    cursor := 1;

    while cursor < args.count {
        argument := args[cursor];

        dashed := argument.count > 0 && argument[0] == #char "-";
        if dashed && !only_free {
            if argument == "--" && (flags & .FREE_ARGUMENTS_ALLOWED) {
                // Everything after a bare `--` is a value, even if it looks
                // like an option.
                only_free = true;
                cursor += 1;
                continue;
            }
            if flags & .DOUBLE_DASH_REQUIRED  dashed = argument.count > 1 && argument[1] == #char "-";
        }

        if !dashed || only_free {
            if !(flags & .FREE_ARGUMENTS_ALLOWED) {
                dashes := ifx flags & .DOUBLE_DASH_REQUIRED then "--" else "-";
                log_error("Command-line argument % does not start with '%'.\n", cursor, dashes);
                return false, result, is_set, free_arguments;
            }
            array_add(*free_arguments, argument);
            cursor += 1;
            continue;
        }

        name := argument;
        advance(*name, 1);
        if flags & .DOUBLE_DASH_REQUIRED  advance(*name, 1);

        found := false;
        index := -1;
        for * member: info.members {
            if should_skip(member)   continue;
            index += 1;
            if member.name != name   continue;

            found = true;
            slot := cast(*u8) *result + member.offset_in_bytes;
            if !take_value(slot, member.type, name, args, *cursor) {
                success = false;
            } else {
                if set_flags[index] {
                    log_error("Argument '%' was given more than once.\n", name);
                    success = false;
                }
                set_flags[index] = true;
            }
            break;
        }

        if !found {
            for help_triggers {
                if name != it  continue;
                show_help(T, flags, help_triggers);
                // Asking for help is not a successful parse: there is nothing
                // for the caller to go on and do.
                return false, result, is_set, free_arguments;
            }

            log_error("Command-line argument % ('%') is not valid.\n", cursor, name);
            success = false;
            if flags & .SHOW_HELP_ON_ERROR       show_help(T, flags, help_triggers);
            else if help_triggers.count          log_error("Use -% to list the valid arguments.\n", help_triggers[0]);
        }

        if !success  break;
        cursor += 1;
    }

    return success, result, is_set, free_arguments;
}

// What `-help` prints: every option, its type, and the note it was declared
// with.
show_help :: ($T: Type, flags := Default_Argument_Flags, help_triggers: [] string = .["help", "HELP", "?"]) {
    info := type_info(T);

    names: [..] string;
    notes: [..] string;
    names.allocator = temp;
    notes.allocator = temp;

    widest := 0;
    for * member: info.members {
        if should_skip(member)  continue;
        array_add(*names, member.name);
        array_add(*notes, help_note(member));
        widest = max(widest, member.name.count);
    }

    if flags & .SORT_HELP  sort_together(names, notes);

    lines: [..] string;
    lines.allocator = temp;
    for names {
        note := notes[it_index];
        if !note.count {
            array_add(*lines, tprint("-%", it));
            continue;
        }

        builder: String_Builder;
        init_string_builder(*builder, temp);
        print_to_builder(*builder, "-%: ", it);
        // The descriptions line up in a column, so the padding goes after the
        // colon rather than before it.
        if flags & .ALIGN_HELP {
            for 1..widest - it.count  append(*builder, #char " ");
        }
        append(*builder, note);
        array_add(*lines, builder_to_string(*builder, temp));
    }

    if help_triggers.count {
        // A deliberately blank line before the help trigger itself.
        array_add(*lines, "");
        array_add(*lines, tprint("-%: Show the list of commands.", join(..help_triggers, ", -", allocator = temp)));
    }

    if lines.count == 0 {
        if flags & .FREE_ARGUMENTS_ALLOWED  print("----- There are no valid command-line options, just free arguments. -----\n");
        else                                print("----- There are no valid command-line options. -----\n");
    } else if lines.count == 1 {
        print("----- Here is the only valid argument: -----\n");
    } else {
        print("----- Here is the list of valid arguments: -----\n");
    }

    for lines  print("    %\n", it);
}

#scope_module

// A member the parser has no business filling in: a constant has no storage,
// and a `using`-brought member belongs to the type it came from.
should_skip :: (member: *Type_Info_Struct_Member) -> bool {
    if member.flags & .CONSTANT  return true;
    return false;
}

// The note the member was declared with, minus the `?` that marks it as help.
help_note :: (member: *Type_Info_Struct_Member) -> string {
    for member.notes {
        if it.count && it[0] == #char "?"  return slice(it, 1, it.count - 1);
    }
    return "";
}


// Fills one member from the argument list, advancing `cursor` past whatever it
// consumed. A `bool` consumes nothing: writing the flag is the whole meaning.
take_value :: (slot: *u8, info: *Type_Info, name: string, args: [] string, cursor: *s64) -> bool {
    if info.type == .BOOL {
        (cast(*bool) slot).* = true;
        return true;
    }

    if info.type == .ARRAY {
        array := cast(*Type_Info_Array) info;
        if array.array_type != .FIXED {
            log_error("Argument '%' has a type the command line cannot fill.\n", name);
            return false;
        }
        for 0..array.array_count - 1 {
            element := slot + array.element_type.runtime_size * it;
            if !take_value(element, array.element_type, name, args, cursor)  return false;
        }
        return true;
    }

    value, ok := next_argument(name, args, cursor);
    if !ok  return false;

    if info.type == {
        case .STRING;
            (cast(*string) slot).* = copy_string(value);
            return true;

        case .INTEGER;
            integer := cast(*Type_Info_Integer) info;
            number, number_ok, remainder := string_to_int(value);
            if !number_ok || remainder.count {
                log_error("Value of argument '%' was not an integer (it was '%').\n", name, value);
                return false;
            }
            fits, low, high := range_check_and_store(number, integer, slot);
            if !fits {
                log_error("Value of argument '%' (%) is outside the range % to %.\n", name, number, low, high);
                return false;
            }
            return true;

        case .FLOAT;
            number, number_ok, remainder := string_to_float64(value);
            if !number_ok || remainder.count {
                log_error("Value of argument '%' was not a float (it was '%').\n", name, value);
                return false;
            }
            // Braced because a parenthesis right after the condition would
            // otherwise read as a call on it.
            if info.runtime_size == 4 {
                (cast(*float32) slot).* = cast(float32) number;
            } else {
                (cast(*float64) slot).* = number;
            }
            return true;

        case .ENUM;
            enum_info := cast(*Type_Info_Enum) info;
            number, number_ok := enum_value(enum_info, value);
            if !number_ok {
                log_error("Value of argument '%' was not one of %'s names (it was '%').\n", name, enum_info.name, value);
                return false;
            }
            if enum_info.internal_type {
                fits := range_check_and_store(number, enum_info.internal_type, slot);
                if !fits  return false;
                return true;
            }
            (cast(*s64) slot).* = number;
            return true;

        case;
            log_error("Argument '%' has a type the command line cannot fill.\n", name);
            return false;
    }
}

next_argument :: (name: string, args: [] string, cursor: *s64) -> (value: string, ok: bool) {
    if cursor.* + 1 >= args.count {
        log_error("Argument '%' needs a value, and there was none after it.\n", name);
        return "", false;
    }
    cursor.* += 1;
    return args[cursor.*], true;
}

// An insertion sort over the two arrays at once, so the help text stays with
// the name it belongs to. The list is short enough that nothing cleverer pays.
sort_together :: (names: [] string, helps: [] string) {
    for 1..names.count - 1 {
        name := names[it];
        help := helps[it];
        at := it - 1;
        while at >= 0 && compare(names[at], name) > 0 {
            names[at + 1] = names[at];
            helps[at + 1] = helps[at];
            at -= 1;
        }
        names[at + 1] = name;
        helps[at + 1] = help;
    }
}
