//
// The Linux kernel surface: the types, constants and procedures that
// `Linux/module.jai` does not declare by hand.
//
// Generated from the system's own C headers by `tools/cbind`, the way the
// reference generates its bindings — see `tools/cbind/README.md`.
//

#scope_export

// ------------------------------------------------------------- constants ---

ABS_HAT0X :: 16;
ABS_HAT0Y :: 17;
ABS_HAT1X :: 18;
ABS_HAT1Y :: 19;
ABS_HAT2X :: 20;
ABS_HAT2Y :: 21;
ABS_HAT3X :: 22;
ABS_HAT3Y :: 23;
ABS_MAX :: 63;
AT_STATX_DONT_SYNC :: 16384;
AT_STATX_FORCE_SYNC :: 8192;
AT_STATX_SYNC_AS_STAT :: 0;
AT_STATX_SYNC_TYPE :: 24576;
BTN_JOYSTICK :: 288;
BTN_MISC :: 256;
EPOLLERR :: 8;
EPOLLET :: 2147483648;
EPOLLEXCLUSIVE :: 268435456;
EPOLLHUP :: 16;
EPOLLIN :: 1;
EPOLLMSG :: 1024;
EPOLLONESHOT :: 1073741824;
EPOLLOUT :: 4;
EPOLLPRI :: 2;
EPOLLRDBAND :: 128;
EPOLLRDHUP :: 8192;
EPOLLRDNORM :: 64;
EPOLLWAKEUP :: 536870912;
EPOLLWRBAND :: 512;
EPOLLWRNORM :: 256;
EPOLL_CLOEXEC :: 524288;
EPOLL_CTL_ADD :: 1;
EPOLL_CTL_DEL :: 2;
EPOLL_CTL_MOD :: 3;
EV_ABS :: 3;
EV_CNT :: 32;
EV_FF :: 21;
EV_FF_STATUS :: 23;
EV_KEY :: 1;
EV_LED :: 17;
EV_MAX :: 31;
EV_MSC :: 4;
EV_PWR :: 22;
EV_REL :: 2;
EV_REP :: 20;
EV_SND :: 18;
EV_SW :: 5;
EV_SYN :: 0;
IN_ACCESS :: 1;
IN_ALL_EVENTS :: 4095;
IN_ATTRIB :: 4;
IN_CLOEXEC :: 524288;
IN_CLOSE :: 24;
IN_CLOSE_NOWRITE :: 16;
IN_CLOSE_WRITE :: 8;
IN_CREATE :: 256;
IN_DELETE :: 512;
IN_DELETE_SELF :: 1024;
IN_DONT_FOLLOW :: 33554432;
IN_EXCL_UNLINK :: 67108864;
IN_IGNORED :: 32768;
IN_ISDIR :: 1073741824;
IN_MASK_ADD :: 536870912;
IN_MODIFY :: 2;
IN_MOVE :: 192;
IN_MOVED_FROM :: 64;
IN_MOVED_TO :: 128;
IN_MOVE_SELF :: 2048;
IN_NONBLOCK :: 2048;
IN_ONESHOT :: 2147483648;
IN_ONLYDIR :: 16777216;
IN_OPEN :: 32;
IN_Q_OVERFLOW :: 16384;
IN_UNMOUNT :: 8192;
IORING_FILE_INDEX_ALLOC :: 4294967295;
IORING_OFF_CQ_RING :: 134217728;
IORING_OFF_SQES :: 268435456;
IORING_OFF_SQ_RING :: 0;
IORING_REGISTER_FILES_SKIP :: -2;
KEY_MAX :: 767;
STATX_ATIME :: 32;
STATX_ATTR_APPEND :: 32;
STATX_ATTR_AUTOMOUNT :: 4096;
STATX_ATTR_COMPRESSED :: 4;
STATX_ATTR_DAX :: 2097152;
STATX_ATTR_ENCRYPTED :: 2048;
STATX_ATTR_IMMUTABLE :: 16;
STATX_ATTR_MOUNT_ROOT :: 8192;
STATX_ATTR_NODUMP :: 64;
STATX_ATTR_VERITY :: 1048576;
STATX_BASIC_STATS :: 2047;
STATX_BLOCKS :: 1024;
STATX_BTIME :: 2048;
STATX_CTIME :: 128;
STATX_GID :: 16;
STATX_INO :: 256;
STATX_MNT_ID :: 4096;
STATX_MODE :: 2;
STATX_MTIME :: 64;
STATX_NLINK :: 4;
STATX_SIZE :: 512;
STATX_TYPE :: 1;
STATX_UID :: 8;
STATX__RESERVED :: 2147483648;

// ------------------------------------------------------------- the types ---

epoll_data :: union {
    ptr: *void;
    fd: s32;
    _u32: uint32_t;
    _u64: uint64_t;
}

epoll_event :: struct {
    events: uint32_t;
    data: epoll_data_t;
}

inotify_event :: struct {
    wd: s32;
    mask: uint32_t;
    cookie: uint32_t;
    len: uint32_t;
    name: [] u8;
}

sys_info :: struct {
    uptime: __kernel_long_t;
    loads: [3] __kernel_ulong_t;
    totalram: __kernel_ulong_t;
    freeram: __kernel_ulong_t;
    sharedram: __kernel_ulong_t;
    bufferram: __kernel_ulong_t;
    totalswap: __kernel_ulong_t;
    freeswap: __kernel_ulong_t;
    procs: __u16;
    pad: __u16;
    totalhigh: __kernel_ulong_t;
    freehigh: __kernel_ulong_t;
    mem_unit: __u32;
    _f: [0] u8;
}

statx_timestamp_t :: struct {
    tv_sec: __s64;
    tv_nsec: __u32;
    __reserved: __s32;
}

statx_t :: struct {
    stx_mask: __u32;
    stx_blksize: __u32;
    stx_attributes: __u64;
    stx_nlink: __u32;
    stx_uid: __u32;
    stx_gid: __u32;
    stx_mode: __u16;
    __spare0: [1] __u16;
    stx_ino: __u64;
    stx_size: __u64;
    stx_blocks: __u64;
    stx_attributes_mask: __u64;
    stx_atime: statx_timestamp_t;
    stx_btime: statx_timestamp_t;
    stx_ctime: statx_timestamp_t;
    stx_mtime: statx_timestamp_t;
    stx_rdev_major: __u32;
    stx_rdev_minor: __u32;
    stx_dev_major: __u32;
    stx_dev_minor: __u32;
    stx_mnt_id: __u64;
    stx_dio_mem_align: __u32;
    stx_dio_offset_align: __u32;
    stx_subvol: __u64;
    stx_atomic_write_unit_min: __u32;
    stx_atomic_write_unit_max: __u32;
    stx_atomic_write_segments_max: __u32;
    stx_dio_read_offset_align: __u32;
    stx_atomic_write_unit_max_opt: __u32;
    __spare2: [1] __u32;
    __spare3: [8] __u64;
}

input_event :: struct {
    time: timeval;
    type: __u16;
    code: __u16;
    value: __s32;
}

input_absinfo :: struct {
    value: __s32;
    minimum: __s32;
    maximum: __s32;
    fuzz: __s32;
    flat: __s32;
    resolution: __s32;
}

io_uring_sqe :: struct {
    opcode: __u8;
    flags: __u8;
    ioprio: __u16;
    fd: __s32;
    using _anonymous_4: union {
        off: __u64;
        addr2: __u64;
        using _anonymous_2: struct {
            cmd_op: __u32;
            __pad1: __u32;
        }
    }
    using _anonymous_12: union {
        addr: __u64;
        splice_off_in: __u64;
        using _anonymous_2: struct {
            level: __u32;
            optname: __u32;
        }
    }
    len: __u32;
    using _anonymous_21: union {
        rw_flags: __u32;
        fsync_flags: __u32;
        poll_events: __u16;
        poll32_events: __u32;
        sync_range_flags: __u32;
        msg_flags: __u32;
        timeout_flags: __u32;
        accept_flags: __u32;
        cancel_flags: __u32;
        open_flags: __u32;
        statx_flags: __u32;
        fadvise_advice: __u32;
        splice_flags: __u32;
        rename_flags: __u32;
        unlink_flags: __u32;
        hardlink_flags: __u32;
        xattr_flags: __u32;
        msg_ring_flags: __u32;
        uring_cmd_flags: __u32;
        waitid_flags: __u32;
        futex_flags: __u32;
        install_fd_flags: __u32;
        nop_flags: __u32;
        pipe_flags: __u32;
    }
    user_data: __u64;
    using _anonymous_48: union {
        buf_index: __u16;
        buf_group: __u16;
    }
    personality: __u16;
    using _anonymous_53: union {
        splice_fd_in: __s32;
        file_index: __u32;
        zcrx_ifq_idx: __u32;
        optlen: __u32;
        using _anonymous_4: struct {
            addr_len: __u16;
            __pad3: [1] __u16;
        }
        using _anonymous_8: struct {
            write_stream: __u8;
            __pad4: [3] __u8;
        }
    }
    using _anonymous_67: union {
        using _anonymous_0: struct {
            addr3: __u64;
            __pad2: [1] __u64;
        }
        using _anonymous_4: struct {
            attr_ptr: __u64;
            attr_type_mask: __u64;
        }
        optval: __u64;
        cmd: [0] __u8;
    }
}

io_uring_cqe :: struct {
    user_data: __u64;
    res: __s32;
    flags: __u32;
    big_cqe: [] __u64;
}

io_sqring_offsets :: struct {
    head: __u32;
    tail: __u32;
    ring_mask: __u32;
    ring_entries: __u32;
    flags: __u32;
    dropped: __u32;
    array: __u32;
    resv1: __u32;
    user_addr: __u64;
}

io_cqring_offsets :: struct {
    head: __u32;
    tail: __u32;
    ring_mask: __u32;
    ring_entries: __u32;
    overflow: __u32;
    cqes: __u32;
    flags: __u32;
    resv1: __u32;
    user_addr: __u64;
}

io_uring_params :: struct {
    sq_entries: __u32;
    cq_entries: __u32;
    flags: __u32;
    sq_thread_cpu: __u32;
    sq_thread_idle: __u32;
    features: __u32;
    wq_fd: __u32;
    resv: [3] __u32;
    sq_off: io_sqring_offsets;
    cq_off: io_cqring_offsets;
}

io_uring_files_update :: struct {
    offset: __u32;
    resv: __u32;
    fds: __u64;
}

io_uring_rsrc_register :: struct {
    nr: __u32;
    flags: __u32;
    resv2: __u64;
    data: __u64;
    tags: __u64;
}

io_uring_rsrc_update :: struct {
    offset: __u32;
    resv: __u32;
    data: __u64;
}

io_uring_rsrc_update2 :: struct {
    offset: __u32;
    resv: __u32;
    data: __u64;
    tags: __u64;
    nr: __u32;
    resv2: __u32;
}

io_uring_probe_op :: struct {
    op: __u8;
    resv: __u8;
    flags: __u16;
    resv2: __u32;
}

io_uring_probe :: struct {
    last_op: __u8;
    ops_len: __u8;
    resv: __u16;
    resv2: [3] __u32;
    ops: [] io_uring_probe_op;
}

io_uring_restriction :: struct {
    opcode: __u16;
    using _anonymous_1: union {
        register_op: __u8;
        sqe_op: __u8;
        sqe_flags: __u8;
    }
    resv: __u8;
    resv2: [3] __u32;
}

io_uring_buf :: struct {
    addr: __u64;
    len: __u32;
    bid: __u16;
    resv: __u16;
}

io_uring_buf_ring :: struct {
    using _anonymous_0: union {
        using _anonymous_0: struct {
            resv1: __u64;
            resv2: __u32;
            resv3: __u16;
            tail: __u16;
        }
        using _anonymous_6: struct {
            __empty_bufs: struct {
            }
            bufs: [] io_uring_buf;
        }
    }
}

io_uring_buf_reg :: struct {
    ring_addr: __u64;
    ring_entries: __u32;
    bgid: __u16;
    flags: __u16;
    min_left: __u32;
    resv: [5] __u32;
}

io_uring_getevents_arg :: struct {
    sigmask: __u64;
    sigmask_sz: __u32;
    min_wait_usec: __u32;
    ts: __u64;
}

uint32_t :: __uint32_t;
uint64_t :: __uint64_t;
__sigset_t :: struct {
    __val: [16] u64;
}

timeval :: struct {
    tv_sec: __time_t;
    tv_usec: __suseconds_t;
}

epoll_data_t :: union {
    ptr: *void;
    fd: s32;
    _u32: uint32_t;
    _u64: uint64_t;
}

__u8 :: u8;
__u16 :: u16;
__s32 :: s32;
__u32 :: u32;
__s64 :: s64;
__u64 :: u64;
__kernel_long_t :: s64;
__kernel_ulong_t :: u64;
__uint32_t :: u32;
__uint64_t :: u64;
__time_t :: s64;
__suseconds_t :: s64;

// ------------------------------------------------------------------ enums ---

IOSQE_Flag_Bits :: enum s32 {
    IOSQE_FIXED_FILE_BIT;
    IOSQE_IO_DRAIN_BIT;
    IOSQE_IO_LINK_BIT;
    IOSQE_IO_HARDLINK_BIT;
    IOSQE_ASYNC_BIT;
    IOSQE_BUFFER_SELECT_BIT;
    IOSQE_CQE_SKIP_SUCCESS_BIT;
}
io_uring_sqe_flags_bit :: IOSQE_Flag_Bits;

IORING_OP :: enum s32 {
    IORING_OP_NOP;
    IORING_OP_READV;
    IORING_OP_WRITEV;
    IORING_OP_FSYNC;
    IORING_OP_READ_FIXED;
    IORING_OP_WRITE_FIXED;
    IORING_OP_POLL_ADD;
    IORING_OP_POLL_REMOVE;
    IORING_OP_SYNC_FILE_RANGE;
    IORING_OP_SENDMSG;
    IORING_OP_RECVMSG;
    IORING_OP_TIMEOUT;
    IORING_OP_TIMEOUT_REMOVE;
    IORING_OP_ACCEPT;
    IORING_OP_ASYNC_CANCEL;
    IORING_OP_LINK_TIMEOUT;
    IORING_OP_CONNECT;
    IORING_OP_FALLOCATE;
    IORING_OP_OPENAT;
    IORING_OP_CLOSE;
    IORING_OP_FILES_UPDATE;
    IORING_OP_STATX;
    IORING_OP_READ;
    IORING_OP_WRITE;
    IORING_OP_FADVISE;
    IORING_OP_MADVISE;
    IORING_OP_SEND;
    IORING_OP_RECV;
    IORING_OP_OPENAT2;
    IORING_OP_EPOLL_CTL;
    IORING_OP_SPLICE;
    IORING_OP_PROVIDE_BUFFERS;
    IORING_OP_REMOVE_BUFFERS;
    IORING_OP_TEE;
    IORING_OP_SHUTDOWN;
    IORING_OP_RENAMEAT;
    IORING_OP_UNLINKAT;
    IORING_OP_MKDIRAT;
    IORING_OP_SYMLINKAT;
    IORING_OP_LINKAT;
    IORING_OP_MSG_RING;
    IORING_OP_FSETXATTR;
    IORING_OP_SETXATTR;
    IORING_OP_FGETXATTR;
    IORING_OP_GETXATTR;
    IORING_OP_SOCKET;
    IORING_OP_URING_CMD;
    IORING_OP_SEND_ZC;
    IORING_OP_SENDMSG_ZC;
    IORING_OP_READ_MULTISHOT;
    IORING_OP_WAITID;
    IORING_OP_FUTEX_WAIT;
    IORING_OP_FUTEX_WAKE;
    IORING_OP_FUTEX_WAITV;
    IORING_OP_FIXED_FD_INSTALL;
    IORING_OP_FTRUNCATE;
    IORING_OP_BIND;
    IORING_OP_LISTEN;
    IORING_OP_RECV_ZC;
    IORING_OP_EPOLL_WAIT;
    IORING_OP_READV_FIXED;
    IORING_OP_WRITEV_FIXED;
    IORING_OP_PIPE;
    IORING_OP_NOP128;
    IORING_OP_URING_CMD128;
    IORING_OP_LAST;
}
io_uring_op :: IORING_OP;

IO_WQ_Categories :: enum s32 {
    IO_WQ_BOUND;
    IO_WQ_UNBOUND;
}
io_wq_type :: IO_WQ_Categories;

IORING_REGISTER :: enum s32 {
    SRC_REGISTERED :: (1 << 0);
    DST_REPLACE :: (1 << 1);
}

IO_URING_RESTRICTION_OP :: enum s32 {
    IORING_RESTRICTION_REGISTER_OP :: 0;
    IORING_RESTRICTION_SQE_OP :: 1;
    IORING_RESTRICTION_SQE_FLAGS_ALLOWED :: 2;
    IORING_RESTRICTION_SQE_FLAGS_REQUIRED :: 3;
    IORING_RESTRICTION_LAST;
}
io_uring_register_restriction_op :: IO_URING_RESTRICTION_OP;

using IOSQE_Flags :: enum_flags u8 {
    IOSQE_ASYNC :: 16;
    IOSQE_BUFFER_SELECT :: 32;
    IOSQE_CQE_SKIP_SUCCESS :: 64;
    IOSQE_FIXED_FILE :: 1;
    IOSQE_IO_DRAIN :: 2;
    IOSQE_IO_HARDLINK :: 8;
    IOSQE_IO_LINK :: 4;
}

using IORING_Setup_Flags :: enum_flags u32 {
    IORING_SETUP_ATTACH_WQ :: 32;
    IORING_SETUP_CLAMP :: 16;
    IORING_SETUP_COOP_TASKRUN :: 256;
    IORING_SETUP_CQE32 :: 2048;
    IORING_SETUP_CQE_MIXED :: 262144;
    IORING_SETUP_CQSIZE :: 8;
    IORING_SETUP_DEFER_TASKRUN :: 8192;
    IORING_SETUP_HYBRID_IOPOLL :: 131072;
    IORING_SETUP_IOPOLL :: 1;
    IORING_SETUP_NO_MMAP :: 16384;
    IORING_SETUP_NO_SQARRAY :: 65536;
    IORING_SETUP_REGISTERED_FD_ONLY :: 32768;
    IORING_SETUP_R_DISABLED :: 64;
    IORING_SETUP_SINGLE_ISSUER :: 4096;
    IORING_SETUP_SQE128 :: 1024;
    IORING_SETUP_SQE_MIXED :: 524288;
    IORING_SETUP_SQPOLL :: 2;
    IORING_SETUP_SQ_AFF :: 4;
    IORING_SETUP_SQ_REWIND :: 1048576;
    IORING_SETUP_SUBMIT_ALL :: 128;
    IORING_SETUP_TASKRUN_FLAG :: 512;
}

using IORING_FEAT_Flags :: enum_flags u32 {
    IORING_FEAT_CQE_SKIP :: 2048;
    IORING_FEAT_CUR_PERSONALITY :: 16;
    IORING_FEAT_EXT_ARG :: 256;
    IORING_FEAT_FAST_POLL :: 32;
    IORING_FEAT_LINKED_FILE :: 4096;
    IORING_FEAT_MIN_TIMEOUT :: 32768;
    IORING_FEAT_NATIVE_WORKERS :: 512;
    IORING_FEAT_NODROP :: 2;
    IORING_FEAT_NO_IOWAIT :: 131072;
    IORING_FEAT_POLL_32BITS :: 64;
    IORING_FEAT_RECVSEND_BUNDLE :: 16384;
    IORING_FEAT_REG_REG_RING :: 8192;
    IORING_FEAT_RSRC_TAGS :: 1024;
    IORING_FEAT_RW_ATTR :: 65536;
    IORING_FEAT_RW_CUR_POS :: 8;
    IORING_FEAT_SINGLE_MMAP :: 1;
    IORING_FEAT_SQPOLL_NONFIXED :: 128;
    IORING_FEAT_SUBMIT_STABLE :: 4;
}

using IORING_SQ_Flags :: enum_flags u32 {
    IORING_SQ_CQ_OVERFLOW :: 2;
    IORING_SQ_NEED_WAKEUP :: 1;
    IORING_SQ_TASKRUN :: 4;
}

using IORING_CQ_Flags :: enum_flags u32 {
    IORING_CQ_EVENTFD_DISABLED :: 1;
}

using IORING_ENTER :: enum_flags u32 {
    IORING_ENTER_ABS_TIMER :: 32;
    IORING_ENTER_EXT_ARG :: 8;
    IORING_ENTER_EXT_ARG_REG :: 64;
    IORING_ENTER_GETEVENTS :: 1;
    IORING_ENTER_NO_IOWAIT :: 128;
    IORING_ENTER_REGISTERED_RING :: 16;
    IORING_ENTER_SQ_WAIT :: 4;
    IORING_ENTER_SQ_WAKEUP :: 2;
}

using IORING_ACCEPT_Flags :: enum_flags u16 {
    IORING_ACCEPT_DONTWAIT :: 2;
    IORING_ACCEPT_MULTISHOT :: 1;
    IORING_ACCEPT_POLL_FIRST :: 4;
}

using IORING_RECVSEND_Flags :: enum_flags u16 {
    IORING_RECVSEND_BUNDLE :: 16;
    IORING_RECVSEND_FIXED_BUF :: 4;
    IORING_RECVSEND_POLL_FIRST :: 1;
}

using IORING_POLL_Flags :: enum_flags u32 {
    IORING_POLL_ADD_LEVEL :: 8;
    IORING_POLL_ADD_MULTI :: 1;
    IORING_POLL_UPDATE_EVENTS :: 2;
    IORING_POLL_UPDATE_USER_DATA :: 4;
}

using IORING_RSRC_Flags :: enum_flags u32 {
    IORING_RSRC_REGISTER_SPARSE :: 1;
}

using IOCQE_Flags :: enum_flags u32 {
    IORING_CQE_F_32 :: 32768;
    IORING_CQE_F_BUFFER :: 1;
    IORING_CQE_F_BUF_MORE :: 16;
    IORING_CQE_F_MORE :: 2;
    IORING_CQE_F_NOTIF :: 8;
    IORING_CQE_F_SKIP :: 32;
    IORING_CQE_F_SOCK_NONEMPTY :: 4;
    IORING_CQE_F_TSTAMP_HW :: 65536;
}


// ------------------------------------------------------------- procedures ---

epoll_create :: (__size: s32) -> s32 #foreign libc;
epoll_create1 :: (__flags: s32) -> s32 #foreign libc;
epoll_ctl :: (__epfd: s32, __op: s32, __fd: s32, __event: *epoll_event) -> s32 #foreign libc;
epoll_pwait :: (__epfd: s32, __events: *epoll_event, __maxevents: s32, __timeout: s32, __ss: *__sigset_t) -> s32 #foreign libc;
epoll_wait :: (__epfd: s32, __events: *epoll_event, __maxevents: s32, __timeout: s32) -> s32 #foreign libc;
inotify_add_watch :: (__fd: s32, __name: *u8, __mask: uint32_t) -> s32 #foreign libc;
inotify_init :: () -> s32 #foreign libc;
inotify_rm_watch :: (__fd: s32, __wd: s32) -> s32 #foreign libc;
sysinfo :: (__info: *sys_info) -> s32 #foreign libc;

// ---------------------------------------------------------- hand-written ---

// The three io_uring calls have no libc wrappers: a program makes the syscall
// itself. The numbers are the kernel's.
//
// `syscall` is C-variadic, so each argument travels on its own rather than
// gathered into a slice (**L§7.11**).
syscall :: (number: s64, args: ..Any) -> s64 #foreign libc;
SYS_io_uring_setup    :: 425;
SYS_io_uring_enter    :: 426;
SYS_io_uring_register :: 427;

io_uring_setup :: (entries: u32, params: *io_uring_params) -> s32 {
    return cast(s32) syscall(SYS_io_uring_setup, entries, params);
}

io_uring_enter :: (fd: s32, to_submit: u32, min_complete: u32, flags: u32, sig: *void, sz: s64) -> s32 {
    return cast(s32) syscall(SYS_io_uring_enter, fd, to_submit, min_complete, flags, sig, sz);
}

io_uring_register :: (fd: s32, opcode: u32, arg: *void, nr_args: u32) -> s32 {
    return cast(s32) syscall(SYS_io_uring_register, fd, opcode, arg, nr_args);
}

// `epoll_ctl`'s operation, which the header spells as three `#define`s.
Epoll_Ctl :: enum s32 {
    ADD :: 1;
    DEL :: 2;
    MOD :: 3;
}

// Mach's "no error", which the reference carries for the platforms that have
// it; zero everywhere.
KERN_SUCCESS :: 0;

// The read/write flags an io_uring submission carries.
__kernel_rwf_t :: s32;

// `<sys/epoll.h>` defines these two outside the event enum.
EPOLLNVAL        :: 0x20;
EPOLL_URING_WAKE :: 0x8000_0000;
