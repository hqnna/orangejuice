//
// The metaprogram that drives an ordinary build (**C§2.1**, **C§3.1**).
//
// `oj build foo.jai` compiles *this* and hands it the command line as
// `Build_Options.compile_time_command_line`; it creates the workspace the
// program is actually compiled in, watches that compile, and turns its own
// output off so nothing of the metaprogram itself is written.
//
// orangejuice parses the command line in Rust before this runs — the exit
// codes and the wording of a command-line error belong to the CLI — so what
// is left here is the part the reference also does at compile time: choosing
// the workspace's options, loading plugins, and driving the message loop.
//

#import "Basic";
#import "Compiler";
#import "Metaprogram_Plugins";
#import "String";

#run build();

build :: () {
    // This metaprogram produces nothing itself.
    set_build_options_dc(.{do_output = false});

    args := get_build_options().compile_time_command_line;
    if !args.count {
        compiler_report("No file was given to compile.", mode = .ERROR);
        return;
    }

    // The Rust side has already validated these; what is read here is only
    // what the metaprogram itself has to act on.
    options_from_command_line: Command_Line_State;
    parse_command_line(args, *options_from_command_line);

    if options_from_command_line.wants_help {
        log(HELP_STRING);
        return;
    }

    w := compiler_create_workspace("Target Program");
    if !w {
        compiler_report("Could not create a workspace.", mode = .ERROR);
        return;
    }

    options := get_build_options(w);
    copy_commonly_propagated_fields(get_build_options(), *options);

    if options_from_command_line.output_executable_name.count {
        options.output_executable_name = options_from_command_line.output_executable_name;
    }
    // Where the output goes: what was asked for, or beside the file being
    // compiled. A fresh `Build_Options` names the *metaprogram*'s directory,
    // which is the distribution's own modules folder and never what anyone
    // wanted.
    if options_from_command_line.output_path.count {
        options.output_path = options_from_command_line.output_path;
    } else {
        directory := path_directory(options_from_command_line.file);
        if directory.count  options.output_path = directory;
    }
    if options_from_command_line.import_paths.count {
        options.import_path = options_from_command_line.import_paths;
    }
    if options_from_command_line.release {
        options.optimization_level = 2;
        options.stack_trace = false;
    }
    // Whatever followed the lone `-` is the program's own command line, not
    // ours (**C§2.1**).
    options.compile_time_command_line = options_from_command_line.program_arguments;

    set_build_options(options, w);

    plugins: [..] *Metaprogram_Plugin;
    if options_from_command_line.plugin_names.count {
        init_plugins(options_from_command_line.plugin_names, *plugins, w);
    }

    intercept_flags: Intercept_Flags;
    run_before_intercept(plugins, *intercept_flags);

    compiler_begin_intercept(w, intercept_flags);
    add_build_file(options_from_command_line.file, w);
    run_add_source(plugins);

    failed := false;
    while true {
        message := compiler_wait_for_message();
        if !message  break;

        run_message(plugins, message);

        if message.kind == {
            case .ERROR;
                failed = true;

            case .COMPLETE;
                complete := cast(*Message_Complete) message;
                if complete.error_code != .NONE  failed = true;
                break;

            case;
        }
    }
    compiler_end_intercept(w);

    run_finish(plugins);
    run_shutdown(plugins);

    // The compilation's own diagnostics have already been reported; saying so
    // again here would print them twice.
    if failed  return;
}

// What this metaprogram takes out of the command line. Everything else has
// already been handled by the time it runs.
Command_Line_State :: struct {
    file:                   string;
    output_executable_name: string;
    output_path:            string;
    import_paths:           [..] string;
    plugin_names:           [..] string;
    program_arguments:      [..] string;
    release:                bool;
    wants_help:             bool;
}

#scope_module

// The first argument is the file; a lone `-` ends the options and sends the
// rest to the program being compiled (**C§2.1**).
parse_command_line :: (args: [] string, state: *Command_Line_State) {
    if args.count  state.file = args[0];

    at := 1;
    while at < args.count {
        argument := args[at];

        if argument == "-" {
            at += 1;
            while at < args.count {
                array_add(*state.program_arguments, args[at]);
                at += 1;
            }
            break;
        }

        if argument == {
            case "-help";    #through;
            case "-h";
                state.wants_help = true;
                at += 1;

            case "-release";
                state.release = true;
                at += 1;

            case "-exe";
                if at + 1 < args.count  state.output_executable_name = args[at + 1];
                at += 2;

            case "-output_path";
                if at + 1 < args.count  state.output_path = args[at + 1];
                at += 2;

            case "-import_dir";
                if at + 1 < args.count  array_add(*state.import_paths, args[at + 1]);
                at += 2;

            case "-plug";    #through;
            case "-plugin";
                if at + 1 < args.count  array_add(*state.plugin_names, args[at + 1]);
                at += 2;

            case "-no_check";
                // Nothing to do: a plugin nobody named is a plugin nobody
                // loads.
                at += 1;

            case;
                // An option this metaprogram does not act on. The Rust side
                // has already accepted or rejected it, so it is skipped
                // rather than reported twice.
                at += 1;
        }
    }
}

HELP_STRING :: #string DONE
Usage: oj build [options] <file.jai> [- program arguments]

    -exe <name>          Name the executable that gets written.
    -output_path <dir>   Choose where output goes.
    -import_dir <dir>    Add a directory to the module search path.
    -plug <module>       Load a metaprogram plugin.
    -no_check            Do not load the Check plugin.
    -release             Optimize, and leave stack traces out.
    -help                Print this.

    -                    Everything after this goes to the program being
                         compiled rather than to the compiler.
DONE
