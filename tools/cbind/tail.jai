
// ------------------------------------------------------- the hand-written ---
//
// Five C shapes the generator does not cover, written out here.

// Never declared on its own in the headers: it appears only behind a pointer,
// which makes it an incomplete type that the library owns the contents of.
__locale_data :: struct {}

// `typedef RET (NAME)(args)` declares a function *type* rather than a pointer
// to one. A Jai procedure type is already a reference, so the `*` a member
// writes in front of these is one level of indirection more than C means; the
// declarations are here so that the structs holding them resolve.
cookie_read_function_t  :: #type (*void, *u8, u64) -> s64 #c_call;
cookie_write_function_t :: #type (*void, *u8, u64) -> s64 #c_call;
cookie_seek_function_t  :: #type (*void, *s64, s32) -> s32 #c_call;
cookie_close_function_t :: #type (*void) -> s32 #c_call;

// glibc puts an alignment attribute after this one's name, which the
// generator's typedef pass does not read past.
Pthread_Unwind_Jmp_Buf :: struct {
    __cancel_jmp_buf: __jmp_buf;
    __mask_was_saved: s32;
}

__pthread_unwind_buf_t :: struct {
    __cancel_jmp_buf: [1] Pthread_Unwind_Jmp_Buf;
    __pad:            [4] *void;
}

// ------------------------------------------------------------ the macros ---
//
// C declares these as macros, so there is nothing in the headers for the
// generator to read. They are written out against what the macros expand to.

// The type bits of a mode, and the predicates over them.
S_ISDIR  :: inline (mode: __mode_t) -> bool { return (mode & S_IFMT) == S_IFDIR;  }
S_ISREG  :: inline (mode: __mode_t) -> bool { return (mode & S_IFMT) == S_IFREG;  }
S_ISCHR  :: inline (mode: __mode_t) -> bool { return (mode & S_IFMT) == S_IFCHR;  }
S_ISBLK  :: inline (mode: __mode_t) -> bool { return (mode & S_IFMT) == S_IFBLK;  }
S_ISFIFO :: inline (mode: __mode_t) -> bool { return (mode & S_IFMT) == S_IFIFO;  }
S_ISLNK  :: inline (mode: __mode_t) -> bool { return (mode & S_IFMT) == S_IFLNK;  }
S_ISSOCK :: inline (mode: __mode_t) -> bool { return (mode & S_IFMT) == S_IFSOCK; }

// A directory entry's type and a file mode describe the same thing in
// different bits; these convert between them.
IFTODT :: inline (mode: __mode_t) -> u32 { return (mode & 0170000) >> 12; }
DTTOIF :: inline (type: u32) -> __mode_t { return cast,no_check(__mode_t) (type << 12); }

// What `waitpid` packs into its status word, beyond the two `module.jai`
// declares.
WSTOPSIG      :: inline (status: s32) -> s32  { return WEXITSTATUS(status); }
WIFSTOPPED    :: inline (status: s32) -> bool { return (status & 0xff) == 0x7f; }
WIFCONTINUED  :: inline (status: s32) -> bool { return status == 0xffff; }
WCOREDUMP     :: inline (status: s32) -> bool { return (status & 0x80) != 0; }

// ------------------------------------------------------------ the globals ---
//
// Symbols libc defines that a program reads rather than calls. `#elsewhere`
// declares the name without giving it storage of its own (**L§4.8**).

stdin  : *FILE #elsewhere libc;
stdout : *FILE #elsewhere libc;
stderr : *FILE #elsewhere libc;

environ:   *[] *u8 #elsewhere libc;
__environ: *[] *u8 #elsewhere libc;

// The current time zone's names, and whether it observes daylight saving.
tzname:     [2] *u8 #elsewhere libc;
__tzname:   [2] *u8 #elsewhere libc;
daylight:   s32     #elsewhere libc;
__daylight: s32     #elsewhere libc;
__timezone: s64     #elsewhere libc;

// Why the last `getdate` failed.
getdate_err: s32 #elsewhere libc;

// How the program was invoked, which a diagnostic wants to quote.
program_invocation_name:       *u8 #elsewhere libc;
program_invocation_short_name: *u8 #elsewhere libc;

// ----------------------------------------------------------- the leftovers ---

// The reference's own name for the value `errno` holds, so that a signature
// says what the number means rather than that it is an `s32`.
OS_Error_Code :: #type,isa s32;

set_errno :: inline (value: OS_Error_Code) #no_context {
    (cast(*OS_Error_Code) __errno_location()).* = value;
}


// The cookie-stream callbacks under the names `<stdio.h>` gives them.
__io_read_fn  :: cookie_read_function_t;
__io_write_fn :: cookie_write_function_t;
__io_seek_fn  :: cookie_seek_function_t;
__io_close_fn :: cookie_close_function_t;
