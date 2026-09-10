//
// What a program says on its way down.
//
// Without this a crash is a silent core dump: the shell reports a signal and
// the program says nothing about where it was. With it, the fault is named,
// the faulty address is printed, and the call stack is walked — which is
// usually enough to find the bug without a debugger.
//
// The entry point installs it before calling `main` unless
// `Build_Options.backtrace_on_crash` is off, so nothing in a program has to
// mention it.
//
// The handler runs on a signal stack of its own, because the commonest crash
// of all — an overflowed stack — leaves no room on the ordinary one to run a
// handler in. Everything it does is what a signal handler is allowed to do:
// it writes with `write` and formats into a fixed buffer, taking no locks and
// allocating nothing.
//

#scope_module

Linux :: #import "Linux";

#scope_export

// Puts the handlers in place. Calling it twice is harmless.
init :: () #no_context {
    if installed  return;
    installed = true;

    // A handler for a stack overflow has to run somewhere other than the stack
    // that overflowed.
    stack: stack_t;
    stack.ss_sp = alternate_stack.data;
    stack.ss_size = alternate_stack.count;
    stack.ss_flags = 0;
    sigaltstack(*stack, null);

    install(SIGSEGV);
    install(SIGBUS);
    install(SIGILL);
    install(SIGFPE);
}

#scope_module

installed := false;

SIGNAL_STACK_SIZE :: 65536;
alternate_stack: [SIGNAL_STACK_SIZE] u8;

// The handler reports and then dies of the same signal, so that a parent shell
// or a debugger still sees the real cause rather than a clean exit.
handle :: (sig: s32, info: *siginfo_t, secret: *void) #c_call {
    describe(sig, info, caller_address(secret));
    write_backtrace();

    // Back to the default and re-raise: the process then dies of the signal it
    // actually got, which is what a waiting parent expects to see.
    action: sigaction_t;
    action.sa_handler = SIG_DFL;
    sigaction(sig, *action, null);
    raise(sig);
}

describe :: (sig: s32, info: *siginfo_t, from: *void) #no_context {
    if sig == SIGSEGV {
        if info.si_code == SEGV_MAPERR {
            if info.si_addr == null {
                werr("Program Exception: SEGV_MAPERR\nNull Pointer Exception. Attempt to dereference a null pointer.\n");
            } else {
                werr("Program Exception: SEGV_MAPERR\nThe program crashed because it tried to write to or read from an invalid memory location.\n");
            }
        } else if info.si_code == SEGV_ACCERR {
            werr("Program Exception: SEGV_ACCERR\nThe program crashed because it tried to write to read only memory.\n");
        } else if info.si_code == SEGV_BNDERR {
            werr("Program Exception: SEGV_BNDERR\nThe program crashed because there was a failed address bound check.\n");
        } else if info.si_code == SEGV_PKUERR {
            werr("Program Exception: SEGV_PKUERR\nThe program crashed because there was a failed protection key check.\n");
        }
        werr("Segmentation fault. The faulty address is 0x");
        werr_hex(cast,no_check(u64) info.si_addr);
        // Which instruction was running, which is the more useful of the two:
        // the address that faulted says what was touched, this says who
        // touched it.
        if from {
            werr(", from 0x");
            werr_hex(cast,no_check(u64) from);
        }
        werr(".\n");
        return;
    }

    if sig == SIGBUS {
        werr("Program Exception: SIGBUS\nThe program crashed because it accessed memory in a way the machine cannot: a misaligned address, or a mapping that is no longer backed.\nThe faulty address is 0x");
        werr_hex(cast,no_check(u64) info.si_addr);
        werr(".\n");
        return;
    }

    if sig == SIGILL {
        werr("Program Exception: SIGILL\nThe program crashed because it tried to execute something that is not an instruction.\nThe faulty address is 0x");
        werr_hex(cast,no_check(u64) info.si_addr);
        werr(".\n");
        return;
    }

    if sig == SIGFPE {
        // Integer division by zero is the one that actually happens; the name
        // is a leftover from when this signal was only about floats.
        werr("Program Exception: SIGFPE\nThe program crashed doing arithmetic that has no answer — dividing by zero, most likely.\n");
        return;
    }

    werr("Program Exception: the program was killed by signal ");
    werr_hex(cast(u64) sig);
    werr(".\n");
}

// The instruction pointer the kernel saved when it delivered the signal. The
// third handler argument is a `ucontext_t`; on x86-64 its saved general
// registers begin at offset 40 and `rip` is the seventeenth of them.
caller_address :: (secret: *void) -> *void #no_context {
    if !secret  return null;
    UC_MCONTEXT_GREGS :: 40;
    REG_RIP           :: 16;
    gregs := cast(*u64) (cast(*u8) secret + UC_MCONTEXT_GREGS);
    return cast,no_check(*void) gregs[REG_RIP];
}

// The return addresses on the stack, as the module names and offsets libc can
// resolve them to. `backtrace` is async-signal-safe once it has been called
// once; `backtrace_symbols_fd` writes without allocating, which is why it is
// the one used here rather than `backtrace_symbols`.
write_backtrace :: () #no_context {
    frames: [64] *void;
    count := backtrace(frames.data, cast(s32) frames.count);
    if count <= 0 {
        werr("  (no stack trace available)\n");
        return;
    }
    // This handler and the kernel's trampoline sit on top of the frame that
    // actually crashed, and are noise to whoever is reading. A procedure whose
    // symbol is internal to the executable prints as an offset into it rather
    // than by name; `addr2line` against the DWARF resolves those.
    HANDLER_FRAMES :: 3;
    skip: s32 = ifx count > HANDLER_FRAMES then HANDLER_FRAMES else 0;
    backtrace_symbols_fd(frames.data + skip, count - skip, STDERR);
}

// ------------------------------------------------------------ the writing ---

STDERR :: 2;

werr :: (s: string) #no_context {
    if !s.count  return;
    written := 0;
    while written < s.count {
        n := Linux.write(STDERR, s.data + written, cast(u64) (s.count - written));
        if n <= 0  break;
        written += n;
    }
}

werr_hex :: (value: u64) #no_context {
    DIGITS :: "0123456789abcdef";
    buffer: [16] u8;
    at := buffer.count;
    remaining := value;
    if remaining == 0 {
        at -= 1;
        buffer[at] = #char "0";
    }
    while remaining > 0 {
        at -= 1;
        buffer[at] = DIGITS[remaining & 0xf];
        remaining >>= 4;
    }
    text: string;
    text.data = buffer.data + at;
    text.count = buffer.count - at;
    werr(text);
}

// ----------------------------------------------------------- the bindings ---

install :: (sig: s32) #no_context {
    action: sigaction_t;
    action.sa_handler = cast(*void) handle;
    // SIGINFO gives the three-argument handler, ONSTACK puts it on the
    // alternate stack, RESTART is what everything else in the program expects
    // of a signal that did not kill it.
    action.sa_flags = SA_SIGINFO | SA_ONSTACK | SA_RESTART;
    sigemptyset(*action.sa_mask);
    sigaction(sig, *action, null);
}

SIGILL  :: 4;
SIGFPE  :: 8;
SIGBUS  :: 7;
SIGSEGV :: 11;

SEGV_MAPERR :: 1;
SEGV_ACCERR :: 2;
SEGV_BNDERR :: 3;
SEGV_PKUERR :: 4;

SA_SIGINFO :: 0x4;
SA_ONSTACK  :: 0x08000000;
SA_RESTART  :: 0x10000000;

SIG_DFL :: null;

// glibc's `siginfo_t`. Only the first three words and `si_addr` are read; the
// rest is the union the kernel fills per signal, kept as padding so the struct
// is the size the kernel writes.
siginfo_t :: struct {
    si_signo: s32;
    si_errno: s32;
    si_code:  s32;
    _pad0:    s32;
    si_addr:  *void;
    _rest:    [112] u8;
}

// glibc's `struct sigaction`. `sa_handler` and `sa_sigaction` are a union, so
// one pointer stands for both.
sigaction_t :: struct {
    sa_handler:  *void;
    sa_mask:     sigset_t;
    sa_flags:    s32;
    _pad:        s32;
    sa_restorer: *void;
}

sigset_t :: struct {
    bits: [16] u64;
}

stack_t :: struct {
    ss_sp:    *void;
    ss_flags: s32;
    _pad:     s32;
    ss_size:  s64;
}

sigaction   :: (sig: s32, action: *sigaction_t, old: *sigaction_t) -> s32 #foreign libc_crash;
sigemptyset :: (set: *sigset_t) -> s32 #foreign libc_crash;
sigaltstack :: (stack: *stack_t, old: *stack_t) -> s32 #foreign libc_crash;
raise       :: (sig: s32) -> s32 #foreign libc_crash;

backtrace            :: (buffer: **void, size: s32) -> s32 #foreign libc_crash;
backtrace_symbols_fd :: (buffer: **void, size: s32, fd: s32) #foreign libc_crash;

libc_crash :: #system_library "c";
