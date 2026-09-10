//
// Check: a plugin that reads the trees the compiler typechecked and reports
// mistakes the language itself allows.
//
// The compiler will not stop you passing four arguments to a `print` whose
// format string wants two — the arguments are varargs and every one of them
// is a legal `Any`. That is exactly the kind of thing this looks for: correct
// programs that are nonetheless wrong.
//
// Loaded by `Default_Metaprogram` unless `-no_check` says otherwise, which is
// what makes it run without anyone asking.
//

#import "Basic";
#import "Compiler";
#import "Metaprogram_Plugins";
#import "String";
#import "Hash_Table";

#module_parameters (CHECK_BINDINGS := true);

Check_Plugin :: struct {
    #as using base: Metaprogram_Plugin;

    // Which nodes have already been reported. One node reaches this plugin
    // several times — it is in its batch's `all` list and in the
    // subexpressions of whatever encloses it — and a warning printed three
    // times is three times as easy to ignore.
    seen: Table(s64, bool);
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
    check := cast(*Check_Plugin) plugin;
    deinit(*check.seen);
    free(plugin);
}

message :: (plugin: *Metaprogram_Plugin, message: *Message) {
    if message.kind != .TYPECHECKED  return;

    check := cast(*Check_Plugin) plugin;
    batch := cast(*Message_Typechecked) message;

    for batch.all  check_node(check, it.expression);
    for batch.all {
        for it.subexpressions  check_node(check, it);
    }
}

// One node. Only the shapes this plugin knows how to be sure about are
// reported: a false positive in a checker that runs by default is worse than
// a miss, because it makes people turn the checker off.
check_node :: (check: *Check_Plugin, node: *Code_Node) {
    if !node  return;
    if node.kind != cast(u8) Code_Node_Kind.PROCEDURE_CALL  return;

    call := cast(*Code_Procedure_Call) node;
    check_format_arguments(check, call);
}

// A call whose first argument is a literal format string, and whose remaining
// arguments are the ones it takes: `print("% %", a)` wants two and has one.
check_format_arguments :: (check: *Check_Plugin, call: *Code_Procedure_Call) {
    name := called_name(call);
    if !takes_a_format_string(name)  return;
    if !call.arguments_unsorted.count  return;

    first := call.arguments_unsorted[0].expression;
    if !first  return;
    if first.kind != cast(u8) Code_Node_Kind.LITERAL  return;

    literal := cast(*Code_Literal) first;
    if literal.value_type != .STRING  return;

    wanted := count_format_slots(literal._string);
    // Everything after the format string is a value for it. A spread is not
    // countable, so a call using one is left alone.
    given := call.arguments_unsorted.count - 1;
    for 1..call.arguments_unsorted.count - 1 {
        argument := call.arguments_unsorted[it].expression;
        if argument && (argument.node_flags & .EXPRESSION_IS_SPREAD)  return;
        // A named argument is not filling a `%`.
        if call.arguments_unsorted[it].name  given -= 1;
    }

    if given == wanted  return;

    if table_contains(*check.seen, call.serial)  return;
    table_set(*check.seen, call.serial, true);
    report_at(call, tprint("'%' wants % format argument% but was given %.",
        name, wanted, ifx wanted == 1 then "" else "s", given));
}

// The procedures whose first argument is a format string. A user procedure
// that forwards varargs is not one of these: it has no format string of its
// own to check.
takes_a_format_string :: (name: string) -> bool {
    if name == "print"            return true;
    if name == "sprint"           return true;
    if name == "tprint"           return true;
    if name == "print_to_builder" return true;
    if name == "log"              return true;
    if name == "log_error"        return true;
    return false;
}

// How many arguments a format string consumes. `%%` is a literal percent and
// takes none, which is the case a naive count gets wrong.
count_format_slots :: (format: string) -> s64 {
    total := 0;
    at := 0;
    while at < format.count {
        if format[at] != #char "%" {
            at += 1;
            continue;
        }
        at += 1;
        if at < format.count && format[at] == #char "%" {
            at += 1;
            continue;
        }
        total += 1;
    }
    return total;
}

// The name a call was written with: the rightmost identifier of its procedure
// expression, so `Basic.print(...)` is "print".
called_name :: (call: *Code_Procedure_Call) -> string {
    node := call.procedure_expression;
    while node {
        if node.kind == cast(u8) Code_Node_Kind.IDENT {
            return (cast(*Code_Ident) node).name;
        }
        if node.kind == cast(u8) Code_Node_Kind.BINARY_OPERATOR {
            binary := cast(*Code_Binary_Operator) node;
            node = binary.right;
            continue;
        }
        break;
    }
    return "";
}

report_at :: (node: *Code_Node, text: string) {
    location: Source_Code_Location;
    if node.location.enclosing_load {
        location.fully_pathed_filename = node.location.enclosing_load.fully_pathed_filename;
    }
    location.line_number = node.location.l0;
    location.character_number = node.location.c0;
    compiler_report(text, loc = location, mode = .WARNING);
}
