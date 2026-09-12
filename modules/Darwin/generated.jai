//
// The Darwin surface: the types, constants, procedures and globals that
// `Darwin/module.jai` does not declare by hand.
//
// What is here is what only Darwin has — kqueue, `sysctl`, the Mach clock,
// dyld — the way `Linux/generated.jai` holds what only Linux has. The C
// library surface both systems share is `POSIX`.
//
// Generated from the system's own headers rather than typed out, the way the
// reference generates its bindings — a constant's value is whatever
// `<sys/event.h>` says it is on the machine the compiler was built on, and a
// procedure's parameter names are the ones the SDK gave them. What is here is
// therefore a projection of the C ABI, not a transcription of anybody's file.
//
// The generator lives in `tools/cbind`; `tools/cbind/README.md` says how to
// run it again.
//

#scope_export

// -------------------------------------------------------------- constants ---

CTL_DEBUG :: 5;
CTL_DEBUG_MAXID :: 20;
CTL_DEBUG_NAME :: 0;
CTL_DEBUG_VALUE :: 1;
CTL_HW :: 6;
CTL_KERN :: 1;
CTL_MACHDEP :: 7;
CTL_MAXID :: 9;
CTL_MAXNAME :: 12;
CTL_NET :: 4;
CTL_UNSPEC :: 0;
CTL_USER :: 8;
CTL_VFS :: 3;
CTL_VM :: 2;
EVFILT_AIO :: -3;
EVFILT_EXCEPT :: -15;
EVFILT_FS :: -9;
EVFILT_MACHPORT :: -8;
EVFILT_PROC :: -5;
EVFILT_READ :: -1;
EVFILT_SIGNAL :: -6;
EVFILT_SYSCOUNT :: 18;
EVFILT_THREADMARKER :: 18;
EVFILT_TIMER :: -7;
EVFILT_USER :: -10;
EVFILT_VM :: -12;
EVFILT_VNODE :: -4;
EVFILT_WRITE :: -2;
HW_AVAILCPU :: 25;
HW_BUS_FREQ :: 14;
HW_BYTEORDER :: 4;
HW_CACHELINE :: 16;
HW_CPU_FREQ :: 15;
HW_DISKNAMES :: 8;
HW_DISKSTATS :: 9;
HW_EPOCH :: 10;
HW_FLOATINGPT :: 11;
HW_L1DCACHESIZE :: 18;
HW_L1ICACHESIZE :: 17;
HW_L2CACHESIZE :: 20;
HW_L2SETTINGS :: 19;
HW_L3CACHESIZE :: 22;
HW_L3SETTINGS :: 21;
HW_MACHINE :: 1;
HW_MACHINE_ARCH :: 12;
HW_MAXID :: 28;
HW_MEMSIZE :: 24;
HW_MODEL :: 2;
HW_NCPU :: 3;
HW_PAGESIZE :: 7;
HW_PHYSMEM :: 5;
HW_PRODUCT :: 27;
HW_TARGET :: 26;
HW_TB_FREQ :: 23;
HW_USERMEM :: 6;
HW_VECTORUNIT :: 13;
MAXPATHLEN :: 1024;
PROC_PIDPATHINFO_MAXSIZE :: 4096;

// -------------------------------------------------------------- the types ---

__uint32_t :: u32;
__darwin_intptr_t :: s64;
__darwin_size_t :: u64;
int16_t :: s16;
int32_t :: s32;
int64_t :: s64;
intptr_t :: __darwin_intptr_t;
uintptr_t :: u64;
uint16_t :: u16;
uint32_t :: u32;
uint64_t :: u64;
__darwin_uid_t :: __uint32_t;
u_int :: u32;
uid_t :: __darwin_uid_t;
size_t :: __darwin_size_t;
kevent_t :: struct {
    ident: uintptr_t;
    filter: int16_t;
    flags: uint16_t;
    fflags: uint32_t;
    data: intptr_t;
    udata: *void;
}

kevent64_s :: struct {
    ident: uint64_t;
    filter: int16_t;
    flags: uint16_t;
    fflags: uint32_t;
    data: int64_t;
    udata: uint64_t;
    ext: [2] uint64_t;
}

fsid_t :: struct {
    val: [2] int32_t;
}

statfs_t :: struct {
    f_bsize: uint32_t;
    f_iosize: int32_t;
    f_blocks: uint64_t;
    f_bfree: uint64_t;
    f_bavail: uint64_t;
    f_files: uint64_t;
    f_ffree: uint64_t;
    f_fsid: fsid_t;
    f_owner: uid_t;
    f_type: uint32_t;
    f_flags: uint32_t;
    f_fssubtype: uint32_t;
    f_fstypename: [16] u8;
    f_mntonname: [1024] u8;
    f_mntfromname: [1024] u8;
    f_flags_ext: uint32_t;
    f_reserved: [7] uint32_t;
}

kern_return_t :: s32;
mach_timebase_info_struct :: struct {
    numer: uint32_t;
    denom: uint32_t;
}

mach_timebase_info_t :: *mach_timebase_info_struct;
mach_timebase_info_data_t :: mach_timebase_info_struct;

// ------------------------------------------------------------------ enums ---

using EV_Flags :: enum_flags u16 {
    EV_ADD :: 1;
    EV_CLEAR :: 32;
    EV_DELETE :: 2;
    EV_DISABLE :: 8;
    EV_DISPATCH :: 128;
    EV_DISPATCH2 :: 384;
    EV_ENABLE :: 4;
    EV_EOF :: 32768;
    EV_ERROR :: 16384;
    EV_FLAG0 :: 4096;
    EV_FLAG1 :: 8192;
    EV_ONESHOT :: 16;
    EV_OOBAND :: 8192;
    EV_POLL :: 4096;
    EV_RECEIPT :: 64;
    EV_SYSFLAGS :: 61440;
    EV_UDATA_SPECIFIC :: 256;
    EV_VANISHED :: 512;
}

using NOTE_Flags :: enum_flags u32 {
    NOTE_ABSOLUTE :: 8;
    NOTE_ATTRIB :: 8;
    NOTE_BACKGROUND :: 64;
    NOTE_CHILD :: 4;
    NOTE_CRITICAL :: 32;
    NOTE_DELETE :: 1;
    NOTE_EXEC :: 536870912;
    NOTE_EXIT :: 2147483648;
    NOTE_EXITSTATUS :: 67108864;
    NOTE_EXIT_CSERROR :: 262144;
    NOTE_EXIT_DECRYPTFAIL :: 65536;
    NOTE_EXIT_DETAIL :: 33554432;
    NOTE_EXIT_DETAIL_MASK :: 458752;
    NOTE_EXIT_MEMORY :: 131072;
    NOTE_EXIT_REPARENTED :: 524288;
    NOTE_EXTEND :: 4;
    NOTE_FFAND :: 1073741824;
    NOTE_FFCOPY :: 3221225472;
    NOTE_FFCTRLMASK :: 3221225472;
    NOTE_FFLAGSMASK :: 16777215;
    NOTE_FFNOP :: 0;
    NOTE_FFOR :: 2147483648;
    NOTE_FORK :: 1073741824;
    NOTE_FUNLOCK :: 256;
    NOTE_LEASE_DOWNGRADE :: 512;
    NOTE_LEASE_RELEASE :: 1024;
    NOTE_LEEWAY :: 16;
    NOTE_LINK :: 16;
    NOTE_LOWAT :: 1;
    NOTE_MACHTIME :: 256;
    NOTE_MACH_CONTINUOUS_TIME :: 128;
    NOTE_NONE :: 128;
    NOTE_NSECONDS :: 4;
    NOTE_OOB :: 2;
    NOTE_PCTRLMASK :: -1048576;
    NOTE_PDATAMASK :: 1048575;
    NOTE_REAP :: 268435456;
    NOTE_RENAME :: 32;
    NOTE_REVOKE :: 64;
    NOTE_SECONDS :: 1;
    NOTE_SIGNAL :: 134217728;
    NOTE_TRACK :: 1;
    NOTE_TRACKERR :: 2;
    NOTE_TRIGGER :: 16777216;
    NOTE_USECONDS :: 2;
    NOTE_VM_ERROR :: 268435456;
    NOTE_VM_PRESSURE :: 2147483648;
    NOTE_VM_PRESSURE_SUDDEN_TERMINATE :: 536870912;
    NOTE_VM_PRESSURE_TERMINATE :: 1073741824;
    NOTE_WRITE :: 2;
}


// ------------------------------------------------------------- procedures ---

_NSGetExecutablePath :: (buf: *u8, bufsize: *uint32_t) -> s32 #foreign libc;
_dyld_get_image_name :: (image_index: uint32_t) -> *u8 #foreign libc;
_dyld_image_count :: () -> uint32_t #foreign libc;
fstatfs :: (__arg0: s32, __arg1: *statfs_t) -> s32 #foreign libc;
getmntinfo :: (__arg0: **statfs_t, __arg1: s32) -> s32 #foreign libc;
kevent :: (kq: s32, changelist: *kevent_t, nchanges: s32, eventlist: *kevent_t, nevents: s32, timeout: *timespec) -> s32 #foreign libc;
kevent64 :: (kq: s32, changelist: *kevent64_s, nchanges: s32, eventlist: *kevent64_s, nevents: s32, flags: u32, timeout: *timespec) -> s32 #foreign libc;
kqueue :: () -> s32 #foreign libc;
mach_absolute_time :: () -> uint64_t #foreign libc;
mach_continuous_time :: () -> uint64_t #foreign libc;
mach_timebase_info :: (info: mach_timebase_info_t) -> kern_return_t #foreign libc;
proc_name :: (pid: s32, buffer: *void, buffersize: uint32_t) -> s32 #foreign libc;
proc_pidpath :: (pid: s32, buffer: *void, buffersize: uint32_t) -> s32 #foreign libc;
statfs :: (__arg0: *u8, __arg1: *statfs_t) -> s32 #foreign libc;
sysctl :: (__arg0: *s32, __arg1: u_int, __arg2: *void, __arg3: *size_t, __arg4: *void, __arg5: size_t) -> s32 #foreign libc;
sysctlbyname :: (__arg0: *u8, __arg1: *void, __arg2: *size_t, __arg3: *void, __arg4: size_t) -> s32 #foreign libc;
sysctlnametomib :: (__arg0: *u8, __arg1: *s32, __arg2: *size_t) -> s32 #foreign libc;

// ------------------------------------------------------- the hand-written ---
//
// C shapes the generator does not cover, written out here.

// `EV_SET` is a macro, so there is nothing in the headers for the generator to
// read. It fills in a `kevent` the way `<sys/event.h>` spells it.
EV_SET :: inline (event: *kevent, ident: u64, filter: s16, flags: u16, fflags: u32, data: s64, udata: *void) {
    event.ident  = ident;
    event.filter = filter;
    event.flags  = flags;
    event.fflags = fflags;
    event.data   = data;
    event.udata  = udata;
}
