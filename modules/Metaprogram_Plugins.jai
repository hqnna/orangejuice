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

Metaprogram_Plugin :: struct {
    name:      string;
    workspace: Workspace;

    // Called before `compiler_begin_intercept`, so a plugin can ask for the
    // messages it needs.
    before_intercept: (plugin: *Metaprogram_Plugin, flags: *Intercept_Flags);
    // Called after intercepting starts, for a plugin that adds code.
    add_source:       (plugin: *Metaprogram_Plugin);
    // Every message, in the order the compiler produced them.
    message:          (plugin: *Metaprogram_Plugin, message: *Message);
    // Called once the compilation is complete, before shutdown.
    finish:           (plugin: *Metaprogram_Plugin);
    // Release whatever the plugin allocated.
    shutdown:         (plugin: *Metaprogram_Plugin);

    // Lets a plugin take command-line options of its own. Returns how many
    // arguments it consumed, and whether it recognised the first.
    handle_one_option: (plugin: *Metaprogram_Plugin, args: [] string, index: s64) -> (new_index: s64, handled: bool);
    // Prints the plugin's own help.
    log_help:          (plugin: *Metaprogram_Plugin);
}

// Filled by the driver with a `get_plugin` call per named plugin. Until it is
// filled, `init_plugins` finds nothing and a build runs without plugins —
// which is what `-no_check` amounts to.
#placeholder PLUGIN_TABLE_INSERT;

// Fills the placeholder above with a table naming the plugins asked for, by
// adding the declaration to this very compilation. The compilation then runs
// again with it in place (`docs/spec.md` §6.5) and `make_plugin` can answer.
//
// This has to happen before `init_plugins` is any use, and it can only happen
// once the names are known — which is inside the `#run` whose code is already
// compiled. That circularity is the whole reason for the placeholder.
request_plugin_table :: (names: [] string) {
    builder: String_Builder;
    init_string_builder(*builder, temp);

    append(*builder, "PLUGIN_TABLE_INSERT :: #code {\n");
    for names {
        print_to_builder(*builder, "    if name == \"%\" {\n", it);
        print_to_builder(*builder, "        M_% :: #import \"%\";\n", it_index, it);
        print_to_builder(*builder, "        return M_%.get_plugin();\n", it_index);
        append(*builder, "    }\n");
    }
    append(*builder, "};\n");

    add_build_string(builder_to_string(*builder, temp), get_current_workspace());
}

// Creates a plugin for each name, in the order given. A name nothing answers
// is reported and skipped rather than failing the build: a missing plugin
// should not stop a compilation that would otherwise succeed.
init_plugins :: (names: [] string, plugins: *[..] *Metaprogram_Plugin, w: Workspace) -> bool {
    ok := true;
    for names {
        plugin := make_plugin(it);
        if !plugin {
            compiler_report(tprint("There is no plugin named '%'.", it), mode = .WARNING);
            ok = false;
            continue;
        }
        plugin.name = it;
        plugin.workspace = w;
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
handle_one_option :: (plugins: [] *Metaprogram_Plugin, args: [] string, index: s64) -> (new_index: s64, handled: bool) {
    for plugins {
        if !it.handle_one_option  continue;
        new_index, handled := it.handle_one_option(it, args, index);
        if !handled  continue;
        // A plugin that moves the index backwards would loop forever.
        if new_index <= index {
            compiler_report(tprint("Plugin % did not advance the argument index.", it.name), mode = .ERROR);
            return index + 1, true;
        }
        return new_index, true;
    }
    return index, false;
}

log_plugin_help :: (plugins: [] *Metaprogram_Plugin) {
    for plugins {
        if !it.log_help  continue;
        log("---- Help for plugin '%': ----\n", it.name);
        it.log_help(it);
        log("\n");
    }
}

#scope_module

// The plugins this distribution ships answer directly. A user plugin is
// answered by the table the driver inserts, which needs the compilation to run
// again with the declaration in place — so a built-in that waited for that
// would not be available to the very `#run` that asked for it.
make_plugin :: (name: string) -> *Metaprogram_Plugin {
    if name == "Check" {
        Check :: #import "Check";
        return Check.get_plugin();
    }
    #insert PLUGIN_TABLE_INSERT;
    return null;
}
