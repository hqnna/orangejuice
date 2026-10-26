//
// Metaprogram_Plugins: loading and driving the plugins a build asked for.
//
// A plugin is a module exporting `get_plugin :: () -> *Metaprogram_Plugin`.
// The driver calls the hooks in order — `before_intercept`, `add_source`,
// then `message` for every message, then `finish` and `shutdown` — so a
// plugin sees the compilation without the metaprogram knowing what any
// particular plugin does.
//
// The `#placeholder` here is what the driver fills: `init_plugins` needs the
// list of plugin modules, and that list is only known once the command line
// has been read, which happens inside the very `#run` whose code is already
// compiled. The reference answers that by stalling the `#run` (**C§6.5**);
// orangejuice answers it by compiling again with the declaration in place
// (`docs/spec.md` §6.5).
//

#import "Basic";
#import "Compiler";

// `Metaprogram_Plugin` itself is declared in `Compiler`, since a build file
// that imports only that still names the type.

// Filled by the driver with a `get_plugin` call per named plugin. Until it is
// filled, `init_plugins` finds nothing and a build runs without plugins —
// which is what `-no_check` amounts to.
#placeholder PLUGIN_TABLE_INSERT;

// One plugin the command line asked for: `+Name` (or `+Name(params)`) and the
// arguments written after it, which are the plugin's own and reach its `init`
// (**C§3.4**).
Plugin_To_Create :: struct {
    name:    string;
    options: [..] string;
}

// Pulls every `+Name` out of `args`, each with the arguments that follow it up
// to the next `+Name`, a lone `-`, or the compiler's `--`/`---`. What is left
// comes back in order as `remaining`; nothing in it is rejected, since an
// argument this does not know is somebody else's to read. A lone `-` and
// everything after it are left alone: those are the program's arguments.
parse_plugin_arguments :: (args: [] string) -> (success: bool, plugins: [] Plugin_To_Create, remaining: [] string) {
    plugins: [..] Plugin_To_Create;
    remaining: [..] string;

    at := 0;
    while at < args.count {
        argument := args[at];
        if argument == "-" {
            while at < args.count {
                array_add(*remaining, args[at]);
                at += 1;
            }
            break;
        }
        if argument.count > 1 && argument[0] == #char "+" {
            plugin: Plugin_To_Create;
            plugin.name = advance(argument, 1);
            at += 1;
            while at < args.count && !ends_plugin_options(args[at]) {
                array_add(*plugin.options, args[at]);
                at += 1;
            }
            array_add(*plugins, plugin);
            continue;
        }
        array_add(*remaining, argument);
        at += 1;
    }
    return true, plugins, remaining;
}

// Fills the placeholder above with a table naming the plugins asked for, by
// adding the declaration to this very compilation. The compilation then runs
// again with it in place (`docs/spec.md` §6.5) and `make_plugin` can answer.
//
// This has to happen before `init_plugins` is any use, and it can only happen
// once the names are known — which is inside the `#run` whose code is already
// compiled. That circularity is the whole reason for the placeholder.
request_plugin_table :: (names: [] string) {
    builder: String_Builder;
    builder.allocator = temp;

    append(*builder, "PLUGIN_TABLE_INSERT :: #code {\n");
    for names {
        if it == "Check"  continue;
        // `Name(params)` is the module `Name` imported with those parameters.
        module, parameters := it, "";
        open := find_open_paren(it);
        if open >= 0 {
            module = slice_of(it, 0, open);
            parameters = slice_of(it, open, it.count - open);
        }
        print_to_builder(*builder, "    if name == \"%\" {\n", it);
        print_to_builder(*builder, "        M_% :: #import \"%\"%;\n", it_index, module, parameters);
        print_to_builder(*builder, "        return M_%.get_plugin();\n", it_index);
        append(*builder, "    }\n");
    }
    append(*builder, "};\n");

    add_build_string(builder_to_string(*builder,, allocator = temp), get_current_workspace(), PLUGIN_TABLE_SCOPE);
}

// Names this module's scope, which is where the table has to land for the
// `#placeholder` above to be the declaration it fills (**C§3.4**).
PLUGIN_TABLE_SCOPE :: #code {};

// Creates a plugin for each name, in the order given. A name nothing answers
// is reported and skipped rather than failing the build: a missing plugin
// should not stop a compilation that would otherwise succeed. What does fail
// it is a plugin whose `init` rejected the arguments it was given.
init_plugins :: (names: [] string, plugins: *[..] *Metaprogram_Plugin, w: Workspace) -> bool {
    to_create: [..] Plugin_To_Create;
    to_create.allocator = temp;
    for names  array_add(*to_create, .{name = it});
    return init_plugins(to_create, plugins, w);
}

// The same for what `parse_plugin_arguments` found: each plugin is created,
// pointed at `w`, and handed its own arguments through `init` (**C§3.4**). A
// plugin whose `init` says no fails the whole call, which is what a build
// script stops on.
init_plugins :: (to_create: [] Plugin_To_Create, plugins: *[..] *Metaprogram_Plugin, w: Workspace) -> bool {
    names: [..] string;
    names.allocator = temp;
    for to_create  array_add(*names, it.name);
    if names.count  request_plugin_table(names);

    ok := true;
    for to_create {
        plugin := make_plugin(it.name);
        if !plugin {
            compiler_report(tprint("There is no plugin named '%'.", it.name), mode = .WARNING);
            continue;
        }
        plugin.workspace = w;
        if plugin.init && !plugin.init(plugin, it.options) {
            ok = false;
            continue;
        }
        array_add(plugins, plugin);
    }
    return ok;
}

// The hooks, in the order the driver runs them.

run_before_intercept :: (plugins: [] *Metaprogram_Plugin, flags: *Intercept_Flags) {
    for plugins  if it.before_intercept  it.before_intercept(it, flags);
}

run_add_source :: (plugins: [] *Metaprogram_Plugin) {
    for plugins  if it.add_source  it.add_source(it);
}

run_message :: (plugins: [] *Metaprogram_Plugin, message: *Message) {
    for plugins  if it.message  it.message(it, message);
}

run_finish :: (plugins: [] *Metaprogram_Plugin) {
    for plugins  if it.finish  it.finish(it);
}

run_shutdown :: (plugins: [] *Metaprogram_Plugin) {
    for plugins  if it.shutdown  it.shutdown(it);
}

// Offers an argument to each plugin in turn. The first to claim it decides how
// many arguments went with it.
// Offers an argument to each plugin in turn. A plugin that recognises it says
// so by moving the cursor on; one that does not leaves it where it was.
handle_one_option :: (plugins: [] *Metaprogram_Plugin, args: [] string, cursor: int) -> (new_cursor: int, handled: bool) {
    for plugins {
        if !it.handle_one_option  continue;
        moved := it.handle_one_option(it, args, cursor);
        if moved <= cursor  continue;
        return moved, true;
    }
    return cursor, false;
}

log_plugin_help :: (plugins: [] *Metaprogram_Plugin) {
    for plugins {
        if !it.log_help  continue;
        it.log_help(it);
        log("\n");
    }
}

#scope_module

ends_plugin_options :: (argument: string) -> bool {
    if argument.count > 1 && argument[0] == #char "+"  return true;
    return argument == "-" || argument == "--" || argument == "---";
}

find_open_paren :: (s: string) -> int {
    for 0..s.count - 1  if s[it] == #char "("  return it;
    return -1;
}

slice_of :: (s: string, start: int, count: int) -> string {
    result: string;
    result.data = s.data + start;
    result.count = count;
    return result;
}

// The plugins this distribution ships answer directly. A user plugin is
// answered by the table the driver inserts, which needs the compilation to run
// again with the declaration in place — so a built-in that waited for that
// would not be available to the very `#run` that asked for it.
make_plugin :: (name: string) -> *Metaprogram_Plugin {
    if name == "Check" {
        Check :: #import "Check";
        return Check.get_plugin();
    }
    #insert,scope() PLUGIN_TABLE_INSERT;
    return null;
}
