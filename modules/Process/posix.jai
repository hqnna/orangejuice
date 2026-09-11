//
// The POSIX half: fork, exec, and three pipes.
//

Linux :: #import "Linux";

Process :: struct {
    input:  Pipe;
    output: Pipe;
    error:  Pipe;

    pid: pid_t;

    Pipe :: struct {
        handle: s32 = -1;
        eof: bool;
    }
}

// Starts `args[0]` and returns as soon as it is running. With
// `capture_and_return_output` the child's three standard descriptors become
// pipes this process holds the other end of; without it the child inherits
// this process's.
//
// `kill_process_if_parent_exits` asks the kernel to send the child a SIGKILL
// if this process dies, so a crash here does not leave it running.
create_process :: (process: *Process, args: .. string, working_directory := "", capture_and_return_output := false, arg_quoting := Process_Argument_Quoting.QUOTE_IF_NEEDED, kill_process_if_parent_exits := true) -> (success: bool) {
    if !args.count  return false;

    if !kill_process_if_parent_exits {
        log_error("Not killing the process is not yet implemented for Unix.");
        return false;
    }

    // Built before the fork: between `fork` and `execvp` the child may only
    // call what is async-signal-safe, and allocating is not.
    argv := NewArray(args.count + 1, *u8,, temp);
    for args  argv[it_index] = to_c_string(it, temp);
    argv[args.count] = null;

    c_directory: *u8;
    if working_directory.count  c_directory = to_c_string(working_directory, temp);

    input_fds:  [2] s32;
    output_fds: [2] s32;
    error_fds:  [2] s32;

    if capture_and_return_output {
        if pipe(*input_fds) != 0  return false;
        if pipe(*output_fds) != 0 {
            close_pair(*input_fds);
            return false;
        }
        if pipe(*error_fds) != 0 {
            close_pair(*input_fds);
            close_pair(*output_fds);
            return false;
        }
    }

    pid := fork();
    if pid < 0 {
        if capture_and_return_output {
            close_pair(*input_fds);
            close_pair(*output_fds);
            close_pair(*error_fds);
        }
        return false;
    }

    if pid == 0 {
        // The child. Nothing here may return: whatever happens, it exits.
        // The child asks the kernel to kill it if this process dies, so a
        // crash here does not leave it running.
        Linux.prctl(Linux.PR_SET_PDEATHSIG, cast(u64) SIGKILL, 0, 0, 0);

        if capture_and_return_output {
            close(input_fds[1]);
            close(output_fds[0]);
            close(error_fds[0]);
            dup2(input_fds[0], STDIN_FILENO);
            dup2(output_fds[1], STDOUT_FILENO);
            dup2(error_fds[1], STDERR_FILENO);
            close(input_fds[0]);
            close(output_fds[1]);
            close(error_fds[1]);
        }
        if c_directory  chdir(c_directory);
        execvp(argv[0], argv.data);
        // Only reached when exec failed; 127 is what a shell reports for a
        // command it could not run.
        _exit(127);
    }

    process.pid = pid;
    if capture_and_return_output {
        close(input_fds[0]);
        close(output_fds[1]);
        close(error_fds[1]);
        process.input.handle  = input_fds[1];
        process.output.handle = output_fds[0];
        process.error.handle  = error_fds[0];
    } else {
        // Nothing to read, so both pipes are at their end already.
        process.output.eof = true;
        process.error.eof  = true;
    }

    return true;
}

// Closes whatever of the child's pipes this process still holds. It does not
// wait for the child; `get_process_result` is what reaps it.
deinit :: (process: *Process) {
    close_and_reset(*process.input.handle);
    close_and_reset(*process.output.handle);
    close_and_reset(*process.error.handle);
}

write_to_process :: (process: *Process, data: [] u8) -> (success: bool, bytes_written: int) {
    if process.input.handle == -1  return false, 0;

    written := write(process.input.handle, data.data, cast(u64) data.count);
    if written < 0  return false, 0;
    return true, written;
}

// Waits for either pipe to have something and reads what is there. A pipe that
// has reached its end is marked and not waited on again, so the call returns
// as soon as both are done rather than blocking on one that is finished.
read_from_process :: (process: *Process, output_buffer: [] u8, error_buffer: [] u8, timeout_ms := -1) -> (success: bool, output_bytes: int, error_bytes: int) {
    output_bytes := 0;
    error_bytes  := 0;

    if process.output.eof && process.error.eof  return true, 0, 0;

    fds: [2] pollfd;
    count: nfds_t = 0;
    output_slot := -1;
    error_slot  := -1;

    if !process.output.eof && process.output.handle != -1 {
        fds[count].fd = process.output.handle;
        fds[count].events = POLLIN;
        output_slot = xx count;
        count += 1;
    }
    if !process.error.eof && process.error.handle != -1 {
        fds[count].fd = process.error.handle;
        fds[count].events = POLLIN;
        error_slot = xx count;
        count += 1;
    }

    if !count  return true, 0, 0;

    ready := poll(fds.data, count, xx timeout_ms);
    while ready < 0 && errno() == EINTR  ready = poll(fds.data, count, xx timeout_ms);
    if ready < 0  return false, 0, 0;
    // A timeout is not a failure; the caller decides what to do about it.
    if ready == 0  return true, 0, 0;

    if output_slot >= 0 && fds[output_slot].revents {
        success, moved := read_pipe(*process.output, output_buffer);
        if !success  return false, output_bytes, error_bytes;
        output_bytes = moved;
    }
    if error_slot >= 0 && fds[error_slot].revents {
        success, moved := read_pipe(*process.error, error_buffer);
        if !success  return false, output_bytes, error_bytes;
        error_bytes = moved;
    }

    return true, output_bytes, error_bytes;
}

// One read. Zero bytes back means the write end is closed, which is what sets
// `eof`.
read_pipe :: (pipe: *Process.Pipe, buffer: [] u8) -> (success: bool, bytes_read: int) {
    if pipe.handle == -1 {
        pipe.eof = true;
        return true, 0;
    }

    while true {
        moved := read(pipe.handle, buffer.data, cast(u64) buffer.count);
        if moved < 0 {
            if errno() == EINTR  continue;
            return false, 0;
        }
        if moved == 0  pipe.eof = true;
        return true, moved;
    }
    return false, 0;
}

kill_process :: (process: *Process) -> bool {
    if !process.pid  return false;
    return kill(process.pid, SIGKILL) == 0;
}

// Reaps the child if it has finished. `timeout_ms` of -1 waits for it; zero
// only looks, and anything else waits that long before answering
// `STILL_RUNNING`. `success` is false only when the wait itself failed.
get_process_result :: (process: *Process, timeout_ms := -1) -> (success: bool, result: Process_Result) {
    result: Process_Result;
    if !process.pid {
        result.type = .UNSTARTED;
        return true, result;
    }

    waited := 0;
    while true {
        status: s32;
        flags: s32 = ifx timeout_ms == -1 then 0 else WNOHANG;
        reaped := waitpid(process.pid, *status, flags);

        if reaped < 0 {
            if errno() == EINTR  continue;
            return false, result;
        }

        if reaped == 0 {
            // Still running, and this was a peek rather than a wait.
            if timeout_ms == 0 || waited >= timeout_ms {
                result.type = .STILL_RUNNING;
                return true, result;
            }
            sleep_milliseconds(POLL_SLICE_MS);
            waited += POLL_SLICE_MS;
            continue;
        }

        process.pid = 0;
        if WIFEXITED(status) {
            result.type = .EXITED;
            result.exit_code = WEXITSTATUS(status);
        } else {
            result.type = .SIGNALED;
            result.signal = WTERMSIG(status);
        }
        return true, result;
    }
    return false, result;
}

#scope_file

POLL_SLICE_MS :: 5;

close_pair :: (fds: *[2] s32) {
    close(fds.*[0]);
    close(fds.*[1]);
}

close_and_reset :: (fd: *s32) {
    if fd.* == -1  return;
    close(fd.*);
    fd.* = -1;
}
