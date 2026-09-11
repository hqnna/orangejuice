//
// Running other programs.
//
// `run_command` is the whole point: fork, exec, wait, and hand back what the
// child wrote. The child's output is read through a pipe while it runs rather
// than after it exits, because a child that fills the pipe buffer and blocks
// would otherwise never exit and never be waited for. stdout and stderr get a
// pipe each, and both are polled, for the same reason.
//

#import "Basic";
#import "POSIX";
#import "String";

Process_Result_Type :: enum s32 {
    UNSTARTED        :: 0;
    STILL_RUNNING    :: 1;
    FAILED_TO_LAUNCH :: 2;
    EXITED           :: 3;
    SIGNALED         :: 4;
}

Process_Result :: struct {
    type: Process_Result_Type;
    // What the child returned. -1 until it has exited.
    exit_code: s32 = -1;
    // What killed it, when `type` is SIGNALED.
    signal: s32;
}

// On Linux the argument list goes to `execvp` as it stands, so there is no
// shell to quote against; the setting exists because the reference's Windows
// side needs it.
Process_Argument_Quoting :: enum {
    QUOTE_IF_NEEDED :: 0;
    NEVER_QUOTE     :: 1;
}

// Runs `args[0]` with the rest as its arguments, without a shell, so nothing
// in them is expanded or split.
//
// `capture_and_return_output` reads the child's stdout and stderr into the two
// returned strings; without it the child writes wherever this process does.
// `timeout_ms` of -1 waits as long as the child takes; anything else kills it
// once the time is up and says so in `timeout_reached`.
run_command :: (args: .. string, working_directory := "", capture_and_return_output := false, print_captured_output := false, timeout_ms := -1, arg_quoting := Process_Argument_Quoting.QUOTE_IF_NEEDED) -> (process_result: Process_Result, output_string := "", error_string := "", timeout_reached := false) {
    result: Process_Result;
    if !args.count {
        result.type = .FAILED_TO_LAUNCH;
        return result, "", "", false;
    }

    // The C argument vector, null-terminated, built before the fork: after it,
    // allocating in the child is not something to rely on.
    argv := NewArray(args.count + 1, *u8, allocator = temp);
    for args  argv[it_index] = to_c_string(it, temp);
    argv[args.count] = null;

    c_directory: *u8;
    if working_directory.count  c_directory = to_c_string(working_directory, temp);

    out_fds: [2] s32;
    err_fds: [2] s32;
    if capture_and_return_output {
        if pipe(*out_fds) != 0 {
            result.type = .FAILED_TO_LAUNCH;
            return result, "", "", false;
        }
        if pipe(*err_fds) != 0 {
            close(out_fds[0]);
            close(out_fds[1]);
            result.type = .FAILED_TO_LAUNCH;
            return result, "", "", false;
        }
    }

    pid := fork();
    if pid < 0 {
        if capture_and_return_output {
            close(out_fds[0]);
            close(out_fds[1]);
            close(err_fds[0]);
            close(err_fds[1]);
        }
        result.type = .FAILED_TO_LAUNCH;
        return result, "", "", false;
    }

    if pid == 0 {
        // The child. Nothing here may return: whatever happens, it exits.
        if capture_and_return_output {
            close(out_fds[0]);
            close(err_fds[0]);
            dup2(out_fds[1], STDOUT_FILENO);
            dup2(err_fds[1], STDERR_FILENO);
            close(out_fds[1]);
            close(err_fds[1]);
        }
        if c_directory  chdir(c_directory);
        execvp(argv[0], argv.data);
        // Only reached if exec failed.
        _exit(127);
    }

    // The parent.
    output: string;
    errors: string;
    timeout_reached := false;

    if capture_and_return_output {
        close(out_fds[1]);
        close(err_fds[1]);
        output, errors, timeout_reached = drain(out_fds[0], err_fds[0], pid, timeout_ms);
        close(out_fds[0]);
        close(err_fds[0]);
    } else if timeout_ms >= 0 {
        timeout_reached = wait_with_timeout(pid, timeout_ms);
    }

    if print_captured_output {
        if output.count  write_string(output);
        if errors.count  write_string(errors, true);
    }

    status: s32;
    while waitpid(pid, *status, 0) < 0 {
        if errno() != EINTR  return result, output, errors, timeout_reached;
    }

    if WIFEXITED(status) {
        result.type = .EXITED;
        result.exit_code = WEXITSTATUS(status);
    } else {
        result.type = .SIGNALED;
        result.signal = WTERMSIG(status);
    }
    return result, output, errors, timeout_reached;
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

#scope_module

POLL_SLICE_MS :: 20;

// Reads both pipes until they close. They are read in turn rather than one
// after the other: a child that fills the pipe nobody is reading blocks
// forever, and which one that is depends on the child.
drain :: (out_fd: s32, err_fd: s32, pid: s32, timeout_ms: s64) -> (output: string, errors: string, timeout_reached: bool) {
    out_builder: String_Builder;
    err_builder: String_Builder;
    out_builder.allocator = temp;
    err_builder.allocator = temp;

    set_non_blocking(out_fd);
    set_non_blocking(err_fd);

    out_open := true;
    err_open := true;
    waited := 0;
    timeout_reached := false;

    while out_open || err_open {
        moved := false;
        if out_open  out_open, moved = pump(out_fd, *out_builder, moved);
        if err_open  err_open, moved = pump(err_fd, *err_builder, moved);

        if moved {
            waited = 0;
            continue;
        }

        // Nothing to read: either the child is thinking, or it is finished and
        // the pipes are about to close.
        if timeout_ms >= 0 && waited >= timeout_ms {
            kill(pid, SIGKILL);
            timeout_reached = true;
            break;
        }
        sleep_ms(POLL_SLICE_MS);
        waited += POLL_SLICE_MS;
    }

    return builder_to_string(*out_builder,, allocator = temp), builder_to_string(*err_builder,, allocator = temp), timeout_reached;
}

// Whether the descriptor is still open, and whether anything came out of it.
pump :: (fd: s32, builder: *String_Builder, moved_already: bool) -> (still_open: bool, moved: bool) {
    moved := moved_already;
    buffer: [4096] u8;
    while true {
        n := read(fd, buffer.data, cast(u64) buffer.count);
        if n < 0 {
            if errno() == EINTR  continue;
            // EAGAIN: nothing right now, but the child has not closed it.
            return true, moved;
        }
        if n == 0  return false, moved;

        piece: string;
        piece.data = buffer.data;
        piece.count = n;
        append(builder, piece);
        moved = true;
    }
    return true, moved;
}

// For a child whose output nobody is capturing: there is nothing to read, so
// the wait is on the process itself.
wait_with_timeout :: (pid: s32, timeout_ms: s64) -> (timeout_reached: bool) {
    waited := 0;
    while waited < timeout_ms {
        status: s32;
        done := waitpid(pid, *status, WNOHANG);
        // Reaped here, but `run_command` waits again; a second wait on a
        // reaped child fails harmlessly and the result is read from there.
        if done != 0  return false;
        sleep_ms(POLL_SLICE_MS);
        waited += POLL_SLICE_MS;
    }
    kill(pid, SIGKILL);
    return true;
}

set_non_blocking :: (fd: s32) {
    flags := fcntl(fd, F_GETFL, 0);
    if flags < 0  return;
    fcntl(fd, F_SETFL, flags | O_NONBLOCK);
}

sleep_ms :: (milliseconds: s64) {
    request: timespec;
    request.tv_sec = milliseconds / 1000;
    request.tv_nsec = (milliseconds % 1000) * 1_000_000;
    remaining: timespec;
    while nanosleep(*request, *remaining) != 0 {
        if errno() != EINTR  break;
        request = remaining;
    }
}
