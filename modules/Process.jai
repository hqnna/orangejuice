//
// Running other programs.
//
// `run_command` is the whole point: fork, exec, wait, and hand back what the
// child wrote. The child's output is read through a pipe while it runs rather
// than after it exits, because a child that fills the pipe buffer and blocks
// would otherwise never exit and never be waited for.
//

#import "Basic";
#import "POSIX";
#import "String";

Process_Result :: struct {
    // What the child returned, or -1 if it could not be started or was killed
    // by a signal.
    exit_code: s32 = -1;
    // Whether it ran at all — separate from whether it succeeded.
    ok := false;
}

// Runs `arguments[0]` with the rest as its arguments, without a shell, so
// nothing in them is expanded or split.
//
// `capture_output` reads the child's stdout; without it the child writes
// wherever this process does.
run_command :: (arguments: .. string, working_directory := "", capture_output := false) -> (result: Process_Result, output: string) {
    empty: string;
    result: Process_Result;
    if !arguments.count  return result, empty;

    // The C argument vector, null-terminated, built before the fork: after
    // it, allocating in the child is not something to rely on.
    argv := NewArray(*u8, arguments.count + 1, temp);
    for arguments  argv[it_index] = to_c_string(it, temp);
    argv[arguments.count] = null;

    c_directory: *u8;
    if working_directory.count  c_directory = to_c_string(working_directory, temp);

    fds: [2] s32;
    if capture_output {
        if pipe(*fds) != 0  return result, empty;
    }

    pid := fork();
    if pid < 0 {
        if capture_output {
            close(fds[0]);
            close(fds[1]);
        }
        return result, empty;
    }

    if pid == 0 {
        // The child. Nothing here may return: whatever happens, it exits.
        if capture_output {
            close(fds[0]);
            dup2(fds[1], STDOUT_FILENO);
            close(fds[1]);
        }
        if c_directory  chdir(c_directory);
        execvp(argv[0], argv.data);
        // Only reached if exec failed.
        _exit(127);
    }

    // The parent.
    output: string;
    if capture_output {
        close(fds[1]);
        output = read_all(fds[0]);
        close(fds[0]);
    }

    status: s32;
    while waitpid(pid, *status, 0) < 0 {
        if errno() != EINTR  return result, output;
    }

    result.ok = true;
    if WIFEXITED(status)  result.exit_code = WEXITSTATUS(status);
    return result, output;
}

// Runs a command line through the shell, so redirection and globbing work.
// Whatever is in `command` is interpreted by `/bin/sh`, so a caller building
// one out of untrusted input is asking for trouble.
run_shell_command :: (command: string) -> Process_Result {
    result: Process_Result;
    c_command := to_c_string(command, temp);
    if !c_command  return result;

    status := system(c_command);
    if status < 0  return result;
    result.ok = true;
    if WIFEXITED(status)  result.exit_code = WEXITSTATUS(status);
    return result;
}

#scope_module

// Everything the descriptor gives until it closes, in temporary storage.
read_all :: (fd: s32) -> string {
    builder: String_Builder;
    init_string_builder(*builder, temp);

    buffer: [4096] u8;
    while true {
        n := read(fd, buffer.data, cast(u64) buffer.count);
        if n < 0 {
            if errno() == EINTR  continue;
            break;
        }
        if n == 0  break;
        piece: string;
        piece.data = buffer.data;
        piece.count = n;
        append(*builder, piece);
    }
    return builder_to_string(*builder, temp);
}
