//
// Starting other programs.
//
// The API is built around an *array* of arguments rather than one command
// string: a program launched from a string has to be quoted by whoever builds
// it, and getting that wrong is how a space in a filename turns into two
// arguments. Keeping them apart all the way to `execvp` means nothing in them
// is ever re-parsed.
//
// `run_command` starts a program, waits for it and hands back what it wrote.
// Underneath it, `create_process` and the read/write/kill/reap routines are
// there for a caller that wants to talk to the child while it runs.
//
// A relative command path is resolved by the *child*, against its own working
// directory, and a bare executable name is looked up only on `PATH` — not in
// this process's directory and not in `working_directory`. Resolving a
// relative path before calling is the safe thing to do.
//

#import "Basic";
#import "POSIX";
#import "String";

#load "posix.jai";

// Breaks a command line into arguments, in temporary storage. An argument may
// be quoted to keep spaces in it, but only at its start: this is a
// convenience for a command typed into a text box, not a shell.
break_command_into_strings :: (input: string) -> [..] string {
    remaining := input;

    result: [..] string;
    result.allocator = temp;

    while remaining.count {
        s := trim_left(remaining);
        if !s  break;

        delimiter: u8 = #char " ";
        if s[0] == #char "\"" {
            advance(*s, 1);
            delimiter = #char "\"";
        }

        index := find_index_from_left(s, delimiter);
        if index == -1 {
            // An unterminated quote ends up here too, and is taken as the
            // rest of the line.
            array_add(*result, s);
            remaining = "";
        } else {
            remaining = advance(s, index + 1);
            s.count = index;
            array_add(*result, trim_right(s));
        }
    }

    return result;
}

// Runs `args[0]` with the rest as its arguments and waits for it.
//
// `capture_and_return_output` reads the child's stdout and stderr into the two
// returned strings; without it the child writes wherever this process does.
// `timeout_ms` of -1 waits as long as the child takes; anything else kills it
// once the time is up and says so in `timeout_reached`.
run_command :: (args: .. string, working_directory := "", capture_and_return_output := false, print_captured_output := false, timeout_ms := -1, arg_quoting := Process_Argument_Quoting.QUOTE_IF_NEEDED) -> (process_result: Process_Result, output_string := "", error_string := "", timeout_reached := false) {
    process: Process;
    if !create_process(*process, ..args, working_directory, capture_and_return_output, arg_quoting) {
        return .{type = .FAILED_TO_LAUNCH};
    }

    defer deinit(*process);

    output_string: string;
    error_string:  string;
    timeout_reached := false;

    exit_timeout_ms := timeout_ms;
    if capture_and_return_output {
        output_builder: String_Builder;
        init_string_builder(*output_builder);
        error_builder: String_Builder;
        init_string_builder(*error_builder);

        start_time := current_time_monotonic();
        remaining_timeout_ms := timeout_ms;

        while true {
            // Both pipes are read every time round: a child that fills the one
            // nobody is reading blocks forever, and which one that is depends
            // on the child.
            output_buffer := ensure_contiguous_space_and_return_available_range(*output_builder);
            error_buffer  := ensure_contiguous_space_and_return_available_range(*error_builder);
            success, output_bytes, error_bytes := read_from_process(*process, output_buffer, error_buffer, remaining_timeout_ms);

            if output_bytes {
                advance_through_ensured_space(*output_builder, output_bytes);
                if print_captured_output  write_string(to_string(output_buffer.data, output_bytes), to_standard_error = false);
            }
            if error_bytes {
                advance_through_ensured_space(*error_builder, error_bytes);
                if print_captured_output  write_string(to_string(error_buffer.data, error_bytes), to_standard_error = true);
            }

            if !success {
                log_error("Could not read output of command \"%\"", get_quoted_command_string(args,, temp));
                kill_process(*process);
                break;
            }

            if timeout_ms >= 0 {
                now := current_time_monotonic();
                remaining_timeout_ms = cast(s32) (timeout_ms - to_milliseconds(now - start_time));

                if remaining_timeout_ms <= 0 {
                    timeout_reached = true;
                    if !kill_process(*process) {
                        error_code, description := get_error_value_and_string();
                        log_error("Could not kill child for command \"%\": % %", get_quoted_command_string(args,, temp), error_code, description);
                    }
                    break;
                }
            }

            if process.output.eof && process.error.eof  break;
        }

        output_string = builder_to_string(*output_builder);
        error_string  = builder_to_string(*error_builder);
        // The child is already finished or killed by now, so the reap below
        // need not wait.
        exit_timeout_ms = -1;
    }

    process_result: Process_Result;
    while true {
        success, result := get_process_result(*process, exit_timeout_ms);
        process_result = result;
        if !success {
            error_code, description := get_error_value_and_string();
            log_error("Could not get process result for command \"%\": % %", get_quoted_command_string(args,, temp), error_code, description);
            kill_process(*process);
            process_result = .{type = .EXITED, exit_code = -1};
            break;
        }

        if process_result.type != .STILL_RUNNING  break;

        // The wait timed out, so the child is killed and reaped on the next
        // time round, which cannot time out.
        assert(exit_timeout_ms >= 0);
        exit_timeout_ms = -1;
        timeout_reached = true;
        if !kill_process(*process) {
            error_code, description := get_error_value_and_string();
            log_error("Could not kill child for command \"%\": % %", get_quoted_command_string(args,, temp), error_code, description);
            process_result = .{type = .EXITED, exit_code = -1};
            break;
        }
    }

    return process_result, output_string, error_string, timeout_reached;
}

// Runs a command line through the shell, so redirection and globbing work.
// Whatever is in `command` is interpreted by `/bin/sh`, so a caller building
// one out of untrusted input is asking for trouble.
run_shell_command :: (command: string) -> Process_Result {
    result: Process_Result;
    c_command := to_c_string(command, temp);
    if !c_command {
        result.type = .FAILED_TO_LAUNCH;
        return result;
    }

    status := system(c_command);
    if status < 0 {
        result.type = .FAILED_TO_LAUNCH;
        return result;
    }
    if WIFEXITED(status) {
        result.type = .EXITED;
        result.exit_code = WEXITSTATUS(status);
    } else {
        result.type = .SIGNALED;
        result.signal = WTERMSIG(status);
    }
    return result;
}

// Whether an argument with a space in it should be wrapped in quotes. It makes
// no difference here — `execvp` takes the array as it stands — and exists
// because a program written for the reference's Windows side passes it.
Process_Argument_Quoting :: enum {
    QUOTE_IF_NEEDED :: 0;
    NEVER_QUOTE     :: 1;
}

Process_Result_Type :: enum s32 {
    UNSTARTED;
    STILL_RUNNING;
    FAILED_TO_LAUNCH;
    EXITED;
    SIGNALED;
}

Process_Result :: struct {
    type: Process_Result_Type;
    // What the child returned. -1 until it has exited.
    exit_code: s32 = -1;
    // What killed it, when `type` is SIGNALED.
    signal: s32;
}

// The argument list as one line, for a log message. Not for handing to
// anything that will parse it again.
get_quoted_command_string :: (args: [] string) -> string {
    builder: String_Builder;
    for args {
        append_quoted(*builder, it);
        if it_index < args.count - 1  append(*builder, " ");
    }
    return builder_to_string(*builder);
}

// An argument holding a space or a quote is wrapped in quotes, with the quotes
// inside it backslashed; anything else is appended as it stands.
append_quoted :: (builder: *String_Builder, s: string) {
    needs_quote := false;
    for 0..s.count - 1 {
        c := s[it];
        if c == #char " " || c == #char "\"" {
            needs_quote = true;
            break;
        }
    }

    if !needs_quote {
        append(builder, s);
        return;
    }

    append(builder, "\"");

    remainder := s;
    while true {
        found, left, right := split_from_left(remainder, #char "\"");
        if !found  break;

        append(builder, left);
        append(builder, "\\\"");

        remainder = right;
    }

    append(builder, remainder);
    append(builder, "\"");
}

#scope_module

System :: #import "System";

get_error_value_and_string :: System.get_error_value_and_string;
