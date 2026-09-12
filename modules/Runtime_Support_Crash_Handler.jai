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

#if OS == .MACOS {
    Libc :: #import "MacOS";
} else {
    Libc :: #import "Linux";
}

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

#scope_export

// The handler reports and then dies of the same signal, so that a parent shell
// or a debugger still sees the real cause rather than a clean exit.
handle_signal :: (sig: s32, info: *siginfo_t, secret: *void) #c_call {
    describe(sig, info, caller_address(secret));
    write_backtrace(secret);

    // Back to the default and re-raise: the process then dies of the signal it
    // actually got, which is what a waiting parent expects to see.
    action: sigaction_t;
    action.sa_handler = SIG_DFL;
    sigaction(sig, *action, null);
    raise(sig);
}

#scope_module

describe :: (sig: s32, info: *siginfo_t, from: *void) #no_context {
    if sig == SIGSEGV {
        // An access at address zero is a null dereference whichever code the
        // kernel attached to it: Linux calls the page unmapped and Darwin
        // calls it unreadable, and the program made the same mistake either
        // way. The name of the code is still the kernel's own.
        if info.si_addr == null {
            werr("Program Exception: ");
            werr(segv_code_name(info.si_code));
            werr("\nNull Pointer Exception. Attempt to dereference a null pointer.\n");
        } else if info.si_code == SEGV_MAPERR {
            werr("Program Exception: SEGV_MAPERR\nThe program crashed because it tried to write to or read from an invalid memory location.\n");
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

segv_code_name :: (code: s32) -> string #no_context {
    if code == SEGV_MAPERR  return "SEGV_MAPERR";
    if code == SEGV_ACCERR  return "SEGV_ACCERR";
    if code == SEGV_BNDERR  return "SEGV_BNDERR";
    if code == SEGV_PKUERR  return "SEGV_PKUERR";
    return "SIGSEGV";
}

// The instruction pointer the kernel saved when it delivered the signal. The
// third handler argument is a `ucontext_t`, and where the saved program
// counter sits inside one is the system's own business.
caller_address :: (secret: *void) -> *void #no_context {
    if !secret  return null;
    #if OS == .MACOS {
        machine := (cast(**u8) (cast(*u8) secret + UC_MCONTEXT)).*;
        if !machine  return null;
        return cast,no_check(*void) (cast(*u64) (machine + MCONTEXT_PC)).*;
    } else {
        // On x86-64 Linux the saved general registers begin at offset 40 and
        // `rip` is the seventeenth of them.
        UC_MCONTEXT_GREGS :: 40;
        REG_RIP           :: 16;
        gregs := cast(*u64) (cast(*u8) secret + UC_MCONTEXT_GREGS);
        return cast,no_check(*void) gregs[REG_RIP];
    }
}

// The return addresses on the stack, as the module names and offsets libc can
// resolve them to. `backtrace` is async-signal-safe once it has been called
// once; `backtrace_symbols_fd` writes without allocating, which is why it is
// the one used here rather than `backtrace_symbols`.
write_backtrace :: (secret: *void) #no_context {
    frames: [64] *void;
    #if OS == .MACOS {
        // Darwin's unwinder does not walk past the signal trampoline, and
        // `backtrace_from_fp` will not start from a frame outside the stack it
        // is running on — which the crashing stack is, since the handler runs
        // on one of its own. So the chain is walked here: on arm64 a frame
        // record is the caller's frame pointer and the return address, in that
        // order, at the address the frame pointer holds.
        count := walk_frames(caller_address(secret), frame_pointer(secret), frames);
    } else {
        count := backtrace(frames.data, cast(s32) frames.count);
    }
    if count <= 0 {
        werr("  (no stack trace available)\n");
        return;
    }
    // This handler and the kernel's trampoline sit on top of the frame that
    // actually crashed, and are noise to whoever is reading. A procedure whose
    // symbol is internal to the executable prints as an offset into it rather
    // than by name; `addr2line` against the DWARF resolves those.
    //
    // On Darwin the walk started at the crashing frame, so there is nothing of
    // this module's on top of it to drop.
    #if OS == .MACOS {
        HANDLER_FRAMES :: 0;
    } else {
        HANDLER_FRAMES :: 3;
    }
    skip: s32 = ifx count > HANDLER_FRAMES then HANDLER_FRAMES else 0;
    backtrace_symbols_fd(frames.data + skip, count - skip, STDERR);
}

// ------------------------------------------------------------ the writing ---

STDERR :: 2;

werr :: (s: string) #no_context {
    if !s.count  return;
    written := 0;
    while written < s.count {
        n := Libc.write(STDERR, s.data + written, cast(u64) (s.count - written));
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
    action.sa_handler = cast(*void) handle_signal;
    // SIGINFO gives the three-argument handler, ONSTACK puts it on the
    // alternate stack, RESTART is what everything else in the program expects
    // of a signal that did not kill it.
    action.sa_flags = SA_SIGINFO | SA_ONSTACK | SA_RESTART;
    sigemptyset(*action.sa_mask);
    sigaction(sig, *action, null);
}

#scope_export

SIGQUIT ::  3;
SIGILL  ::  4;
SIGTRAP ::  5;
SIGABRT ::  6;
SIGFPE  ::  8;
SIGSEGV :: 11;

SEGV_MAPERR :: 1;
SEGV_ACCERR :: 2;
SEGV_BNDERR :: 3;
SEGV_PKUERR :: 4;

SIG_DFL :: null;

#if OS == .MACOS {
    // BSD numbered `SIGBUS` 10 where Linux numbers it 7.
    SIGBUS :: 10;

    // Where the machine state the kernel saved is, and where the two registers
    // read out of it sit inside it. Darwin's `ucontext_t` holds a *pointer* to
    // an `mcontext64` rather than the thing itself; inside one, the exception
    // state is sixteen bytes and an arm64 thread state follows it, with `fp`
    // just past `x[29]` and `pc` after `fp`, `lr` and `sp`.
    //
    // @Volatile: these are the SDK's, and
    // `a_darwin_layout_is_what_the_c_compiler_says` measures them against it.
    UC_MCONTEXT :: 48;
    MCONTEXT_FP :: 16 + 29 * 8;
    MCONTEXT_PC :: 16 + 29 * 8 + 24;

    SA_ONSTACK :: 0x0001;
    SA_RESTART :: 0x0002;
    SA_SIGINFO :: 0x0040;

    // The three together, which is what the handler is installed with.
    SA_RESTART_ONSTACK_SIGINFO :: 0x0043;

    // BSD's `siginfo_t`. It carries the process and user that sent the signal
    // between `si_code` and `si_addr`, which glibc's does not, so the address
    // is eight bytes further along.
    siginfo_t :: struct {
        si_signo:  s32;
        si_errno:  s32;
        si_code:   s32;
        si_pid:    s32;
        si_uid:    u32;
        si_status: s32;
        si_addr:   *void;
        si_value:  sigval_t;
        si_band:   s64;
        _pad:      [7] u64;
    }

    // Darwin's `struct sigaction`. `sa_handler` and `sa_sigaction` are a
    // union, so one pointer stands for both; the mask is a single word rather
    // than the bit array glibc keeps.
    sigaction_t :: struct {
        sa_handler: *void;
        sa_mask:    sigset_t;
        sa_flags:   s32;
    }

    sigset_t :: struct {
        bits: u32;
    }

    // BSD puts the size before the flags, where Linux puts it after.
    stack_t :: struct {
        ss_sp:    *void;
        ss_size:  s64;
        ss_flags: s32;
        _pad:     s32;
    }
} else {
    SIGBUS :: 7;

    SA_SIGINFO :: 0x4;
    SA_ONSTACK  :: 0x08000000;
    SA_RESTART  :: 0x10000000;

    // The three together, which is what the handler is installed with.
    SA_RESTART_ONSTACK_SIGINFO :: 0x18000004;

    // glibc's `siginfo_t`. Only the first three words and `si_addr` are read;
    // the rest is the union the kernel fills per signal, kept as padding so
    // the struct is the size the kernel writes.
    siginfo_t :: struct {
        si_signo: s32;
        si_errno: s32;
        si_code:  s32;
        _pad0:    s32;
        si_addr:  *void;
        _rest:    [112] u8;
    }

    // glibc's `struct sigaction`. `sa_handler` and `sa_sigaction` are a union,
    // so one pointer stands for both.
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
}

sigaction   :: (sig: s32, action: *sigaction_t, old: *sigaction_t) -> s32 #foreign libc;
sigemptyset :: (set: *sigset_t) -> s32 #foreign libc;
sigaltstack :: (stack: *stack_t, old: *stack_t) -> s32 #foreign libc;
raise       :: (sig: s32) -> s32 #foreign libc;

backtrace            :: (buffer: **void, size: s32) -> s32 #foreign libc;
backtrace_symbols_fd :: (buffer: **void, size: s32, fd: s32) #foreign libc;

#if OS == .MACOS {
    // The frame pointer the kernel saved, which is the frame that crashed.
    frame_pointer :: (secret: *void) -> *void #no_context {
        if !secret  return null;
        machine := (cast(**u8) (cast(*u8) secret + UC_MCONTEXT)).*;
        if !machine  return null;
        return cast,no_check(*void) (cast(*u64) (machine + MCONTEXT_FP)).*;
    }

    // The return addresses reachable from one frame pointer, with the
    // instruction that faulted first. Nothing here is trusted: a chain that
    // does not ascend, or a frame pointer that is not aligned the way the
    // procedure call standard requires one to be, is where the walk stops —
    // a crash handler that crashes says nothing at all.
    walk_frames :: (pc: *void, start: *void, frames: [] *void) -> s32 #no_context {
        count: s32 = 0;
        if pc {
            frames[count] = pc;
            count += 1;
        }

        current := cast,no_check(u64) start;
        while count < cast(s32) frames.count {
            if !current || (current & 0xf) != 0  break;
            record := cast(*u64) current;
            next   := record[0];
            ret    := record[1];
            if !ret  break;
            frames[count] = cast,no_check(*void) ret;
            count += 1;
            // Stacks grow down, so a caller's frame is always further up than
            // the one that called it.
            if next <= current  break;
            current = next;
        }
        return count;
    }
}

libc :: #library,system "libc";

// glibc folded the backtrace routines into libc, and Darwin has always had
// them in libSystem; the name is kept because a program written against the
// reference may say it.
libexecinfo :: libc;

size_t  :: u64;
clock_t :: s64;

// What `sigqueue` carries. Not read here; it is part of the shape.
sigval_t :: union {
    sival_int: s32;
    sival_ptr: *void;
}

// Enough of the machine context to find the instruction that faulted. Not the
// real size — nothing here makes one, and `caller_address` reads the saved
// program counter out of the pointer the kernel handed over rather than out of
// this.
ucontext_t :: struct {
    uc_flags: u64;
    uc_link: *ucontext_t;
    ss_sp:    *void;
    ss_flags: s32;
    ss_size:  size_t;
    gregs: [23] s64;
}
