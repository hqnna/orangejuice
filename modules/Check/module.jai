//
// Check: a plugin that reads the trees the compiler typechecked and reports
// mistakes the language itself allows.
//
// The compiler will not stop you passing one argument to a `print` whose
// format string wants two — the arguments are varargs and every one of them
// is a legal `Any`. That is exactly the kind of thing this looks for: correct
// programs that are nonetheless wrong.
//
// Which procedures get checked is decided by the `@PrintLike` and `@ScanLike`
// notes on their headers, not by a list of names here, so a procedure of one's
// own taking a format string is checked too.
//
// Loaded by `Default_Metaprogram` unless `-no_check` says otherwise, which is
// what makes it run without anyone asking.
//

#import "Basic";
#import "Compiler";
#import "Metaprogram_Plugins";
#import "String";

#module_parameters (CHECK_BINDINGS := true);

Check_Plugin :: struct {
    #as using base: Metaprogram_Plugin;
}

get_plugin :: () -> *Metaprogram_Plugin {
    plugin := New(Check_Plugin);
    plugin.before_intercept = before_intercept;
    plugin.message          = message;
    plugin.shutdown         = shutdown;
    return plugin;
}

#scope_module

before_intercept :: (plugin: *Metaprogram_Plugin, flags: *Intercept_Flags) {
    // Nothing to ask for: the typechecked batches arrive anyway.
}

shutdown :: (plugin: *Metaprogram_Plugin) {
    free(plugin);
}

message :: (plugin: *Metaprogram_Plugin, message: *Message) {
    if message.kind != .TYPECHECKED  return;

    batch := cast(*Message_Typechecked) message;

    // Per *body*, rather than over everything in the batch: a body arrives
    // once, so nothing is reported twice, and the subexpressions of one are
    // exactly the calls written inside it.
    for batch.procedure_bodies {
        body := it.expression;
        // A polymorphic body is not a program yet; its specializations arrive
        // separately.
        if body.header && (body.header.procedure_flags & .POLYMORPHIC)  continue;

        check_print_calls(it);
    }
}

check_print_calls :: (tc: Typechecked(Code_Procedure_Body)) {
    for tc.subexpressions {
        if it.kind != .PROCEDURE_CALL  continue;

        call := cast(*Code_Procedure_Call) it;

        // `Basic.print(…)` is a dot whose right side is the name.
        expression := call.procedure_expression;
        while expression && expression.kind == .BINARY_OPERATOR {
            expression = (cast(*Code_Binary_Operator) expression).right;
        }
        if !expression || expression.kind != .IDENT  continue;

        ident := cast(*Code_Ident) expression;

        resolved: *Code_Declaration;
        if call.overloads  resolved = call.overloads.*[0];
        else               resolved = ident.resolved_declaration;

        if !resolved                                    continue;
        if !resolved.expression                         continue;
        if resolved.expression.kind != .PROCEDURE_HEADER continue;

        header := cast(*Code_Procedure_Header) resolved.expression;
        if !is_printlike_or_scanlike(header)  continue;

        if !call.arguments_sorted.count  continue;

        // A spread hands over an array whose length is not known here, so the
        // call is left alone.
        last := call.arguments_sorted[call.arguments_sorted.count - 1];
        if last && (last.node_flags & .EXPRESSION_IS_SPREAD)  continue;

        format_string_index, varargs_index := get_argument_indices_of_format_string_and_varargs(header);
        // A header whose shape makes no sense is the `#PrintLike` itself being
        // wrong, which is not this call's fault.
        if format_string_index < 0  continue;
        if format_string_index >= call.arguments_sorted.count  continue;
        if varargs_index       >= call.arguments_sorted.count  continue;

        expr := call.arguments_sorted[format_string_index];
        if !expr || expr.kind != .LITERAL  continue;

        format_string := cast(*Code_Literal) expr;
        if format_string.value_type != .STRING  continue;

        number_of_percents := count_percents(format_string._string, format_string);

        // A header with no `..` at all is one that is only ever given the format
        // string — `Basic.print`'s overload for text with nothing to format.
        number_of_varargs := 0;
        if varargs_index >= 0 {
            va_expr := call.arguments_sorted[varargs_index];
            if !va_expr  continue;
            if va_expr.kind != .MAKE_VARARGS {
                // An array in the slot is a spread we cannot count.
                continue;
            }
            number_of_varargs = (cast(*Code_Make_Varargs) va_expr).expressions.count;
        }

        if number_of_percents == number_of_varargs  continue;

        percents_string := "1 argument";
        if number_of_percents != 1  percents_string = tprint("% arguments", number_of_percents);

        varargs_string := "1 argument is given";
        if number_of_varargs != 1  varargs_string = tprint("% arguments are given", number_of_varargs);

        error_string := tprint("Incorrect number of arguments supplied to '%': The format string requires %, but %.\n", ident.name, percents_string, varargs_string);

        // Too few is an error — something will print as a complaint about a
        // missing argument. Too many only wastes them.
        mode := Report.ERROR;
        if number_of_percents < number_of_varargs  mode = .WARNING;
        compiler_report(get_filename(call), call.l0, call.c0, error_string, mode = mode);
    }
}

is_printlike_or_scanlike :: (header: *Code_Procedure_Header) -> bool {
    for header.notes {
        if it.text == "PrintLike"  return true;
        if it.text == "ScanLike"   return true;
    }
    return false;
}

is_printlike :: (header: *Code_Procedure_Header) -> bool {
    for header.notes  if it.text == "PrintLike"  return true;
    return false;
}

// The format string is the parameter before the `..` one, or the last one when
// there is no `..`.
get_argument_indices_of_format_string_and_varargs :: (proc: *Code_Procedure_Header) -> (format_string_index: int, varargs_index: int) {
    varargs_index := -1;
    for proc.arguments {
        inst := it.type_inst;
        if !inst  continue;

        if inst.inst_flags & .VARARGS {
            varargs_index = it_index;
            break;
        }
    }

    if varargs_index == -1  return proc.arguments.count - 1, -1;
    // Varargs first leaves no room for a format string before it.
    if varargs_index == 0   return -1, 0;

    return varargs_index - 1, varargs_index;
}

// How many arguments a format string consumes. `%3` names the third one
// rather than the next, so what matters is the highest index reached, not the
// count of `%`s.
count_percents :: (s: string, site: *Code_Literal) -> int {
    current_index := 0;
    max_index     := 0;

    at := 0;
    while at < s.count {
        if s[at] != #char "%" {
            at += 1;
            continue;
        }
        at += 1;

        if at >= s.count {
            current_index += 1;
        } else if s[at] == #char "%" {
            loc := make_location(site);
            loc.character_number += at;
            compiler_report("The substring '%%' in print statements is about to change meaning; it will soon no longer mean 'put an ASCII % here'. Use '\\%' for this purpose instead.\n", loc, mode = .WARNING);
            at += 1;
        } else if s[at] == #char "0" {
            // `%00` is the empty string and takes nothing; `%0` is the next
            // argument, the same as a bare `%`.
            if at + 1 < s.count && s[at + 1] == #char "0" {
                at += 2;
            } else {
                at += 1;
                current_index += 1;
            }
        } else if s[at] >= #char "1" && s[at] <= #char "9" {
            n := 0;
            while at < s.count && s[at] >= #char "0" && s[at] <= #char "9" {
                n = n * 10 + (s[at] - #char "0");
                at += 1;
            }
            current_index = n;
        } else {
            current_index += 1;
        }

        if current_index > max_index  max_index = current_index;
    }

    return max_index;
}
