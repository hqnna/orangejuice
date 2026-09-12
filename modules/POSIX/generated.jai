//
// The glibc surface: the types, constants, procedures and globals that
// `POSIX/module.jai` and `POSIX/linux.jai` do not declare by hand.
//
// This file is Linux's alone. `POSIX/module.jai` loads it only under
// `#if OS == .LINUX`, beside `linux.jai`; the Darwin side of the module is
// `macos.jai`, which has no generated binding behind it.
//
// Generated from the system's own C headers rather than typed out, the way the
// reference generates its bindings — a constant's value is whatever
// `<fcntl.h>` says it is on the machine the compiler was built on, and a
// procedure's parameter names are the ones glibc gave them. What is here is
// therefore a projection of the C ABI, not a transcription of anybody's file.
//
// The generator lives in `tools/cbind`; `tools/cbind/README.md` says how to
// run it again.
//

#scope_export


// -------------------------------------------------------------- constants ---

ACCESSPERMS :: 511;
ALLPERMS :: 4095;
AT_EACCESS :: 512;
AT_EMPTY_PATH :: 4096;
AT_FDCWD :: -100;
AT_NO_AUTOMOUNT :: 2048;
AT_REMOVEDIR :: 512;
AT_SYMLINK_FOLLOW :: 1024;
AT_SYMLINK_NOFOLLOW :: 256;
CLOCKS_PER_SEC :: 1000000;
CLONE_CHILD_CLEARTID :: 2097152;
CLONE_CHILD_SETTID :: 16777216;
CLONE_DETACHED :: 4194304;
CLONE_FILES :: 1024;
CLONE_FS :: 512;
CLONE_IO :: 2147483648;
CLONE_NEWCGROUP :: 33554432;
CLONE_NEWIPC :: 134217728;
CLONE_NEWNET :: 1073741824;
CLONE_NEWNS :: 131072;
CLONE_NEWPID :: 536870912;
CLONE_NEWUSER :: 268435456;
CLONE_NEWUTS :: 67108864;
CLONE_PARENT :: 32768;
CLONE_PARENT_SETTID :: 1048576;
CLONE_PTRACE :: 8192;
CLONE_SETTLS :: 524288;
CLONE_SIGHAND :: 2048;
CLONE_SYSVSEM :: 262144;
CLONE_THREAD :: 65536;
CLONE_UNTRACED :: 8388608;
CLONE_VFORK :: 16384;
CLONE_VM :: 256;
CPU_SETSIZE :: 1024;
CSIGNAL :: 255;
DEFFILEMODE :: 438;
DN_ACCESS :: 1;
DN_ATTRIB :: 32;
DN_CREATE :: 4;
DN_DELETE :: 8;
DN_MODIFY :: 2;
DN_MULTISHOT :: 2147483648;
DN_RENAME :: 16;
E2BIG :: 7;
EADDRINUSE :: 98;
EADDRNOTAVAIL :: 99;
EADV :: 68;
EAFNOSUPPORT :: 97;
EALREADY :: 114;
EBADE :: 52;
EBADFD :: 77;
EBADMSG :: 74;
EBADR :: 53;
EBADRQC :: 56;
EBADSLT :: 57;
EBFONT :: 59;
EBUSY :: 16;
ECANCELED :: 125;
ECHILD :: 10;
ECHRNG :: 44;
ECOMM :: 70;
ECONNABORTED :: 103;
ECONNREFUSED :: 111;
ECONNRESET :: 104;
EDEADLK :: 35;
EDEADLOCK :: 35;
EDESTADDRREQ :: 89;
EDOM :: 33;
EDOTDOT :: 73;
EDQUOT :: 122;
EFAULT :: 14;
EFBIG :: 27;
EHOSTDOWN :: 112;
EHOSTUNREACH :: 113;
EHWPOISON :: 133;
EIDRM :: 43;
EILSEQ :: 84;
EINPROGRESS :: 115;
EISCONN :: 106;
EISNAM :: 120;
EKEYEXPIRED :: 127;
EKEYREJECTED :: 129;
EKEYREVOKED :: 128;
EL2HLT :: 51;
EL2NSYNC :: 45;
EL3HLT :: 46;
EL3RST :: 47;
ELIBACC :: 79;
ELIBBAD :: 80;
ELIBEXEC :: 83;
ELIBMAX :: 82;
ELIBSCN :: 81;
ELNRNG :: 48;
ELOOP :: 40;
EMEDIUMTYPE :: 124;
EMFILE :: 24;
EMLINK :: 31;
EMSGSIZE :: 90;
EMULTIHOP :: 72;
ENAMETOOLONG :: 36;
ENAVAIL :: 119;
ENETDOWN :: 100;
ENETRESET :: 102;
ENETUNREACH :: 101;
ENFILE :: 23;
ENOANO :: 55;
ENOBUFS :: 105;
ENOCSI :: 50;
ENODATA :: 61;
ENODEV :: 19;
ENOEXEC :: 8;
ENOKEY :: 126;
ENOLCK :: 37;
ENOLINK :: 67;
ENOMEDIUM :: 123;
ENOMSG :: 42;
ENONET :: 64;
ENOPKG :: 65;
ENOPROTOOPT :: 92;
ENOSPC :: 28;
ENOSR :: 63;
ENOSTR :: 60;
ENOSYS :: 38;
ENOTBLK :: 15;
ENOTCONN :: 107;
ENOTEMPTY :: 39;
ENOTNAM :: 118;
ENOTRECOVERABLE :: 131;
ENOTSOCK :: 88;
ENOTSUP :: 95;
ENOTTY :: 25;
ENOTUNIQ :: 76;
ENXIO :: 6;
EOPNOTSUPP :: 95;
EOVERFLOW :: 75;
EOWNERDEAD :: 130;
EPFNOSUPPORT :: 96;
EPIPE :: 32;
EPROTO :: 71;
EPROTONOSUPPORT :: 93;
EPROTOTYPE :: 91;
ERANGE :: 34;
EREMCHG :: 78;
EREMOTE :: 66;
EREMOTEIO :: 121;
ERESTART :: 85;
ERFKILL :: 132;
EROFS :: 30;
ESHUTDOWN :: 108;
ESOCKTNOSUPPORT :: 94;
ESPIPE :: 29;
ESRCH :: 3;
ESRMNT :: 69;
ESTALE :: 116;
ESTRPIPE :: 86;
ETIME :: 62;
ETIMEDOUT :: 110;
ETOOMANYREFS :: 109;
ETXTBSY :: 26;
EUCLEAN :: 117;
EUNATCH :: 49;
EUSERS :: 87;
EWOULDBLOCK :: 11;
EXDEV :: 18;
EXFULL :: 54;
FAPPEND :: 1024;
FASYNC :: 8192;
FD_CLOEXEC :: 1;
FFSYNC :: 1052672;
FIOASYNC :: 21586;
FIOCLEX :: 21585;
FIONBIO :: 21537;
FIONCLEX :: 21584;
FIONREAD :: 21531;
FIOQSIZE :: 21600;
FNDELAY :: 2048;
FNM_CASEFOLD :: 16;
FNM_EXTMATCH :: 32;
FNM_FILE_NAME :: 1;
FNM_LEADING_DIR :: 8;
FNM_NOESCAPE :: 2;
FNM_NOMATCH :: 1;
FNM_NOSYS :: -1;
FNM_PATHNAME :: 1;
FNM_PERIOD :: 4;
FNONBLOCK :: 2048;
F_ADD_SEALS :: 1033;
F_DUPFD :: 0;
F_DUPFD_CLOEXEC :: 1030;
F_EXLCK :: 4;
F_GETLEASE :: 1025;
F_GETLK :: 5;
F_GETLK64 :: 5;
F_GETOWN :: 9;
F_GETOWN_EX :: 16;
F_GETPIPE_SZ :: 1032;
F_GETSIG :: 11;
F_GET_FILE_RW_HINT :: 1037;
F_GET_RW_HINT :: 1035;
F_GET_SEALS :: 1034;
F_LOCK :: 1;
F_NOTIFY :: 1026;
F_OFD_GETLK :: 36;
F_OFD_SETLK :: 37;
F_OFD_SETLKW :: 38;
F_OK :: 0;
F_RDLCK :: 0;
F_SEAL_GROW :: 4;
F_SEAL_SEAL :: 1;
F_SEAL_SHRINK :: 2;
F_SEAL_WRITE :: 8;
F_SETLEASE :: 1024;
F_SETLK :: 6;
F_SETLK64 :: 6;
F_SETLKW :: 7;
F_SETLKW64 :: 7;
F_SETOWN :: 8;
F_SETOWN_EX :: 15;
F_SETPIPE_SZ :: 1031;
F_SETSIG :: 10;
F_SET_FILE_RW_HINT :: 1038;
F_SET_RW_HINT :: 1036;
F_SHLCK :: 8;
F_TEST :: 3;
F_TLOCK :: 2;
F_ULOCK :: 0;
F_UNLCK :: 2;
F_WRLCK :: 1;
IPC_CREAT :: 512;
IPC_EXCL :: 1024;
IPC_INFO :: 3;
IPC_NOWAIT :: 2048;
IPC_PRIVATE :: 0;
IPC_RMID :: 0;
IPC_SET :: 1;
IPC_STAT :: 2;
LM_ID_BASE :: 0;
LM_ID_NEWLM :: -1;
LOCK_EX :: 2;
LOCK_MAND :: 32;
LOCK_NB :: 4;
LOCK_READ :: 64;
LOCK_RW :: 192;
LOCK_SH :: 1;
LOCK_UN :: 8;
LOCK_WRITE :: 128;
L_INCR :: 1;
L_SET :: 0;
L_XTND :: 2;
MADV_DODUMP :: 17;
MADV_DOFORK :: 11;
MADV_DONTDUMP :: 16;
MADV_DONTFORK :: 10;
MADV_DONTNEED :: 4;
MADV_FREE :: 8;
MADV_HUGEPAGE :: 14;
MADV_HWPOISON :: 100;
MADV_KEEPONFORK :: 19;
MADV_MERGEABLE :: 12;
MADV_NOHUGEPAGE :: 15;
MADV_NORMAL :: 0;
MADV_RANDOM :: 1;
MADV_REMOVE :: 9;
MADV_SEQUENTIAL :: 2;
MADV_UNMERGEABLE :: 13;
MADV_WILLNEED :: 3;
MADV_WIPEONFORK :: 18;
MAP_32BIT :: 64;
MAP_ANON :: 32;
MAP_DENYWRITE :: 2048;
MAP_EXECUTABLE :: 4096;
MAP_FAILED :: -1;
MAP_FILE :: 0;
MAP_FIXED :: 16;
MAP_GROWSDOWN :: 256;
MAP_HUGETLB :: 262144;
MAP_HUGE_MASK :: 63;
MAP_HUGE_SHIFT :: 26;
MAP_LOCKED :: 8192;
MAP_NONBLOCK :: 65536;
MAP_NORESERVE :: 16384;
MAP_POPULATE :: 32768;
MAP_STACK :: 131072;
MAP_TYPE :: 15;
MAX_HANDLE_SZ :: 128;
MCL_CURRENT :: 1;
MCL_FUTURE :: 2;
MCL_ONFAULT :: 4;
MREMAP_FIXED :: 2;
MREMAP_MAYMOVE :: 1;
MS_ASYNC :: 1;
MS_INVALIDATE :: 2;
MS_SYNC :: 4;
NGREG :: 23;
NSIG :: 65;
NSS_BUFLEN_PASSWD :: 1024;
O_ACCMODE :: 3;
O_ASYNC :: 8192;
O_CLOEXEC :: 524288;
O_DIRECT :: 16384;
O_DSYNC :: 4096;
O_FSYNC :: 1052672;
O_LARGEFILE :: 0;
O_NDELAY :: 2048;
O_NOATIME :: 262144;
O_NOCTTY :: 256;
O_NOFOLLOW :: 131072;
O_PATH :: 2097152;
O_RSYNC :: 1052672;
O_SYNC :: 1052672;
O_TMPFILE :: 4259840;
POLLERR :: 8;
POLLHUP :: 16;
POLLIN :: 1;
POLLMSG :: 1024;
POLLNVAL :: 32;
POLLOUT :: 4;
POLLPRI :: 2;
POLLRDBAND :: 128;
POLLRDHUP :: 8192;
POLLRDNORM :: 64;
POLLREMOVE :: 4096;
POLLWRBAND :: 512;
POLLWRNORM :: 256;
POSIX_FADV_DONTNEED :: 4;
POSIX_FADV_NOREUSE :: 5;
POSIX_FADV_NORMAL :: 0;
POSIX_FADV_RANDOM :: 1;
POSIX_FADV_SEQUENTIAL :: 2;
POSIX_FADV_WILLNEED :: 3;
PRIO_MAX :: 20;
PRIO_MIN :: -20;
PROT_EXEC :: 4;
PROT_GROWSDOWN :: 16777216;
PROT_GROWSUP :: 33554432;
PROT_NONE :: 0;
PTHREAD_BARRIER_SERIAL_THREAD :: -1;
PTHREAD_CANCELED :: -1;
PTHREAD_ONCE_INIT :: 0;
RLIM64_INFINITY :: -1;
RLIM_INFINITY :: -1;
RLIM_SAVED_CUR :: -1;
RLIM_SAVED_MAX :: -1;
RTLD_BINDING_MASK :: 3;
RTLD_DEEPBIND :: 8;
RTLD_DEFAULT :: 0;
RTLD_GLOBAL :: 256;
RTLD_LAZY :: 1;
RTLD_LOCAL :: 0;
RTLD_NEXT :: -1;
RTLD_NODELETE :: 4096;
RTLD_NOLOAD :: 4;
RTLD_NOW :: 2;
RWF_WRITE_LIFE_NOT_SET :: 0;
RWH_WRITE_LIFE_EXTREME :: 5;
RWH_WRITE_LIFE_LONG :: 4;
RWH_WRITE_LIFE_MEDIUM :: 3;
RWH_WRITE_LIFE_NONE :: 1;
RWH_WRITE_LIFE_SHORT :: 2;
R_OK :: 4;
SA_INTERRUPT :: 536870912;
SA_NOCLDSTOP :: 1;
SA_NOCLDWAIT :: 2;
SA_NODEFER :: 1073741824;
SA_NOMASK :: 1073741824;
SA_ONESHOT :: 2147483648;
SA_ONSTACK :: 134217728;
SA_RESETHAND :: 2147483648;
SA_RESTART :: 268435456;
SA_SIGINFO :: 4;
SA_STACK :: 134217728;
SCHED_BATCH :: 3;
SCHED_DEADLINE :: 6;
SCHED_FIFO :: 1;
SCHED_IDLE :: 5;
SCHED_ISO :: 4;
SCHED_OTHER :: 0;
SCHED_RESET_ON_FORK :: 1073741824;
SCHED_RR :: 2;
SEEK_DATA :: 3;
SEEK_HOLE :: 4;
SHM_DEST :: 512;
SHM_EXEC :: 32768;
SHM_HUGETLB :: 2048;
SHM_INFO :: 14;
SHM_LOCK :: 11;
SHM_LOCKED :: 1024;
SHM_NORESERVE :: 4096;
SHM_R :: 256;
SHM_RDONLY :: 4096;
SHM_REMAP :: 16384;
SHM_RND :: 8192;
SHM_STAT :: 13;
SHM_UNLOCK :: 12;
SHM_W :: 128;
SIGABRT :: 6;
SIGALRM :: 14;
SIGBUS :: 7;
SIGCHLD :: 17;
SIGCLD :: 17;
SIGCONT :: 18;
SIGFPE :: 8;
SIGHUP :: 1;
SIGILL :: 4;
SIGIO :: 29;
SIGIOT :: 6;
SIGPIPE :: 13;
SIGPOLL :: 29;
SIGPROF :: 27;
SIGPWR :: 30;
SIGQUIT :: 3;
SIGSEGV :: 11;
SIGSTKFLT :: 16;
SIGSTOP :: 19;
SIGSYS :: 31;
SIGTRAP :: 5;
SIGTSTP :: 20;
SIGTTIN :: 21;
SIGTTOU :: 22;
SIGURG :: 23;
SIGUSR1 :: 10;
SIGUSR2 :: 12;
SIGVTALRM :: 26;
SIGWINCH :: 28;
SIGXCPU :: 24;
SIGXFSZ :: 25;
SIG_BLOCK :: 0;
SIG_DFL :: 0;
SIG_ERR :: -1;
SIG_HOLD :: 2;
SIG_IGN :: 1;
SIG_SETMASK :: 2;
SIG_UNBLOCK :: 1;
SIOCADDDLCI :: 35200;
SIOCADDMULTI :: 35121;
SIOCADDRT :: 35083;
SIOCDARP :: 35155;
SIOCDELDLCI :: 35201;
SIOCDELMULTI :: 35122;
SIOCDELRT :: 35084;
SIOCDEVPRIVATE :: 35312;
SIOCDIFADDR :: 35126;
SIOCDRARP :: 35168;
SIOCGARP :: 35156;
SIOCGIFADDR :: 35093;
SIOCGIFBR :: 35136;
SIOCGIFBRDADDR :: 35097;
SIOCGIFCONF :: 35090;
SIOCGIFCOUNT :: 35128;
SIOCGIFDSTADDR :: 35095;
SIOCGIFENCAP :: 35109;
SIOCGIFFLAGS :: 35091;
SIOCGIFHWADDR :: 35111;
SIOCGIFINDEX :: 35123;
SIOCGIFMAP :: 35184;
SIOCGIFMEM :: 35103;
SIOCGIFMETRIC :: 35101;
SIOCGIFMTU :: 35105;
SIOCGIFNAME :: 35088;
SIOCGIFNETMASK :: 35099;
SIOCGIFPFLAGS :: 35125;
SIOCGIFSLAVE :: 35113;
SIOCGIFTXQLEN :: 35138;
SIOCGRARP :: 35169;
SIOCPROTOPRIVATE :: 35296;
SIOCRTMSG :: 35085;
SIOCSARP :: 35157;
SIOCSIFADDR :: 35094;
SIOCSIFBR :: 35137;
SIOCSIFBRDADDR :: 35098;
SIOCSIFDSTADDR :: 35096;
SIOCSIFENCAP :: 35110;
SIOCSIFFLAGS :: 35092;
SIOCSIFHWADDR :: 35108;
SIOCSIFHWBROADCAST :: 35127;
SIOCSIFLINK :: 35089;
SIOCSIFMAP :: 35185;
SIOCSIFMEM :: 35104;
SIOCSIFMETRIC :: 35102;
SIOCSIFMTU :: 35106;
SIOCSIFNAME :: 35107;
SIOCSIFNETMASK :: 35100;
SIOCSIFPFLAGS :: 35124;
SIOCSIFSLAVE :: 35120;
SIOCSIFTXQLEN :: 35139;
SIOCSRARP :: 35170;
SIOGIFINDEX :: 35123;
SPLICE_F_GIFT :: 8;
SPLICE_F_MORE :: 4;
SPLICE_F_MOVE :: 1;
SPLICE_F_NONBLOCK :: 2;
SYNC_FILE_RANGE_WAIT_AFTER :: 4;
SYNC_FILE_RANGE_WAIT_BEFORE :: 1;
SYNC_FILE_RANGE_WRITE :: 2;
SYS__sysctl :: 156;
SYS_accept :: 43;
SYS_accept4 :: 288;
SYS_access :: 21;
SYS_acct :: 163;
SYS_add_key :: 248;
SYS_adjtimex :: 159;
SYS_afs_syscall :: 183;
SYS_alarm :: 37;
SYS_arch_prctl :: 158;
SYS_bind :: 49;
SYS_bpf :: 321;
SYS_brk :: 12;
SYS_capget :: 125;
SYS_capset :: 126;
SYS_chdir :: 80;
SYS_chmod :: 90;
SYS_chown :: 92;
SYS_chroot :: 161;
SYS_clock_adjtime :: 305;
SYS_clock_getres :: 229;
SYS_clock_gettime :: 228;
SYS_clock_nanosleep :: 230;
SYS_clock_settime :: 227;
SYS_clone :: 56;
SYS_close :: 3;
SYS_connect :: 42;
SYS_copy_file_range :: 326;
SYS_creat :: 85;
SYS_create_module :: 174;
SYS_delete_module :: 176;
SYS_dup :: 32;
SYS_dup2 :: 33;
SYS_dup3 :: 292;
SYS_epoll_create :: 213;
SYS_epoll_create1 :: 291;
SYS_epoll_ctl :: 233;
SYS_epoll_ctl_old :: 214;
SYS_epoll_pwait :: 281;
SYS_epoll_wait :: 232;
SYS_epoll_wait_old :: 215;
SYS_eventfd :: 284;
SYS_eventfd2 :: 290;
SYS_execve :: 59;
SYS_execveat :: 322;
SYS_exit :: 60;
SYS_exit_group :: 231;
SYS_faccessat :: 269;
SYS_fadvise64 :: 221;
SYS_fallocate :: 285;
SYS_fanotify_init :: 300;
SYS_fanotify_mark :: 301;
SYS_fchdir :: 81;
SYS_fchmod :: 91;
SYS_fchmodat :: 268;
SYS_fchown :: 93;
SYS_fchownat :: 260;
SYS_fcntl :: 72;
SYS_fdatasync :: 75;
SYS_fgetxattr :: 193;
SYS_finit_module :: 313;
SYS_flistxattr :: 196;
SYS_flock :: 73;
SYS_fork :: 57;
SYS_fremovexattr :: 199;
SYS_fsetxattr :: 190;
SYS_fstat :: 5;
SYS_fstatfs :: 138;
SYS_fsync :: 74;
SYS_ftruncate :: 77;
SYS_futex :: 202;
SYS_futimesat :: 261;
SYS_get_kernel_syms :: 177;
SYS_get_mempolicy :: 239;
SYS_get_robust_list :: 274;
SYS_get_thread_area :: 211;
SYS_getcpu :: 309;
SYS_getcwd :: 79;
SYS_getdents :: 78;
SYS_getdents64 :: 217;
SYS_getegid :: 108;
SYS_geteuid :: 107;
SYS_getgid :: 104;
SYS_getgroups :: 115;
SYS_getitimer :: 36;
SYS_getpeername :: 52;
SYS_getpgid :: 121;
SYS_getpgrp :: 111;
SYS_getpid :: 39;
SYS_getpmsg :: 181;
SYS_getppid :: 110;
SYS_getpriority :: 140;
SYS_getrandom :: 318;
SYS_getresgid :: 120;
SYS_getresuid :: 118;
SYS_getrlimit :: 97;
SYS_getrusage :: 98;
SYS_getsid :: 124;
SYS_getsockname :: 51;
SYS_getsockopt :: 55;
SYS_gettid :: 186;
SYS_gettimeofday :: 96;
SYS_getuid :: 102;
SYS_getxattr :: 191;
SYS_init_module :: 175;
SYS_inotify_add_watch :: 254;
SYS_inotify_init :: 253;
SYS_inotify_init1 :: 294;
SYS_inotify_rm_watch :: 255;
SYS_io_cancel :: 210;
SYS_io_destroy :: 207;
SYS_io_getevents :: 208;
SYS_io_setup :: 206;
SYS_io_submit :: 209;
SYS_ioctl :: 16;
SYS_ioperm :: 173;
SYS_iopl :: 172;
SYS_ioprio_get :: 252;
SYS_ioprio_set :: 251;
SYS_kcmp :: 312;
SYS_kexec_file_load :: 320;
SYS_kexec_load :: 246;
SYS_keyctl :: 250;
SYS_kill :: 62;
SYS_lchown :: 94;
SYS_lgetxattr :: 192;
SYS_link :: 86;
SYS_linkat :: 265;
SYS_listen :: 50;
SYS_listxattr :: 194;
SYS_llistxattr :: 195;
SYS_lookup_dcookie :: 212;
SYS_lremovexattr :: 198;
SYS_lseek :: 8;
SYS_lsetxattr :: 189;
SYS_lstat :: 6;
SYS_madvise :: 28;
SYS_mbind :: 237;
SYS_membarrier :: 324;
SYS_memfd_create :: 319;
SYS_migrate_pages :: 256;
SYS_mincore :: 27;
SYS_mkdir :: 83;
SYS_mkdirat :: 258;
SYS_mknod :: 133;
SYS_mknodat :: 259;
SYS_mlock :: 149;
SYS_mlock2 :: 325;
SYS_mlockall :: 151;
SYS_mmap :: 9;
SYS_modify_ldt :: 154;
SYS_mount :: 165;
SYS_move_pages :: 279;
SYS_mprotect :: 10;
SYS_mq_getsetattr :: 245;
SYS_mq_notify :: 244;
SYS_mq_open :: 240;
SYS_mq_timedreceive :: 243;
SYS_mq_timedsend :: 242;
SYS_mq_unlink :: 241;
SYS_mremap :: 25;
SYS_msgctl :: 71;
SYS_msgget :: 68;
SYS_msgrcv :: 70;
SYS_msgsnd :: 69;
SYS_msync :: 26;
SYS_munlock :: 150;
SYS_munlockall :: 152;
SYS_munmap :: 11;
SYS_name_to_handle_at :: 303;
SYS_nanosleep :: 35;
SYS_newfstatat :: 262;
SYS_nfsservctl :: 180;
SYS_open :: 2;
SYS_open_by_handle_at :: 304;
SYS_openat :: 257;
SYS_pause :: 34;
SYS_perf_event_open :: 298;
SYS_personality :: 135;
SYS_pipe :: 22;
SYS_pipe2 :: 293;
SYS_pivot_root :: 155;
SYS_pkey_alloc :: 330;
SYS_pkey_free :: 331;
SYS_pkey_mprotect :: 329;
SYS_poll :: 7;
SYS_ppoll :: 271;
SYS_prctl :: 157;
SYS_pread64 :: 17;
SYS_preadv :: 295;
SYS_preadv2 :: 327;
SYS_prlimit64 :: 302;
SYS_process_vm_readv :: 310;
SYS_process_vm_writev :: 311;
SYS_pselect6 :: 270;
SYS_ptrace :: 101;
SYS_putpmsg :: 182;
SYS_pwrite64 :: 18;
SYS_pwritev :: 296;
SYS_pwritev2 :: 328;
SYS_query_module :: 178;
SYS_quotactl :: 179;
SYS_read :: 0;
SYS_readahead :: 187;
SYS_readlink :: 89;
SYS_readlinkat :: 267;
SYS_readv :: 19;
SYS_reboot :: 169;
SYS_recvfrom :: 45;
SYS_recvmmsg :: 299;
SYS_recvmsg :: 47;
SYS_remap_file_pages :: 216;
SYS_removexattr :: 197;
SYS_rename :: 82;
SYS_renameat :: 264;
SYS_renameat2 :: 316;
SYS_request_key :: 249;
SYS_restart_syscall :: 219;
SYS_rmdir :: 84;
SYS_rt_sigaction :: 13;
SYS_rt_sigpending :: 127;
SYS_rt_sigprocmask :: 14;
SYS_rt_sigqueueinfo :: 129;
SYS_rt_sigreturn :: 15;
SYS_rt_sigsuspend :: 130;
SYS_rt_sigtimedwait :: 128;
SYS_rt_tgsigqueueinfo :: 297;
SYS_sched_get_priority_max :: 146;
SYS_sched_get_priority_min :: 147;
SYS_sched_getaffinity :: 204;
SYS_sched_getattr :: 315;
SYS_sched_getparam :: 143;
SYS_sched_getscheduler :: 145;
SYS_sched_rr_get_interval :: 148;
SYS_sched_setaffinity :: 203;
SYS_sched_setattr :: 314;
SYS_sched_setparam :: 142;
SYS_sched_setscheduler :: 144;
SYS_sched_yield :: 24;
SYS_seccomp :: 317;
SYS_security :: 185;
SYS_select :: 23;
SYS_semctl :: 66;
SYS_semget :: 64;
SYS_semop :: 65;
SYS_semtimedop :: 220;
SYS_sendfile :: 40;
SYS_sendmmsg :: 307;
SYS_sendmsg :: 46;
SYS_sendto :: 44;
SYS_set_mempolicy :: 238;
SYS_set_robust_list :: 273;
SYS_set_thread_area :: 205;
SYS_set_tid_address :: 218;
SYS_setdomainname :: 171;
SYS_setfsgid :: 123;
SYS_setfsuid :: 122;
SYS_setgid :: 106;
SYS_setgroups :: 116;
SYS_sethostname :: 170;
SYS_setitimer :: 38;
SYS_setns :: 308;
SYS_setpgid :: 109;
SYS_setpriority :: 141;
SYS_setregid :: 114;
SYS_setresgid :: 119;
SYS_setresuid :: 117;
SYS_setreuid :: 113;
SYS_setrlimit :: 160;
SYS_setsid :: 112;
SYS_setsockopt :: 54;
SYS_settimeofday :: 164;
SYS_setuid :: 105;
SYS_setxattr :: 188;
SYS_shmat :: 30;
SYS_shmctl :: 31;
SYS_shmdt :: 67;
SYS_shmget :: 29;
SYS_shutdown :: 48;
SYS_sigaltstack :: 131;
SYS_signalfd :: 282;
SYS_signalfd4 :: 289;
SYS_socket :: 41;
SYS_socketpair :: 53;
SYS_splice :: 275;
SYS_stat :: 4;
SYS_statfs :: 137;
SYS_statx :: 332;
SYS_swapoff :: 168;
SYS_swapon :: 167;
SYS_symlink :: 88;
SYS_symlinkat :: 266;
SYS_sync :: 162;
SYS_sync_file_range :: 277;
SYS_syncfs :: 306;
SYS_sysfs :: 139;
SYS_sysinfo :: 99;
SYS_syslog :: 103;
SYS_tee :: 276;
SYS_tgkill :: 234;
SYS_time :: 201;
SYS_timer_create :: 222;
SYS_timer_delete :: 226;
SYS_timer_getoverrun :: 225;
SYS_timer_gettime :: 224;
SYS_timer_settime :: 223;
SYS_timerfd_create :: 283;
SYS_timerfd_gettime :: 287;
SYS_timerfd_settime :: 286;
SYS_times :: 100;
SYS_tkill :: 200;
SYS_truncate :: 76;
SYS_tuxcall :: 184;
SYS_umask :: 95;
SYS_umount2 :: 166;
SYS_uname :: 63;
SYS_unlink :: 87;
SYS_unlinkat :: 263;
SYS_unshare :: 272;
SYS_uselib :: 134;
SYS_userfaultfd :: 323;
SYS_ustat :: 136;
SYS_utime :: 132;
SYS_utimensat :: 280;
SYS_utimes :: 235;
SYS_vfork :: 58;
SYS_vhangup :: 153;
SYS_vmsplice :: 278;
SYS_vserver :: 236;
SYS_wait4 :: 61;
SYS_waitid :: 247;
SYS_write :: 1;
SYS_writev :: 20;
S_BLKSIZE :: 512;
S_IEXEC :: 64;
S_IFBLK :: 24576;
S_IFCHR :: 8192;
S_IFIFO :: 4096;
S_IFSOCK :: 49152;
S_IREAD :: 256;
S_IRGRP :: 32;
S_IROTH :: 4;
S_IRUSR :: 256;
S_IRWXG :: 56;
S_IRWXO :: 7;
S_IRWXU :: 448;
S_ISGID :: 1024;
S_ISUID :: 2048;
S_ISVTX :: 512;
S_IWGRP :: 16;
S_IWOTH :: 2;
S_IWRITE :: 128;
S_IWUSR :: 128;
S_IXGRP :: 8;
S_IXOTH :: 1;
S_IXUSR :: 64;
TCFLSH :: 21515;
TCGETA :: 21509;
TCGETS :: 21505;
TCGETX :: 21554;
TCSBRK :: 21513;
TCSBRKP :: 21541;
TCSETA :: 21510;
TCSETAF :: 21512;
TCSETAW :: 21511;
TCSETS :: 21506;
TCSETSF :: 21508;
TCSETSW :: 21507;
TCSETX :: 21555;
TCSETXF :: 21556;
TCSETXW :: 21557;
TCXONC :: 21514;
TIMER_ABSTIME :: 1;
TIME_UTC :: 1;
TIOCCBRK :: 21544;
TIOCCONS :: 21533;
TIOCEXCL :: 21516;
TIOCGETD :: 21540;
TIOCGICOUNT :: 21597;
TIOCGLCKTRMIOS :: 21590;
TIOCGPGRP :: 21519;
TIOCGRS485 :: 21550;
TIOCGSERIAL :: 21534;
TIOCGSID :: 21545;
TIOCGSOFTCAR :: 21529;
TIOCGWINSZ :: 21523;
TIOCINQ :: 21531;
TIOCLINUX :: 21532;
TIOCMBIC :: 21527;
TIOCMBIS :: 21526;
TIOCMGET :: 21525;
TIOCMIWAIT :: 21596;
TIOCMSET :: 21528;
TIOCNOTTY :: 21538;
TIOCNXCL :: 21517;
TIOCOUTQ :: 21521;
TIOCPKT :: 21536;
TIOCPKT_DATA :: 0;
TIOCPKT_DOSTOP :: 32;
TIOCPKT_FLUSHREAD :: 1;
TIOCPKT_FLUSHWRITE :: 2;
TIOCPKT_IOCTL :: 64;
TIOCPKT_NOSTOP :: 16;
TIOCPKT_START :: 8;
TIOCPKT_STOP :: 4;
TIOCSBRK :: 21543;
TIOCSCTTY :: 21518;
TIOCSERCONFIG :: 21587;
TIOCSERGETLSR :: 21593;
TIOCSERGETMULTI :: 21594;
TIOCSERGSTRUCT :: 21592;
TIOCSERGWILD :: 21588;
TIOCSERSETMULTI :: 21595;
TIOCSERSWILD :: 21589;
TIOCSER_TEMT :: 1;
TIOCSETD :: 21539;
TIOCSLCKTRMIOS :: 21591;
TIOCSPGRP :: 21520;
TIOCSRS485 :: 21551;
TIOCSSERIAL :: 21535;
TIOCSSOFTCAR :: 21530;
TIOCSTI :: 21522;
TIOCSWINSZ :: 21524;
TIOCVHANGUP :: 21559;
UTIME_NOW :: 1073741823;
UTIME_OMIT :: 1073741822;
WAIT_ANY :: -1;
WAIT_MYPGRP :: 0;
WCONTINUED :: 8;
WEXITED :: 4;
WNOWAIT :: 16777216;
WSTOPPED :: 2;
W_OK :: 2;
X_OK :: 1;
_DIRENT_MATCHES_DIRENT64 :: 1;
_GNU_SOURCE :: 1;
_IOFBF :: 0;
_IOLBF :: 1;
_IONBF :: 2;
_NSIG :: 65;
_STRUCT_TIMESPEC :: 1;
_XOPEN_ENH_I18N :: 1;
_XOPEN_LEGACY :: 1;
_XOPEN_UNIX :: 1;
_XOPEN_VERSION :: 700;
_XOPEN_XCU_VERSION :: 4;
_XOPEN_XPG2 :: 1;
_XOPEN_XPG3 :: 1;
_XOPEN_XPG4 :: 1;
__BIT_TYPES_DEFINED__ :: 1;
__CPU_SETSIZE :: 1024;
__F_GETOWN :: 9;
__F_GETOWN_EX :: 16;
__F_GETSIG :: 11;
__F_SETOWN :: 8;
__F_SETOWN_EX :: 15;
__F_SETSIG :: 10;
__NGREG :: 23;
__O_CLOEXEC :: 524288;
__O_DIRECT :: 16384;
__O_DIRECTORY :: 65536;
__O_DSYNC :: 4096;
__O_LARGEFILE :: 0;
__O_NOATIME :: 262144;
__O_NOFOLLOW :: 131072;
__O_PATH :: 2097152;
__O_TMPFILE :: 4259840;
__POSIX2_THIS_VERSION :: 200809;
__POSIX_FADV_DONTNEED :: 4;
__POSIX_FADV_NOREUSE :: 5;
__PTHREAD_MUTEX_HAVE_PREV :: 1;
__SIGEV_MAX_SIZE :: 64;
__SIGRTMAX :: 64;
__SIGRTMIN :: 32;
__S_IEXEC :: 64;
__S_IFBLK :: 24576;
__S_IFCHR :: 8192;
__S_IFDIR :: 16384;
__S_IFIFO :: 4096;
__S_IFLNK :: 40960;
__S_IFMT :: 61440;
__S_IFREG :: 32768;
__S_IFSOCK :: 49152;
__S_IREAD :: 256;
__S_ISGID :: 1024;
__S_ISUID :: 2048;
__S_ISVTX :: 512;
__S_IWRITE :: 128;
__WALL :: 1073741824;
__WCLONE :: 2147483648;
__WNOTHREAD :: 536870912;
__error_t_defined :: 1;
__have_pthread_attr_t :: 1;
__rusage_defined :: 1;

// -------------------------------------------------------------- the types ---

__uint8_t :: u8;
__uint16_t :: u16;
__int32_t :: s32;
__uint32_t :: u32;
__uint64_t :: u64;
__dev_t :: u64;
__uid_t :: u32;
__gid_t :: u32;
__ino_t :: u64;
__ino64_t :: u64;
__mode_t :: u32;
__nlink_t :: u64;
__off_t :: s64;
__off64_t :: s64;
__pid_t :: s32;
__fsid_t :: struct {
    __val: [2] s32;
}

__clock_t :: s64;
__rlim_t :: u64;
__rlim64_t :: u64;
__id_t :: u32;
__time_t :: s64;
__useconds_t :: u32;
__suseconds_t :: s64;
__key_t :: s32;
__clockid_t :: s32;
__timer_t :: *void;
__blksize_t :: s64;
__blkcnt_t :: s64;
__blkcnt64_t :: s64;
__fsblkcnt_t :: u64;
__fsblkcnt64_t :: u64;
__fsfilcnt_t :: u64;
__fsfilcnt64_t :: u64;
__fsword_t :: s64;
__ssize_t :: s64;
__syscall_slong_t :: s64;
__syscall_ulong_t :: u64;
__intptr_t :: s64;
fsid_t :: __fsid_t;
ino_t :: __ino_t;
ino64_t :: __ino64_t;
dev_t :: __dev_t;
gid_t :: __gid_t;
mode_t :: __mode_t;
nlink_t :: __nlink_t;
uid_t :: __uid_t;
off_t :: __off_t;
off64_t :: __off64_t;
pid_t :: __pid_t;
id_t :: __id_t;
ssize_t :: __ssize_t;
key_t :: __key_t;
clock_t :: __clock_t;
timer_t :: __timer_t;
useconds_t :: __useconds_t;
suseconds_t :: __suseconds_t;
size_t :: u64;
int32_t :: __int32_t;
register_t :: s32;
__sigset_t :: struct {
    __val: [16] u64;
}

sigset_t :: __sigset_t;
timeval :: struct {
    tv_sec: __time_t;
    tv_usec: __suseconds_t;
}

blksize_t :: __blksize_t;
blkcnt_t :: __blkcnt_t;
fsblkcnt_t :: __fsblkcnt_t;
fsfilcnt_t :: __fsfilcnt_t;
blkcnt64_t :: __blkcnt64_t;
fsblkcnt64_t :: __fsblkcnt64_t;
fsfilcnt64_t :: __fsfilcnt64_t;
__atomic_wide_counter :: union {
    __value64: u64;
    __value32: struct {
        __low: u32;
        __high: u32;
    }
}

__pthread_internal_list :: struct {
    __prev: *__pthread_internal_list;
    __next: *__pthread_internal_list;
}

__pthread_list_t :: __pthread_internal_list;

__pthread_mutex_s :: struct {
    __lock: s32;
    __count: u32;
    __owner: s32;
    __nusers: u32;
    __kind: s32;
    __spins: s16;
    __elision: s16;
    __list: __pthread_list_t;
}

__pthread_rwlock_arch_t :: struct {
    __readers: u32;
    __writers: u32;
    __wrphase_futex: u32;
    __writers_futex: u32;
    __pad3: u32;
    __pad4: u32;
    __cur_writer: s32;
    __shared: s32;
    __rwelision: u8;
    __pad1: [7] u8;
    __pad2: u64;
    __flags: u32;
}

__pthread_cond_s :: struct {
    __wseq: __atomic_wide_counter;
    __g1_start: __atomic_wide_counter;
    __g_size: [2] u32;
    __g1_orig_size: u32;
    __wrefs: u32;
    __g_signals: [2] u32;
    __unused_initialized_1: u32;
    __unused_initialized_2: u32;
}

pthread_mutexattr_t :: union {
    __size: [4] u8;
    __align: s32;
}

pthread_condattr_t :: union {
    __size: [4] u8;
    __align: s32;
}

pthread_key_t :: u32;
pthread_once_t :: s32;
pthread_rwlock_t :: union {
    __data: __pthread_rwlock_arch_t;
    __size: [56] u8;
    __align: s64;
}

pthread_rwlockattr_t :: union {
    __size: [8] u8;
    __align: s64;
}

pthread_spinlock_t :: s32;
pthread_barrier_t :: union {
    __size: [32] u8;
    __align: s64;
}

pthread_barrierattr_t :: union {
    __size: [4] u8;
    __align: s32;
}

sigval :: union {
    sival_int: s32;
    sival_ptr: *void;
}

__sigval_t :: sigval;
sigevent :: struct {
    sigev_value: __sigval_t;
    sigev_signo: s32;
    sigev_notify: s32;
    _sigev_un: union {
        _pad: [12] s32;
        _tid: __pid_t;
        _sigev_thread: struct {
            _function: #type (__sigval_t) -> void #c_call;
            _attribute: *pthread_attr_t;
        }
    }
}

sigevent_t :: sigevent;

aiocb :: struct {
    aio_fildes: s32;
    aio_lio_opcode: s32;
    aio_reqprio: s32;
    aio_buf: *void;
    aio_nbytes: size_t;
    aio_sigevent: sigevent;
    __next_prio: *aiocb;
    __abs_prio: s32;
    __policy: s32;
    __error_code: s32;
    __return_value: __ssize_t;
    aio_offset: __off_t;
    __pad: [0] u8;
    __glibc_reserved: [32] u8;
}

aiocb64 :: struct {
    aio_fildes: s32;
    aio_lio_opcode: s32;
    aio_reqprio: s32;
    aio_buf: *void;
    aio_nbytes: size_t;
    aio_sigevent: sigevent;
    __next_prio: *aiocb;
    __abs_prio: s32;
    __policy: s32;
    __error_code: s32;
    __return_value: __ssize_t;
    aio_offset: __off64_t;
    __glibc_reserved: [32] u8;
}

aioinit :: struct {
    aio_threads: s32;
    aio_num: s32;
    aio_locks: s32;
    aio_usedba: s32;
    aio_debug: s32;
    aio_numusers: s32;
    aio_idle_time: s32;
    aio_reserved: s32;
}

dirent64 :: struct {
    d_ino: __ino64_t;
    d_off: __off64_t;
    d_reclen: u16;
    d_type: u8;
    d_name: [256] u8;
}

Lmid_t :: s64;
Dl_info :: struct {
    dli_fname: *u8;
    dli_fbase: *void;
    dli_sname: *u8;
    dli_saddr: *void;
}

Dl_serpath :: struct {
    dls_name: *u8;
    dls_flags: u32;
}

Dl_serinfo :: struct {
    dls_size: size_t;
    dls_cnt: u32;
    using _anonymous_2: union {
        dls_serpath: [0] Dl_serpath;
        __dls_serpath_pad: [1] Dl_serpath;
    }
}

error_t :: s32;
flock :: struct {
    l_type: s16;
    l_whence: s16;
    l_start: __off_t;
    l_len: __off_t;
    l_pid: __pid_t;
}

flock64 :: struct {
    l_type: s16;
    l_whence: s16;
    l_start: __off64_t;
    l_len: __off64_t;
    l_pid: __pid_t;
}

iovec :: struct {
    iov_base: *void;
    iov_len: size_t;
}

f_owner_ex :: struct {
    type: __pid_type;
    pid: __pid_t;
}

file_handle :: struct {
    handle_bytes: u32;
    handle_type: s32;
    f_handle: [0] u8;
}

stat64_t :: struct {
    st_dev: __dev_t;
    st_ino: __ino64_t;
    st_nlink: __nlink_t;
    st_mode: __mode_t;
    st_uid: __uid_t;
    st_gid: __gid_t;
    __pad0: s32;
    st_rdev: __dev_t;
    st_size: __off_t;
    st_blksize: __blksize_t;
    st_blocks: __blkcnt64_t;
    st_atim: timespec;
    st_mtim: timespec;
    st_ctim: timespec;
    __glibc_reserved: [3] __syscall_slong_t;
}

// Opaque: the library owns the contents.
_IO_FILE :: struct {}

FILE :: _IO_FILE;
__locale_struct :: struct {
    __locales: [13] *__locale_data;
    __ctype_b: *u16;
    __ctype_tolower: *s32;
    __ctype_toupper: *s32;
    __names: [13] *u8;
}

__locale_t :: *__locale_struct;
locale_t :: __locale_t;
nfds_t :: u64;
pollfd :: struct {
    fd: s32;
    events: s16;
    revents: s16;
}

__u16 :: u16;
__u32 :: u32;
__kernel_long_t :: s64;
__kernel_ulong_t :: u64;
sched_param :: struct {
    sched_priority: s32;
}

__cpu_mask :: u64;
cpu_set_t :: struct {
    __bits: [16] __cpu_mask;
}

timex :: struct {
    modes: u32;
    offset: __syscall_slong_t;
    freq: __syscall_slong_t;
    maxerror: __syscall_slong_t;
    esterror: __syscall_slong_t;
    status: s32;
    constant: __syscall_slong_t;
    precision: __syscall_slong_t;
    tolerance: __syscall_slong_t;
    time: timeval;
    tick: __syscall_slong_t;
    ppsfreq: __syscall_slong_t;
    jitter: __syscall_slong_t;
    shift: s32;
    stabil: __syscall_slong_t;
    jitcnt: __syscall_slong_t;
    calcnt: __syscall_slong_t;
    errcnt: __syscall_slong_t;
    stbcnt: __syscall_slong_t;
    tai: s32;
    // bitfield dropped: int : 32
    // bitfield dropped: int : 32
    // bitfield dropped: int : 32
    // bitfield dropped: int : 32
    // bitfield dropped: int : 32
    // bitfield dropped: int : 32
    // bitfield dropped: int : 32
    // bitfield dropped: int : 32
    // bitfield dropped: int : 32
    // bitfield dropped: int : 32
    // bitfield dropped: int : 32
}

itimerspec :: struct {
    it_interval: timespec;
    it_value: timespec;
}

__jmp_buf :: [8] s64;
__jmp_buf_tag :: struct {
    __jmpbuf: __jmp_buf;
    __mask_was_saved: s32;
    __saved_mask: __sigset_t;
}

_pthread_cleanup_buffer :: struct {
    __routine: #type (*void) -> void #c_call;
    __arg: *void;
    __canceltype: s32;
    __prev: *_pthread_cleanup_buffer;
}

__pthread_cleanup_frame :: struct {
    __cancel_routine: #type (*void) -> void #c_call;
    __cancel_arg: *void;
    __do_it: s32;
    __cancel_type: s32;
}

passwd :: struct {
    pw_name: *u8;
    pw_passwd: *u8;
    pw_uid: __uid_t;
    pw_gid: __gid_t;
    pw_gecos: *u8;
    pw_dir: *u8;
    pw_shell: *u8;
}

sem_t :: union {
    __size: [32] u8;
    __align: s64;
}

jmp_buf :: [1] __jmp_buf_tag;
sigjmp_buf :: [1] __jmp_buf_tag;
siginfo_t :: struct {
    si_signo: s32;
    si_errno: s32;
    si_code: s32;
    __pad0: s32;
    _sifields: union {
        _pad: [28] s32;
        _kill: struct {
            si_pid: __pid_t;
            si_uid: __uid_t;
        }
        _timer: struct {
            si_tid: s32;
            si_overrun: s32;
            si_sigval: __sigval_t;
        }
        _rt: struct {
            si_pid: __pid_t;
            si_uid: __uid_t;
            si_sigval: __sigval_t;
        }
        _sigchld: struct {
            si_pid: __pid_t;
            si_uid: __uid_t;
            si_status: s32;
            si_utime: __clock_t;
            si_stime: __clock_t;
        }
        _sigfault: struct {
            si_addr: *void;
            si_addr_lsb: s16;
            _bounds: union {
                _addr_bnd: struct {
                    _lower: *void;
                    _upper: *void;
                }
                _pkey: __uint32_t;
            }
        }
        _sigpoll: struct {
            si_band: s64;
            si_fd: s32;
        }
        _sigsys: struct {
            _call_addr: *void;
            _syscall: s32;
            _arch: u32;
        }
    }
}

__sighandler_t :: #type (s32) -> void #c_call;
sighandler_t :: __sighandler_t;
sig_t :: __sighandler_t;
sigaction_t :: struct {
    __sigaction_handler: union {
        sa_handler: __sighandler_t;
        sa_sigaction: #type (s32, *siginfo_t, *void) -> void #c_call;
    }
    sa_mask: __sigset_t;
    sa_flags: s32;
    sa_restorer: #type () -> void #c_call;
}

_fpxreg :: struct {
    significand: [4] u16;
    exponent: u16;
    __glibc_reserved1: [3] u16;
}

_xmmreg :: struct {
    element: [4] __uint32_t;
}

_fpstate :: struct {
    cwd: __uint16_t;
    swd: __uint16_t;
    ftw: __uint16_t;
    fop: __uint16_t;
    rip: __uint64_t;
    rdp: __uint64_t;
    mxcsr: __uint32_t;
    mxcr_mask: __uint32_t;
    _st: [8] _fpxreg;
    _xmm: [16] _xmmreg;
    __glibc_reserved1: [24] __uint32_t;
}

sigcontext :: struct {
    r8: __uint64_t;
    r9: __uint64_t;
    r10: __uint64_t;
    r11: __uint64_t;
    r12: __uint64_t;
    r13: __uint64_t;
    r14: __uint64_t;
    r15: __uint64_t;
    rdi: __uint64_t;
    rsi: __uint64_t;
    rbp: __uint64_t;
    rbx: __uint64_t;
    rdx: __uint64_t;
    rax: __uint64_t;
    rcx: __uint64_t;
    rsp: __uint64_t;
    rip: __uint64_t;
    eflags: __uint64_t;
    cs: u16;
    gs: u16;
    fs: u16;
    __pad0: u16;
    err: __uint64_t;
    trapno: __uint64_t;
    oldmask: __uint64_t;
    cr2: __uint64_t;
    using _anonymous_26: union {
        fpstate: *_fpstate;
        __fpstate_word: __uint64_t;
    }
    __reserved1: [8] __uint64_t;
}

stack_t :: struct {
    ss_sp: *void;
    ss_flags: s32;
    ss_size: size_t;
}

greg_t :: s64;
gregset_t :: [23] greg_t;
_libc_fpxreg :: struct {
    significand: [4] u16;
    exponent: u16;
    __glibc_reserved1: [3] u16;
}

_libc_xmmreg :: struct {
    element: [4] __uint32_t;
}

_libc_fpstate :: struct {
    cwd: __uint16_t;
    swd: __uint16_t;
    ftw: __uint16_t;
    fop: __uint16_t;
    rip: __uint64_t;
    rdp: __uint64_t;
    mxcsr: __uint32_t;
    mxcr_mask: __uint32_t;
    _st: [8] _libc_fpxreg;
    _xmm: [16] _libc_xmmreg;
    __glibc_reserved1: [24] __uint32_t;
}

fpregset_t :: *_libc_fpstate;
mcontext_t :: struct {
    gregs: gregset_t;
    fpregs: fpregset_t;
    __reserved1: [8] u64;
}

ucontext_t :: struct {
    uc_flags: u64;
    uc_link: *ucontext_t;
    uc_stack: stack_t;
    uc_mcontext: mcontext_t;
    uc_sigmask: sigset_t;
    __fpregs_mem: _libc_fpstate;
    __ssp: [4] u64;
}

intptr_t :: __intptr_t;
sigstack_t :: struct {
    ss_sp: *void;
    ss_onstack: s32;
}

__mbstate_t :: struct {
    __count: s32;
    __value: union {
        __wch: u32;
        __wchb: [4] u8;
    }
}

_G_fpos_t :: struct {
    __pos: __off_t;
    __state: __mbstate_t;
}

__fpos_t :: _G_fpos_t;

_G_fpos64_t :: struct {
    __pos: __off64_t;
    __state: __mbstate_t;
}

__fpos64_t :: _G_fpos64_t;

_IO_cookie_io_functions_t :: struct {
    read: *cookie_read_function_t;
    write: *cookie_write_function_t;
    seek: *cookie_seek_function_t;
    close: *cookie_close_function_t;
}

cookie_io_functions_t :: _IO_cookie_io_functions_t;

fpos_t :: __fpos_t;
fpos64_t :: __fpos64_t;
// Opaque: the library owns the contents.
obstack :: struct {}

uint8_t :: __uint8_t;
uint16_t :: __uint16_t;
uint32_t :: __uint32_t;
uint64_t :: __uint64_t;
eventfd_t :: uint64_t;
ipc_perm :: struct {
    __key: __key_t;
    uid: __uid_t;
    gid: __gid_t;
    cuid: __uid_t;
    cgid: __gid_t;
    mode: __mode_t;
    __seq: u16;
    __pad2: u16;
    __glibc_reserved1: __syscall_ulong_t;
    __glibc_reserved2: __syscall_ulong_t;
}

rlim_t :: __rlim_t;
rlim64_t :: __rlim64_t;
rlimit :: struct {
    rlim_cur: rlim_t;
    rlim_max: rlim_t;
}

rlimit64 :: struct {
    rlim_cur: rlim64_t;
    rlim_max: rlim64_t;
}

rusage :: struct {
    ru_utime: timeval;
    ru_stime: timeval;
    using _anonymous_2: union {
        ru_maxrss: s64;
        __ru_maxrss_word: __syscall_slong_t;
    }
    using _anonymous_6: union {
        ru_ixrss: s64;
        __ru_ixrss_word: __syscall_slong_t;
    }
    using _anonymous_10: union {
        ru_idrss: s64;
        __ru_idrss_word: __syscall_slong_t;
    }
    using _anonymous_14: union {
        ru_isrss: s64;
        __ru_isrss_word: __syscall_slong_t;
    }
    using _anonymous_18: union {
        ru_minflt: s64;
        __ru_minflt_word: __syscall_slong_t;
    }
    using _anonymous_22: union {
        ru_majflt: s64;
        __ru_majflt_word: __syscall_slong_t;
    }
    using _anonymous_26: union {
        ru_nswap: s64;
        __ru_nswap_word: __syscall_slong_t;
    }
    using _anonymous_30: union {
        ru_inblock: s64;
        __ru_inblock_word: __syscall_slong_t;
    }
    using _anonymous_34: union {
        ru_oublock: s64;
        __ru_oublock_word: __syscall_slong_t;
    }
    using _anonymous_38: union {
        ru_msgsnd: s64;
        __ru_msgsnd_word: __syscall_slong_t;
    }
    using _anonymous_42: union {
        ru_msgrcv: s64;
        __ru_msgrcv_word: __syscall_slong_t;
    }
    using _anonymous_46: union {
        ru_nsignals: s64;
        __ru_nsignals_word: __syscall_slong_t;
    }
    using _anonymous_50: union {
        ru_nvcsw: s64;
        __ru_nvcsw_word: __syscall_slong_t;
    }
    using _anonymous_54: union {
        ru_nivcsw: s64;
        __ru_nivcsw_word: __syscall_slong_t;
    }
}

__rlimit_resource_t :: __rlimit_resource;
__rusage_who_t :: __rusage_who;
__priority_which_t :: __priority_which;
shmatt_t :: __syscall_ulong_t;
shmid_ds :: struct {
    shm_perm: ipc_perm;
    shm_segsz: size_t;
    shm_atime: __time_t;
    shm_dtime: __time_t;
    shm_ctime: __time_t;
    shm_cpid: __pid_t;
    shm_lpid: __pid_t;
    shm_nattch: shmatt_t;
    __glibc_reserved5: __syscall_ulong_t;
    __glibc_reserved6: __syscall_ulong_t;
}

shminfo :: struct {
    shmmax: __syscall_ulong_t;
    shmmin: __syscall_ulong_t;
    shmmni: __syscall_ulong_t;
    shmseg: __syscall_ulong_t;
    shmall: __syscall_ulong_t;
    __glibc_reserved1: __syscall_ulong_t;
    __glibc_reserved2: __syscall_ulong_t;
    __glibc_reserved3: __syscall_ulong_t;
    __glibc_reserved4: __syscall_ulong_t;
}

shm_info :: struct {
    used_ids: s32;
    shm_tot: __syscall_ulong_t;
    shm_rss: __syscall_ulong_t;
    shm_swp: __syscall_ulong_t;
    swap_attempts: __syscall_ulong_t;
    swap_successes: __syscall_ulong_t;
}

signalfd_siginfo :: struct {
    ssi_signo: uint32_t;
    ssi_errno: int32_t;
    ssi_code: int32_t;
    ssi_pid: uint32_t;
    ssi_uid: uint32_t;
    ssi_fd: int32_t;
    ssi_tid: uint32_t;
    ssi_band: uint32_t;
    ssi_overrun: uint32_t;
    ssi_trapno: uint32_t;
    ssi_status: int32_t;
    ssi_int: int32_t;
    ssi_ptr: uint64_t;
    ssi_utime: uint64_t;
    ssi_stime: uint64_t;
    ssi_addr: uint64_t;
    ssi_addr_lsb: uint16_t;
    __pad2: uint16_t;
    ssi_syscall: int32_t;
    ssi_call_addr: uint64_t;
    ssi_arch: uint32_t;
    __pad: [28] uint8_t;
}

statvfs_t :: struct {
    f_bsize: u64;
    f_frsize: u64;
    f_blocks: __fsblkcnt_t;
    f_bfree: __fsblkcnt_t;
    f_bavail: __fsblkcnt_t;
    f_files: __fsfilcnt_t;
    f_ffree: __fsfilcnt_t;
    f_favail: __fsfilcnt_t;
    f_fsid: u64;
    f_flag: u64;
    f_namemax: u64;
    f_type: u32;
    __f_spare: [5] s32;
}

statvfs64_t :: struct {
    f_bsize: u64;
    f_frsize: u64;
    f_blocks: __fsblkcnt64_t;
    f_bfree: __fsblkcnt64_t;
    f_bavail: __fsblkcnt64_t;
    f_files: __fsfilcnt64_t;
    f_ffree: __fsfilcnt64_t;
    f_favail: __fsfilcnt64_t;
    f_fsid: u64;
    f_flag: u64;
    f_namemax: u64;
    f_type: u32;
    __f_spare: [5] s32;
}

sysinfo_t :: struct {
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

timezone :: struct {
    tz_minuteswest: s32;
    tz_dsttime: s32;
}

itimerval :: struct {
    it_interval: timeval;
    it_value: timeval;
}

__itimer_which_t :: __itimer_which;
utsname :: struct {
    sysname: [65] u8;
    nodename: [65] u8;
    release: [65] u8;
    version: [65] u8;
    machine: [65] u8;
    domainname: [65] u8;
}

if_nameindex_t :: struct {
    if_index: u32;
    if_name: *u8;
}

statfs_t :: struct {
    f_type: __fsword_t;
    f_bsize: __fsword_t;
    f_blocks: __fsblkcnt_t;
    f_bfree: __fsblkcnt_t;
    f_bavail: __fsblkcnt_t;
    f_files: __fsfilcnt_t;
    f_ffree: __fsfilcnt_t;
    f_fsid: __fsid_t;
    f_namelen: __fsword_t;
    f_frsize: __fsword_t;
    f_flags: __fsword_t;
    f_spare: [4] __fsword_t;
}

statfs64_t :: struct {
    f_type: __fsword_t;
    f_bsize: __fsword_t;
    f_blocks: __fsblkcnt64_t;
    f_bfree: __fsblkcnt64_t;
    f_bavail: __fsblkcnt64_t;
    f_files: __fsfilcnt64_t;
    f_ffree: __fsfilcnt64_t;
    f_fsid: __fsid_t;
    f_namelen: __fsword_t;
    f_frsize: __fsword_t;
    f_flags: __fsword_t;
    f_spare: [4] __fsword_t;
}


// ------------------------------------------------------------------ enums ---

AIO :: enum s32 {
    CANCELED;
    NOTCANCELED;
    ALLDONE;
}

LIO :: enum s32 {
    READ;
    WRITE;
    NOP;
}

LIO_1 :: enum s32 {
    WAIT;
    NOWAIT;
}

DT :: enum u8 {
    UNKNOWN :: 0;
    FIFO :: 1;
    CHR :: 2;
    DIR :: 4;
    BLK :: 6;
    REG :: 8;
    LNK :: 10;
    SOCK :: 12;
    WHT :: 14;
}

RTLD_DL :: enum s32 {
    SYMENT :: 1;
    LINKMAP :: 2;
}

RTLD_DI :: enum s32 {
    LMID :: 1;
    LINKMAP :: 2;
    CONFIGADDR :: 3;
    SERINFO :: 4;
    SERINFOSIZE :: 5;
    ORIGIN :: 6;
    PROFILENAME :: 7;
    PROFILEOUT :: 8;
    TLS_MODID :: 9;
    TLS_DATA :: 10;
    PHDR :: 11;
    MAX :: 11;
}

__pid_type :: enum s32 {
    F_OWNER_TID :: 0;
    F_OWNER_PID;
    F_OWNER_PGRP;
    F_OWNER_GID :: F_OWNER_PGRP;
}

PTHREAD_CREATE :: enum s32 {
    JOINABLE;
    DETACHED;
}

PTHREAD_MUTEX :: enum s32 {
    TIMED_NP;
    RECURSIVE_NP;
    ERRORCHECK_NP;
    ADAPTIVE_NP;
    NORMAL :: TIMED_NP;
    RECURSIVE :: RECURSIVE_NP;
    ERRORCHECK :: ERRORCHECK_NP;
    DEFAULT :: NORMAL;
    FAST_NP :: TIMED_NP;
}

PTHREAD_MUTEX_1 :: enum s32 {
    STALLED;
    STALLED_NP :: STALLED;
    ROBUST;
    ROBUST_NP :: ROBUST;
}

PTHREAD_PRIO :: enum s32 {
    NONE;
    INHERIT;
    PROTECT;
}

PTHREAD_RWLOCK :: enum s32 {
    PREFER_READER_NP;
    PREFER_WRITER_NP;
    PREFER_WRITER_NONRECURSIVE_NP;
    DEFAULT_NP :: PREFER_READER_NP;
}

PTHREAD :: enum s32 {
    INHERIT_SCHED;
    EXPLICIT_SCHED;
}

PTHREAD_SCOPE :: enum s32 {
    SYSTEM;
    PROCESS;
}

PTHREAD_PROCESS :: enum s32 {
    PRIVATE;
    SHARED;
}

PTHREAD_CANCEL :: enum s32 {
    ENABLE;
    DISABLE;
}

PTHREAD_CANCEL_1 :: enum s32 {
    DEFERRED;
    ASYNCHRONOUS;
}

REG :: enum s32 {
    R8 :: 0;
    R9;
    R10;
    R11;
    R12;
    R13;
    R14;
    R15;
    RDI;
    RSI;
    RBP;
    RBX;
    RDX;
    RAX;
    RCX;
    RSP;
    RIP;
    EFL;
    CSGSFS;
    ERR;
    TRAPNO;
    OLDMASK;
    CR2;
}

PC :: enum s32 {
    _PC_LINK_MAX;
    _PC_MAX_CANON;
    _PC_MAX_INPUT;
    _PC_NAME_MAX;
    _PC_PATH_MAX;
    _PC_PIPE_BUF;
    _PC_CHOWN_RESTRICTED;
    _PC_NO_TRUNC;
    _PC_VDISABLE;
    _PC_SYNC_IO;
    _PC_ASYNC_IO;
    _PC_PRIO_IO;
    _PC_SOCK_MAXBUF;
    _PC_FILESIZEBITS;
    _PC_REC_INCR_XFER_SIZE;
    _PC_REC_MAX_XFER_SIZE;
    _PC_REC_MIN_XFER_SIZE;
    _PC_REC_XFER_ALIGN;
    _PC_ALLOC_SIZE_MIN;
    _PC_SYMLINK_MAX;
    _PC_2_SYMLINKS;
}

using _SC_definitions :: enum s32 {
    _SC_ARG_MAX;
    _SC_CHILD_MAX;
    _SC_CLK_TCK;
    _SC_NGROUPS_MAX;
    _SC_OPEN_MAX;
    _SC_STREAM_MAX;
    _SC_TZNAME_MAX;
    _SC_JOB_CONTROL;
    _SC_SAVED_IDS;
    _SC_REALTIME_SIGNALS;
    _SC_PRIORITY_SCHEDULING;
    _SC_TIMERS;
    _SC_ASYNCHRONOUS_IO;
    _SC_PRIORITIZED_IO;
    _SC_SYNCHRONIZED_IO;
    _SC_FSYNC;
    _SC_MAPPED_FILES;
    _SC_MEMLOCK;
    _SC_MEMLOCK_RANGE;
    _SC_MEMORY_PROTECTION;
    _SC_MESSAGE_PASSING;
    _SC_SEMAPHORES;
    _SC_SHARED_MEMORY_OBJECTS;
    _SC_AIO_LISTIO_MAX;
    _SC_AIO_MAX;
    _SC_AIO_PRIO_DELTA_MAX;
    _SC_DELAYTIMER_MAX;
    _SC_MQ_OPEN_MAX;
    _SC_MQ_PRIO_MAX;
    _SC_VERSION;
    _SC_PAGESIZE;
    _SC_RTSIG_MAX;
    _SC_SEM_NSEMS_MAX;
    _SC_SEM_VALUE_MAX;
    _SC_SIGQUEUE_MAX;
    _SC_TIMER_MAX;
    _SC_BC_BASE_MAX;
    _SC_BC_DIM_MAX;
    _SC_BC_SCALE_MAX;
    _SC_BC_STRING_MAX;
    _SC_COLL_WEIGHTS_MAX;
    _SC_EQUIV_CLASS_MAX;
    _SC_EXPR_NEST_MAX;
    _SC_LINE_MAX;
    _SC_RE_DUP_MAX;
    _SC_CHARCLASS_NAME_MAX;
    _SC_2_VERSION;
    _SC_2_C_BIND;
    _SC_2_C_DEV;
    _SC_2_FORT_DEV;
    _SC_2_FORT_RUN;
    _SC_2_SW_DEV;
    _SC_2_LOCALEDEF;
    _SC_PII;
    _SC_PII_XTI;
    _SC_PII_SOCKET;
    _SC_PII_INTERNET;
    _SC_PII_OSI;
    _SC_POLL;
    _SC_SELECT;
    _SC_UIO_MAXIOV;
    _SC_IOV_MAX :: _SC_UIO_MAXIOV;
    _SC_PII_INTERNET_STREAM;
    _SC_PII_INTERNET_DGRAM;
    _SC_PII_OSI_COTS;
    _SC_PII_OSI_CLTS;
    _SC_PII_OSI_M;
    _SC_T_IOV_MAX;
    _SC_THREADS;
    _SC_THREAD_SAFE_FUNCTIONS;
    _SC_GETGR_R_SIZE_MAX;
    _SC_GETPW_R_SIZE_MAX;
    _SC_LOGIN_NAME_MAX;
    _SC_TTY_NAME_MAX;
    _SC_THREAD_DESTRUCTOR_ITERATIONS;
    _SC_THREAD_KEYS_MAX;
    _SC_THREAD_STACK_MIN;
    _SC_THREAD_THREADS_MAX;
    _SC_THREAD_ATTR_STACKADDR;
    _SC_THREAD_ATTR_STACKSIZE;
    _SC_THREAD_PRIORITY_SCHEDULING;
    _SC_THREAD_PRIO_INHERIT;
    _SC_THREAD_PRIO_PROTECT;
    _SC_THREAD_PROCESS_SHARED;
    _SC_NPROCESSORS_CONF;
    _SC_NPROCESSORS_ONLN;
    _SC_PHYS_PAGES;
    _SC_AVPHYS_PAGES;
    _SC_ATEXIT_MAX;
    _SC_PASS_MAX;
    _SC_XOPEN_VERSION;
    _SC_XOPEN_XCU_VERSION;
    _SC_XOPEN_UNIX;
    _SC_XOPEN_CRYPT;
    _SC_XOPEN_ENH_I18N;
    _SC_XOPEN_SHM;
    _SC_2_CHAR_TERM;
    _SC_2_C_VERSION;
    _SC_2_UPE;
    _SC_XOPEN_XPG2;
    _SC_XOPEN_XPG3;
    _SC_XOPEN_XPG4;
    _SC_CHAR_BIT;
    _SC_CHAR_MAX;
    _SC_CHAR_MIN;
    _SC_INT_MAX;
    _SC_INT_MIN;
    _SC_LONG_BIT;
    _SC_WORD_BIT;
    _SC_MB_LEN_MAX;
    _SC_NZERO;
    _SC_SSIZE_MAX;
    _SC_SCHAR_MAX;
    _SC_SCHAR_MIN;
    _SC_SHRT_MAX;
    _SC_SHRT_MIN;
    _SC_UCHAR_MAX;
    _SC_UINT_MAX;
    _SC_ULONG_MAX;
    _SC_USHRT_MAX;
    _SC_NL_ARGMAX;
    _SC_NL_LANGMAX;
    _SC_NL_MSGMAX;
    _SC_NL_NMAX;
    _SC_NL_SETMAX;
    _SC_NL_TEXTMAX;
    _SC_XBS5_ILP32_OFF32;
    _SC_XBS5_ILP32_OFFBIG;
    _SC_XBS5_LP64_OFF64;
    _SC_XBS5_LPBIG_OFFBIG;
    _SC_XOPEN_LEGACY;
    _SC_XOPEN_REALTIME;
    _SC_XOPEN_REALTIME_THREADS;
    _SC_ADVISORY_INFO;
    _SC_BARRIERS;
    _SC_BASE;
    _SC_C_LANG_SUPPORT;
    _SC_C_LANG_SUPPORT_R;
    _SC_CLOCK_SELECTION;
    _SC_CPUTIME;
    _SC_THREAD_CPUTIME;
    _SC_DEVICE_IO;
    _SC_DEVICE_SPECIFIC;
    _SC_DEVICE_SPECIFIC_R;
    _SC_FD_MGMT;
    _SC_FIFO;
    _SC_PIPE;
    _SC_FILE_ATTRIBUTES;
    _SC_FILE_LOCKING;
    _SC_FILE_SYSTEM;
    _SC_MONOTONIC_CLOCK;
    _SC_MULTI_PROCESS;
    _SC_SINGLE_PROCESS;
    _SC_NETWORKING;
    _SC_READER_WRITER_LOCKS;
    _SC_SPIN_LOCKS;
    _SC_REGEXP;
    _SC_REGEX_VERSION;
    _SC_SHELL;
    _SC_SIGNALS;
    _SC_SPAWN;
    _SC_SPORADIC_SERVER;
    _SC_THREAD_SPORADIC_SERVER;
    _SC_SYSTEM_DATABASE;
    _SC_SYSTEM_DATABASE_R;
    _SC_TIMEOUTS;
    _SC_TYPED_MEMORY_OBJECTS;
    _SC_USER_GROUPS;
    _SC_USER_GROUPS_R;
    _SC_2_PBS;
    _SC_2_PBS_ACCOUNTING;
    _SC_2_PBS_LOCATE;
    _SC_2_PBS_MESSAGE;
    _SC_2_PBS_TRACK;
    _SC_SYMLOOP_MAX;
    _SC_STREAMS;
    _SC_2_PBS_CHECKPOINT;
    _SC_V6_ILP32_OFF32;
    _SC_V6_ILP32_OFFBIG;
    _SC_V6_LP64_OFF64;
    _SC_V6_LPBIG_OFFBIG;
    _SC_HOST_NAME_MAX;
    _SC_TRACE;
    _SC_TRACE_EVENT_FILTER;
    _SC_TRACE_INHERIT;
    _SC_TRACE_LOG;
    _SC_LEVEL1_ICACHE_SIZE;
    _SC_LEVEL1_ICACHE_ASSOC;
    _SC_LEVEL1_ICACHE_LINESIZE;
    _SC_LEVEL1_DCACHE_SIZE;
    _SC_LEVEL1_DCACHE_ASSOC;
    _SC_LEVEL1_DCACHE_LINESIZE;
    _SC_LEVEL2_CACHE_SIZE;
    _SC_LEVEL2_CACHE_ASSOC;
    _SC_LEVEL2_CACHE_LINESIZE;
    _SC_LEVEL3_CACHE_SIZE;
    _SC_LEVEL3_CACHE_ASSOC;
    _SC_LEVEL3_CACHE_LINESIZE;
    _SC_LEVEL4_CACHE_SIZE;
    _SC_LEVEL4_CACHE_ASSOC;
    _SC_LEVEL4_CACHE_LINESIZE;
    _SC_IPV6 :: _SC_LEVEL1_ICACHE_SIZE + 50;
    _SC_RAW_SOCKETS;
    _SC_V7_ILP32_OFF32;
    _SC_V7_ILP32_OFFBIG;
    _SC_V7_LP64_OFF64;
    _SC_V7_LPBIG_OFFBIG;
    _SC_SS_REPL_MAX;
    _SC_TRACE_EVENT_NAME_MAX;
    _SC_TRACE_NAME_MAX;
    _SC_TRACE_SYS_MAX;
    _SC_TRACE_USER_EVENT_MAX;
    _SC_XOPEN_STREAMS;
    _SC_THREAD_ROBUST_PRIO_INHERIT;
    _SC_THREAD_ROBUST_PRIO_PROTECT;
    _SC_MINSIGSTKSZ;
    _SC_SIGSTKSZ;
}

CS :: enum s32 {
    _CS_PATH;
    _CS_V6_WIDTH_RESTRICTED_ENVS;
    _CS_GNU_LIBC_VERSION;
    _CS_GNU_LIBPTHREAD_VERSION;
    _CS_V5_WIDTH_RESTRICTED_ENVS;
    _CS_V7_WIDTH_RESTRICTED_ENVS;
    _CS_LFS_CFLAGS :: 1000;
    _CS_LFS_LDFLAGS;
    _CS_LFS_LIBS;
    _CS_LFS_LINTFLAGS;
    _CS_LFS64_CFLAGS;
    _CS_LFS64_LDFLAGS;
    _CS_LFS64_LIBS;
    _CS_LFS64_LINTFLAGS;
    _CS_XBS5_ILP32_OFF32_CFLAGS :: 1100;
    _CS_XBS5_ILP32_OFF32_LDFLAGS;
    _CS_XBS5_ILP32_OFF32_LIBS;
    _CS_XBS5_ILP32_OFF32_LINTFLAGS;
    _CS_XBS5_ILP32_OFFBIG_CFLAGS;
    _CS_XBS5_ILP32_OFFBIG_LDFLAGS;
    _CS_XBS5_ILP32_OFFBIG_LIBS;
    _CS_XBS5_ILP32_OFFBIG_LINTFLAGS;
    _CS_XBS5_LP64_OFF64_CFLAGS;
    _CS_XBS5_LP64_OFF64_LDFLAGS;
    _CS_XBS5_LP64_OFF64_LIBS;
    _CS_XBS5_LP64_OFF64_LINTFLAGS;
    _CS_XBS5_LPBIG_OFFBIG_CFLAGS;
    _CS_XBS5_LPBIG_OFFBIG_LDFLAGS;
    _CS_XBS5_LPBIG_OFFBIG_LIBS;
    _CS_XBS5_LPBIG_OFFBIG_LINTFLAGS;
    _CS_POSIX_V6_ILP32_OFF32_CFLAGS;
    _CS_POSIX_V6_ILP32_OFF32_LDFLAGS;
    _CS_POSIX_V6_ILP32_OFF32_LIBS;
    _CS_POSIX_V6_ILP32_OFF32_LINTFLAGS;
    _CS_POSIX_V6_ILP32_OFFBIG_CFLAGS;
    _CS_POSIX_V6_ILP32_OFFBIG_LDFLAGS;
    _CS_POSIX_V6_ILP32_OFFBIG_LIBS;
    _CS_POSIX_V6_ILP32_OFFBIG_LINTFLAGS;
    _CS_POSIX_V6_LP64_OFF64_CFLAGS;
    _CS_POSIX_V6_LP64_OFF64_LDFLAGS;
    _CS_POSIX_V6_LP64_OFF64_LIBS;
    _CS_POSIX_V6_LP64_OFF64_LINTFLAGS;
    _CS_POSIX_V6_LPBIG_OFFBIG_CFLAGS;
    _CS_POSIX_V6_LPBIG_OFFBIG_LDFLAGS;
    _CS_POSIX_V6_LPBIG_OFFBIG_LIBS;
    _CS_POSIX_V6_LPBIG_OFFBIG_LINTFLAGS;
    _CS_POSIX_V7_ILP32_OFF32_CFLAGS;
    _CS_POSIX_V7_ILP32_OFF32_LDFLAGS;
    _CS_POSIX_V7_ILP32_OFF32_LIBS;
    _CS_POSIX_V7_ILP32_OFF32_LINTFLAGS;
    _CS_POSIX_V7_ILP32_OFFBIG_CFLAGS;
    _CS_POSIX_V7_ILP32_OFFBIG_LDFLAGS;
    _CS_POSIX_V7_ILP32_OFFBIG_LIBS;
    _CS_POSIX_V7_ILP32_OFFBIG_LINTFLAGS;
    _CS_POSIX_V7_LP64_OFF64_CFLAGS;
    _CS_POSIX_V7_LP64_OFF64_LDFLAGS;
    _CS_POSIX_V7_LP64_OFF64_LIBS;
    _CS_POSIX_V7_LP64_OFF64_LINTFLAGS;
    _CS_POSIX_V7_LPBIG_OFFBIG_CFLAGS;
    _CS_POSIX_V7_LPBIG_OFFBIG_LDFLAGS;
    _CS_POSIX_V7_LPBIG_OFFBIG_LIBS;
    _CS_POSIX_V7_LPBIG_OFFBIG_LINTFLAGS;
    _CS_V6_ENV;
    _CS_V7_ENV;
}

EFD :: enum s32 {
    SEMAPHORE :: 1;
    CLOEXEC :: 524288;
    NONBLOCK :: 2048;
}

RLIMIT_RESOURCE :: enum s32 {
    RLIMIT_CPU :: 0;
    RLIMIT_FSIZE :: 1;
    RLIMIT_DATA :: 2;
    RLIMIT_STACK :: 3;
    RLIMIT_CORE :: 4;
    __RLIMIT_RSS :: 5;
    RLIMIT_NOFILE :: 7;
    __RLIMIT_OFILE :: RLIMIT_NOFILE;
    RLIMIT_AS :: 9;
    __RLIMIT_NPROC :: 6;
    __RLIMIT_MEMLOCK :: 8;
    __RLIMIT_LOCKS :: 10;
    __RLIMIT_SIGPENDING :: 11;
    __RLIMIT_MSGQUEUE :: 12;
    __RLIMIT_NICE :: 13;
    __RLIMIT_RTPRIO :: 14;
    __RLIMIT_RTTIME :: 15;
    __RLIMIT_NLIMITS :: 16;
    __RLIM_NLIMITS :: __RLIMIT_NLIMITS;
}

RUSAGE :: enum s32 {
    RUSAGE_SELF :: 0;
    RUSAGE_CHILDREN :: -1;
    RUSAGE_THREAD :: 1;
}

PRIO :: enum s32 {
    PRIO_PROCESS :: 0;
    PRIO_PGRP :: 1;
    PRIO_USER :: 2;
}

SFD :: enum s32 {
    CLOEXEC :: 524288;
    NONBLOCK :: 2048;
}

__itimer_which :: enum s32 {
    ITIMER_REAL :: 0;
    ITIMER_VIRTUAL :: 1;
    ITIMER_PROF :: 2;
}

TFD :: enum s32 {
    CLOEXEC :: 524288;
    NONBLOCK :: 2048;
}

TFD_TIMER :: enum s32 {
    ABSTIME :: 1 << 0;
    CANCEL_ON_SET :: 1 << 1;
}

idtype_t :: enum s32 {
    P_ALL;
    P_PID;
    P_PGID;
    P_PIDFD;
}


__rlimit_resource :: RLIMIT_RESOURCE;

__rusage_who :: RUSAGE;

__priority_which :: PRIO;

// ------------------------------------------------------------- procedures ---

__asprintf :: (__ptr: **u8, __fmt: *u8, __args: ..Any) -> s32 #foreign libc;
__getdelim :: (__lineptr: **u8, __n: *size_t, __delimiter: s32, __stream: *FILE) -> __ssize_t #foreign libc;
__getpagesize :: () -> s32 #foreign libc;
__getpgid :: (__pid: __pid_t) -> __pid_t #foreign libc;
__libc_current_sigrtmax :: () -> s32 #foreign libc;
__libc_current_sigrtmin :: () -> s32 #foreign libc;
__pthread_register_cancel :: (__buf: *__pthread_unwind_buf_t) -> void #foreign libc;
__pthread_register_cancel_defer :: (__buf: *__pthread_unwind_buf_t) -> void #foreign libc;
__pthread_unregister_cancel :: (__buf: *__pthread_unwind_buf_t) -> void #foreign libc;
__pthread_unregister_cancel_restore :: (__buf: *__pthread_unwind_buf_t) -> void #foreign libc;
__pthread_unwind_next :: (__buf: *__pthread_unwind_buf_t) -> void #foreign libc;
__sched_cpualloc :: (__count: size_t) -> *cpu_set_t #foreign libc;
__sched_cpucount :: (__setsize: size_t, __setp: *cpu_set_t) -> s32 #foreign libc;
__sched_cpufree :: (__set: *cpu_set_t) -> void #foreign libc;
__sigsetjmp :: (__env: *__jmp_buf_tag, __savemask: s32) -> s32 #foreign libc;
__sysv_signal :: (__sig: s32, __handler: __sighandler_t) -> __sighandler_t #foreign libc;
_dl_mcount_wrapper_check :: (__selfpc: *void) -> void #foreign libc;
_longjmp :: (__env: *__jmp_buf_tag, __val: s32) -> void #foreign libc;
_setjmp :: (__env: *__jmp_buf_tag) -> s32 #foreign libc;
access :: (__name: *u8, __type: s32) -> s32 #foreign libc;
acct :: (__name: *u8) -> s32 #foreign libc;
adjtime :: (__delta: *timeval, __olddelta: *timeval) -> s32 #foreign libc;
aio_cancel :: (__fildes: s32, __aiocbp: *aiocb) -> s32 #foreign libc;
aio_cancel64 :: (__fildes: s32, __aiocbp: *aiocb64) -> s32 #foreign libc;
aio_error :: (__aiocbp: *aiocb) -> s32 #foreign libc;
aio_error64 :: (__aiocbp: *aiocb64) -> s32 #foreign libc;
aio_fsync :: (__operation: s32, __aiocbp: *aiocb) -> s32 #foreign libc;
aio_fsync64 :: (__operation: s32, __aiocbp: *aiocb64) -> s32 #foreign libc;
aio_init :: (__init: *aioinit) -> void #foreign libc;
aio_read :: (__aiocbp: *aiocb) -> s32 #foreign libc;
aio_read64 :: (__aiocbp: *aiocb64) -> s32 #foreign libc;
aio_return :: (__aiocbp: *aiocb) -> __ssize_t #foreign libc;
aio_return64 :: (__aiocbp: *aiocb64) -> __ssize_t #foreign libc;
aio_suspend :: (__list: **aiocb, __nent: s32, __timeout: *timespec) -> s32 #foreign libc;
aio_suspend64 :: (__list: **aiocb64, __nent: s32, __timeout: *timespec) -> s32 #foreign libc;
aio_write :: (__aiocbp: *aiocb) -> s32 #foreign libc;
aio_write64 :: (__aiocbp: *aiocb64) -> s32 #foreign libc;
alarm :: (__seconds: u32) -> u32 #foreign libc;
alphasort :: (__e1: **dirent, __e2: **dirent) -> s32 #foreign libc;
alphasort64 :: (__e1: **dirent64, __e2: **dirent64) -> s32 #foreign libc;
asctime :: (__tp: *tm) -> *u8 #foreign libc;
asctime_r :: (__tp: *tm, __buf: *u8) -> *u8 #foreign libc;
asprintf :: (__ptr: **u8, __fmt: *u8, __args: ..Any) -> s32 #foreign libc;
backtrace :: (__array: **void, __size: s32) -> s32 #foreign libc;
backtrace_symbols :: (__array: **void, __size: s32) -> **u8 #foreign libc;
backtrace_symbols_fd :: (__array: **void, __size: s32, __fd: s32) -> void #foreign libc;
brk :: (__addr: *void) -> s32 #foreign libc;
chmod :: (__file: *u8, __mode: __mode_t) -> s32 #foreign libc;
chown :: (__file: *u8, __owner: __uid_t, __group: __gid_t) -> s32 #foreign libc;
chroot :: (__path: *u8) -> s32 #foreign libc;
clearerr :: (__stream: *FILE) -> void #foreign libc;
clearerr_unlocked :: (__stream: *FILE) -> void #foreign libc;
clock :: () -> clock_t #foreign libc;
clock_adjtime :: (__clock_id: __clockid_t, __utx: *timex) -> s32 #foreign libc;
clock_getcpuclockid :: (__pid: pid_t, __clock_id: *clockid_t) -> s32 #foreign libc;
clock_getres :: (__clock_id: clockid_t, __res: *timespec) -> s32 #foreign libc;
clock_nanosleep :: (__clock_id: clockid_t, __flags: s32, __req: *timespec, __rem: *timespec) -> s32 #foreign libc;
clock_settime :: (__clock_id: clockid_t, __tp: *timespec) -> s32 #foreign libc;
clone :: (__fn: #type (*void) -> s32 #c_call, __child_stack: *void, __flags: s32, __arg: *void, __args: ..Any) -> s32 #foreign libc;
confstr :: (__name: s32, __buf: *u8, __len: size_t) -> size_t #foreign libc;
copy_file_range :: (__infd: s32, __pinoff: *__off64_t, __outfd: s32, __poutoff: *__off64_t, __length: size_t, __flags: u32) -> ssize_t #foreign libc;
creat :: (__file: *u8, __mode: mode_t) -> s32 #foreign libc;
creat64 :: (__file: *u8, __mode: mode_t) -> s32 #foreign libc;
ctermid :: (__s: *u8) -> *u8 #foreign libc;
ctime :: (__timer: *time_t) -> *u8 #foreign libc;
ctime_r :: (__timer: *time_t, __buf: *u8) -> *u8 #foreign libc;
cuserid :: (__s: *u8) -> *u8 #foreign libc;
daemon :: (__nochdir: s32, __noclose: s32) -> s32 #foreign libc;
difftime :: (__time1: time_t, __time0: time_t) -> float64 #foreign libc;
dirfd :: (__dirp: *DIR) -> s32 #foreign libc;
dladdr :: (__address: *void, __info: *Dl_info) -> s32 #foreign libc;
dladdr1 :: (__address: *void, __info: *Dl_info, __extra_info: **void, __flags: s32) -> s32 #foreign libc;
dlclose :: (__handle: *void) -> s32 #foreign libc;
dlerror :: () -> *u8 #foreign libc;
dlinfo :: (__handle: *void, __request: s32, __arg: *void) -> s32 #foreign libc;
dlmopen :: (__nsid: Lmid_t, __file: *u8, __mode: s32) -> *void #foreign libc;
dlopen :: (__file: *u8, __mode: s32) -> *void #foreign libc;
dlsym :: (__handle: *void, __name: *u8) -> *void #foreign libc;
dlvsym :: (__handle: *void, __name: *u8, __version: *u8) -> *void #foreign libc;
dprintf :: (__fd: s32, __fmt: *u8, __args: ..Any) -> s32 #foreign libc;
dup :: (__fd: s32) -> s32 #foreign libc;
dup3 :: (__fd: s32, __fd2: s32, __flags: s32) -> s32 #foreign libc;
dysize :: (__year: s32) -> s32 #foreign libc;
eaccess :: (__name: *u8, __type: s32) -> s32 #foreign libc;
endpwent :: () -> void #foreign libc;
endusershell :: () -> void #foreign libc;
euidaccess :: (__name: *u8, __type: s32) -> s32 #foreign libc;
eventfd :: (__count: u32, __flags: s32) -> s32 #foreign libc;
eventfd_read :: (__fd: s32, __value: *eventfd_t) -> s32 #foreign libc;
eventfd_write :: (__fd: s32, __value: eventfd_t) -> s32 #foreign libc;
execl :: (__path: *u8, __arg: *u8, __args: ..Any) -> s32 #foreign libc;
execle :: (__path: *u8, __arg: *u8, __args: ..Any) -> s32 #foreign libc;
execlp :: (__file: *u8, __arg: *u8, __args: ..Any) -> s32 #foreign libc;
execv :: (__path: *u8, __argv: **u8) -> s32 #foreign libc;
execve :: (__path: *u8, __argv: **u8, __envp: **u8) -> s32 #foreign libc;
execvpe :: (__file: *u8, __argv: **u8, __envp: **u8) -> s32 #foreign libc;
faccessat :: (__fd: s32, __file: *u8, __type: s32, __flag: s32) -> s32 #foreign libc;
fallocate :: (__fd: s32, __mode: s32, __offset: __off_t, __len: __off_t) -> s32 #foreign libc;
fallocate64 :: (__fd: s32, __mode: s32, __offset: __off64_t, __len: __off64_t) -> s32 #foreign libc;
fchdir :: (__fd: s32) -> s32 #foreign libc;
fchmod :: (__fd: s32, __mode: __mode_t) -> s32 #foreign libc;
fchmodat :: (__fd: s32, __file: *u8, __mode: __mode_t, __flag: s32) -> s32 #foreign libc;
fchown :: (__fd: s32, __owner: __uid_t, __group: __gid_t) -> s32 #foreign libc;
fchownat :: (__fd: s32, __file: *u8, __owner: __uid_t, __group: __gid_t, __flag: s32) -> s32 #foreign libc;
fclose :: (__stream: *FILE) -> s32 #foreign libc;
fcloseall :: () -> s32 #foreign libc;
fdatasync :: (__fildes: s32) -> s32 #foreign libc;
fdopen :: (__fd: s32, __modes: *u8) -> *FILE #foreign libc;
fdopendir :: (__fd: s32) -> *DIR #foreign libc;
feof :: (__stream: *FILE) -> s32 #foreign libc;
feof_unlocked :: (__stream: *FILE) -> s32 #foreign libc;
ferror :: (__stream: *FILE) -> s32 #foreign libc;
ferror_unlocked :: (__stream: *FILE) -> s32 #foreign libc;
fexecve :: (__fd: s32, __argv: **u8, __envp: **u8) -> s32 #foreign libc;
fflush :: (__stream: *FILE) -> s32 #foreign libc;
fflush_unlocked :: (__stream: *FILE) -> s32 #foreign libc;
fgetc :: (__stream: *FILE) -> s32 #foreign libc;
fgetc_unlocked :: (__stream: *FILE) -> s32 #foreign libc;
fgetpos :: (__stream: *FILE, __pos: *fpos_t) -> s32 #foreign libc;
fgetpos64 :: (__stream: *FILE, __pos: *fpos64_t) -> s32 #foreign libc;
fgetpwent :: (__stream: *FILE) -> *passwd #foreign libc;
fgetpwent_r :: (__stream: *FILE, __resultbuf: *passwd, __buffer: *u8, __buflen: size_t, __result: **passwd) -> s32 #foreign libc;
fgets :: (__s: *u8, __n: s32, __stream: *FILE) -> *u8 #foreign libc;
fgets_unlocked :: (__s: *u8, __n: s32, __stream: *FILE) -> *u8 #foreign libc;
fileno :: (__stream: *FILE) -> s32 #foreign libc;
fileno_unlocked :: (__stream: *FILE) -> s32 #foreign libc;
flockfile :: (__stream: *FILE) -> void #foreign libc;
fmemopen :: (__s: *void, __len: size_t, __modes: *u8) -> *FILE #foreign libc;
fnmatch :: (__pattern: *u8, __name: *u8, __flags: s32) -> s32 #foreign libc;
fopen :: (__filename: *u8, __modes: *u8) -> *FILE #foreign libc;
fopen64 :: (__filename: *u8, __modes: *u8) -> *FILE #foreign libc;
fopencookie :: (__magic_cookie: *void, __modes: *u8, __io_funcs: cookie_io_functions_t) -> *FILE #foreign libc;
fpathconf :: (__fd: s32, __name: s32) -> s64 #foreign libc;
fprintf :: (__stream: *FILE, __format: *u8, __args: ..Any) -> s32 #foreign libc;
fputc :: (__c: s32, __stream: *FILE) -> s32 #foreign libc;
fputc_unlocked :: (__c: s32, __stream: *FILE) -> s32 #foreign libc;
fputs :: (__s: *u8, __stream: *FILE) -> s32 #foreign libc;
fputs_unlocked :: (__s: *u8, __stream: *FILE) -> s32 #foreign libc;
fread :: (__ptr: *void, __size: size_t, __n: size_t, __stream: *FILE) -> u64 #foreign libc;
fread_unlocked :: (__ptr: *void, __size: size_t, __n: size_t, __stream: *FILE) -> size_t #foreign libc;
freopen :: (__filename: *u8, __modes: *u8, __stream: *FILE) -> *FILE #foreign libc;
freopen64 :: (__filename: *u8, __modes: *u8, __stream: *FILE) -> *FILE #foreign libc;
fscanf :: (__stream: *FILE, __format: *u8, __args: ..Any) -> s32 #foreign libc;
fseek :: (__stream: *FILE, __off: s64, __whence: s32) -> s32 #foreign libc;
fseeko :: (__stream: *FILE, __off: __off_t, __whence: s32) -> s32 #foreign libc;
fseeko64 :: (__stream: *FILE, __off: __off64_t, __whence: s32) -> s32 #foreign libc;
fsetpos :: (__stream: *FILE, __pos: *fpos_t) -> s32 #foreign libc;
fsetpos64 :: (__stream: *FILE, __pos: *fpos64_t) -> s32 #foreign libc;
ftell :: (__stream: *FILE) -> s64 #foreign libc;
ftello :: (__stream: *FILE) -> __off_t #foreign libc;
ftello64 :: (__stream: *FILE) -> __off64_t #foreign libc;
ftok :: (__pathname: *u8, __proj_id: s32) -> key_t #foreign libc;
ftruncate64 :: (__fd: s32, __length: __off64_t) -> s32 #foreign libc;
ftrylockfile :: (__stream: *FILE) -> s32 #foreign libc;
funlockfile :: (__stream: *FILE) -> void #foreign libc;
futimens :: (__fd: s32, __times: *timespec) -> s32 #foreign libc;
futimes :: (__fd: s32, __tvp: *timeval) -> s32 #foreign libc;
futimesat :: (__fd: s32, __file: *u8, __tvp: *timeval) -> s32 #foreign libc;
fwrite :: (__ptr: *void, __size: size_t, __n: size_t, __s: *FILE) -> u64 #foreign libc;
fwrite_unlocked :: (__ptr: *void, __size: size_t, __n: size_t, __stream: *FILE) -> size_t #foreign libc;
get_current_dir_name :: () -> *u8 #foreign libc;
getc :: (__stream: *FILE) -> s32 #foreign libc;
getc_unlocked :: (__stream: *FILE) -> s32 #foreign libc;
getchar :: () -> s32 #foreign libc;
getchar_unlocked :: () -> s32 #foreign libc;
getdate :: (__string: *u8) -> *tm #foreign libc;
getdate_r :: (__string: *u8, __resbufp: *tm) -> s32 #foreign libc;
getdelim :: (__lineptr: **u8, __n: *size_t, __delimiter: s32, __stream: *FILE) -> __ssize_t #foreign libc;
getdirentries :: (__fd: s32, __buf: *u8, __nbytes: size_t, __basep: *__off_t) -> __ssize_t #foreign libc;
getdirentries64 :: (__fd: s32, __buf: *u8, __nbytes: size_t, __basep: *__off64_t) -> __ssize_t #foreign libc;
getdomainname :: (__name: *u8, __len: size_t) -> s32 #foreign libc;
getdtablesize :: () -> s32 #foreign libc;
getegid :: () -> __gid_t #foreign libc;
getentropy :: (__buffer: *void, __length: size_t) -> s32 #foreign libc;
geteuid :: () -> __uid_t #foreign libc;
getgid :: () -> __gid_t #foreign libc;
getgroups :: (__size: s32, __list: *__gid_t) -> s32 #foreign libc;
gethostid :: () -> s64 #foreign libc;
gethostname :: (__name: *u8, __len: size_t) -> s32 #foreign libc;
getitimer :: (__which: __itimer_which_t, __value: *itimerval) -> s32 #foreign libc;
getline :: (__lineptr: **u8, __n: *size_t, __stream: *FILE) -> __ssize_t #foreign libc;
getlogin :: () -> *u8 #foreign libc;
getlogin_r :: (__name: *u8, __name_len: size_t) -> s32 #foreign libc;
getpagesize :: () -> s32 #foreign libc;
getpass :: (__prompt: *u8) -> *u8 #foreign libc;
getpgid :: (__pid: __pid_t) -> __pid_t #foreign libc;
getpgrp :: () -> __pid_t #foreign libc;
getppid :: () -> __pid_t #foreign libc;
getpriority :: (__which: __priority_which_t, __who: id_t) -> s32 #foreign libc;
getpw :: (__uid: __uid_t, __buffer: *u8) -> s32 #foreign libc;
getpwent :: () -> *passwd #foreign libc;
getpwent_r :: (__resultbuf: *passwd, __buffer: *u8, __buflen: size_t, __result: **passwd) -> s32 #foreign libc;
getpwnam :: (__name: *u8) -> *passwd #foreign libc;
getpwnam_r :: (__name: *u8, __resultbuf: *passwd, __buffer: *u8, __buflen: size_t, __result: **passwd) -> s32 #foreign libc;
getpwuid :: (__uid: __uid_t) -> *passwd #foreign libc;
getpwuid_r :: (__uid: __uid_t, __resultbuf: *passwd, __buffer: *u8, __buflen: size_t, __result: **passwd) -> s32 #foreign libc;
getresgid :: (__rgid: *__gid_t, __egid: *__gid_t, __sgid: *__gid_t) -> s32 #foreign libc;
getresuid :: (__ruid: *__uid_t, __euid: *__uid_t, __suid: *__uid_t) -> s32 #foreign libc;
getrlimit :: (__resource: __rlimit_resource_t, __rlimits: *rlimit) -> s32 #foreign libc;
getrlimit64 :: (__resource: __rlimit_resource_t, __rlimits: *rlimit64) -> s32 #foreign libc;
getrusage :: (__who: __rusage_who_t, __usage: *rusage) -> s32 #foreign libc;
getsid :: (__pid: __pid_t) -> __pid_t #foreign libc;
gettimeofday :: (__tv: *timeval, __tz: *void) -> s32 #foreign libc;
getuid :: () -> __uid_t #foreign libc;
getusershell :: () -> *u8 #foreign libc;
getw :: (__stream: *FILE) -> s32 #foreign libc;
getwd :: (__buf: *u8) -> *u8 #foreign libc;
gmtime :: (__timer: *time_t) -> *tm #foreign libc;
group_member :: (__gid: __gid_t) -> s32 #foreign libc;
gsignal :: (__sig: s32) -> s32 #foreign libc;
ioctl :: (__fd: s32, __request: u64, __args: ..Any) -> s32 #foreign libc;
isatty :: (__fd: s32) -> s32 #foreign libc;
killpg :: (__pgrp: __pid_t, __sig: s32) -> s32 #foreign libc;
lchmod :: (__file: *u8, __mode: __mode_t) -> s32 #foreign libc;
lchown :: (__file: *u8, __owner: __uid_t, __group: __gid_t) -> s32 #foreign libc;
link :: (__from: *u8, __to: *u8) -> s32 #foreign libc;
linkat :: (__fromfd: s32, __from: *u8, __tofd: s32, __to: *u8, __flags: s32) -> s32 #foreign libc;
lio_listio :: (__mode: s32, __list: **aiocb, __nent: s32, __sig: *sigevent) -> s32 #foreign libc;
lio_listio64 :: (__mode: s32, __list: **aiocb64, __nent: s32, __sig: *sigevent) -> s32 #foreign libc;
localtime :: (__timer: *time_t) -> *tm #foreign libc;
lockf :: (__fd: s32, __cmd: s32, __len: off_t) -> s32 #foreign libc;
lockf64 :: (__fd: s32, __cmd: s32, __len: off64_t) -> s32 #foreign libc;
longjmp :: (__env: *__jmp_buf_tag, __val: s32) -> void #foreign libc;
lseek64 :: (__fd: s32, __offset: __off64_t, __whence: s32) -> __off64_t #foreign libc;
lutimes :: (__file: *u8, __tvp: *timeval) -> s32 #foreign libc;
madvise :: (__addr: *void, __len: size_t, __advice: s32) -> s32 #foreign libc;
mincore :: (__start: *void, __len: size_t, __vec: *u8) -> s32 #foreign libc;
mkdirat :: (__fd: s32, __path: *u8, __mode: __mode_t) -> s32 #foreign libc;
mkdtemp :: (__template: *u8) -> *u8 #foreign libc;
mkfifo :: (__path: *u8, __mode: __mode_t) -> s32 #foreign libc;
mkfifoat :: (__fd: s32, __path: *u8, __mode: __mode_t) -> s32 #foreign libc;
mkostemp :: (__template: *u8, __flags: s32) -> s32 #foreign libc;
mkostemps :: (__template: *u8, __suffixlen: s32, __flags: s32) -> s32 #foreign libc;
mkstemp :: (__template: *u8) -> s32 #foreign libc;
mkstemps :: (__template: *u8, __suffixlen: s32) -> s32 #foreign libc;
mktemp :: (__template: *u8) -> *u8 #foreign libc;
mktime :: (__tp: *tm) -> time_t #foreign libc;
mlock :: (__addr: *void, __len: size_t) -> s32 #foreign libc;
mlockall :: (__flags: s32) -> s32 #foreign libc;
mmap64 :: (__addr: *void, __len: size_t, __prot: s32, __flags: s32, __fd: s32, __offset: __off64_t) -> *void #foreign libc;
mprotect :: (__addr: *void, __len: size_t, __prot: s32) -> s32 #foreign libc;
mremap :: (__addr: *void, __old_len: size_t, __new_len: size_t, __flags: s32, __args: ..Any) -> *void #foreign libc;
msync :: (__addr: *void, __len: size_t, __flags: s32) -> s32 #foreign libc;
munlock :: (__addr: *void, __len: size_t) -> s32 #foreign libc;
munlockall :: () -> s32 #foreign libc;
name_to_handle_at :: (__dfd: s32, __name: *u8, __handle: *file_handle, __mnt_id: *s32, __flags: s32) -> s32 #foreign libc;
nice :: (__inc: s32) -> s32 #foreign libc;
obstack_printf :: (__obstack: *obstack, __format: *u8, __args: ..Any) -> s32 #foreign libc;
open64 :: (__file: *u8, __oflag: s32, __args: ..Any) -> s32 #foreign libc;
open_by_handle_at :: (__mountdirfd: s32, __handle: *file_handle, __flags: s32) -> s32 #foreign libc;
open_memstream :: (__bufloc: **u8, __sizeloc: *size_t) -> *FILE #foreign libc;
openat :: (__fd: s32, __file: *u8, __oflag: s32, __args: ..Any) -> s32 #foreign libc;
openat64 :: (__fd: s32, __file: *u8, __oflag: s32, __args: ..Any) -> s32 #foreign libc;
pathconf :: (__path: *u8, __name: s32) -> s64 #foreign libc;
pclose :: (__stream: *FILE) -> s32 #foreign libc;
perror :: (__s: *u8) -> void #foreign libc;
pipe2 :: (__pipedes: *s32, __flags: s32) -> s32 #foreign libc;
poll :: (__fds: *pollfd, __nfds: nfds_t, __timeout: s32) -> s32 #foreign libc;
popen :: (__command: *u8, __modes: *u8) -> *FILE #foreign libc;
posix_fadvise :: (__fd: s32, __offset: off_t, __len: off_t, __advise: s32) -> s32 #foreign libc;
posix_fadvise64 :: (__fd: s32, __offset: off64_t, __len: off64_t, __advise: s32) -> s32 #foreign libc;
posix_fallocate :: (__fd: s32, __offset: off_t, __len: off_t) -> s32 #foreign libc;
posix_fallocate64 :: (__fd: s32, __offset: off64_t, __len: off64_t) -> s32 #foreign libc;
posix_madvise :: (__addr: *void, __len: size_t, __advice: s32) -> s32 #foreign libc;
ppoll :: (__fds: *pollfd, __nfds: nfds_t, __timeout: *timespec, __ss: *__sigset_t) -> s32 #foreign libc;
pread :: (__fd: s32, __buf: *void, __nbytes: size_t, __offset: __off_t) -> ssize_t #foreign libc;
pread64 :: (__fd: s32, __buf: *void, __nbytes: size_t, __offset: __off64_t) -> ssize_t #foreign libc;
preadv :: (__fd: s32, __iovec: *iovec, __count: s32, __offset: __off_t) -> ssize_t #foreign libc;
preadv2 :: (__fp: s32, __iovec: *iovec, __count: s32, __offset: __off_t, ___flags: s32) -> ssize_t #foreign libc;
preadv64 :: (__fd: s32, __iovec: *iovec, __count: s32, __offset: __off64_t) -> ssize_t #foreign libc;
preadv64v2 :: (__fp: s32, __iovec: *iovec, __count: s32, __offset: __off64_t, ___flags: s32) -> ssize_t #foreign libc;
printf :: (__format: *u8, __args: ..Any) -> s32 #foreign libc;
prlimit :: (__pid: __pid_t, __resource: __rlimit_resource, __new_limit: *rlimit, __old_limit: *rlimit) -> s32 #foreign libc;
prlimit64 :: (__pid: __pid_t, __resource: __rlimit_resource, __new_limit: *rlimit64, __old_limit: *rlimit64) -> s32 #foreign libc;
profil :: (__sample_buffer: *u16, __size: size_t, __offset: size_t, __scale: u32) -> s32 #foreign libc;
psiginfo :: (__pinfo: *siginfo_t, __s: *u8) -> void #foreign libc;
psignal :: (__sig: s32, __s: *u8) -> void #foreign libc;
pthread_attr_destroy :: (__attr: *pthread_attr_t) -> s32 #foreign libc;
pthread_attr_getaffinity_np :: (__attr: *pthread_attr_t, __cpusetsize: size_t, __cpuset: *cpu_set_t) -> s32 #foreign libc;
pthread_attr_getdetachstate :: (__attr: *pthread_attr_t, __detachstate: *s32) -> s32 #foreign libc;
pthread_attr_getguardsize :: (__attr: *pthread_attr_t, __guardsize: *size_t) -> s32 #foreign libc;
pthread_attr_getinheritsched :: (__attr: *pthread_attr_t, __inherit: *s32) -> s32 #foreign libc;
pthread_attr_getschedparam :: (__attr: *pthread_attr_t, __param: *sched_param) -> s32 #foreign libc;
pthread_attr_getschedpolicy :: (__attr: *pthread_attr_t, __policy: *s32) -> s32 #foreign libc;
pthread_attr_getscope :: (__attr: *pthread_attr_t, __scope: *s32) -> s32 #foreign libc;
pthread_attr_getstack :: (__attr: *pthread_attr_t, __stackaddr: **void, __stacksize: *size_t) -> s32 #foreign libc;
pthread_attr_getstackaddr :: (__attr: *pthread_attr_t, __stackaddr: **void) -> s32 #foreign libc;
pthread_attr_getstacksize :: (__attr: *pthread_attr_t, __stacksize: *size_t) -> s32 #foreign libc;
pthread_attr_init :: (__attr: *pthread_attr_t) -> s32 #foreign libc;
pthread_attr_setaffinity_np :: (__attr: *pthread_attr_t, __cpusetsize: size_t, __cpuset: *cpu_set_t) -> s32 #foreign libc;
pthread_attr_setdetachstate :: (__attr: *pthread_attr_t, __detachstate: s32) -> s32 #foreign libc;
pthread_attr_setguardsize :: (__attr: *pthread_attr_t, __guardsize: size_t) -> s32 #foreign libc;
pthread_attr_setinheritsched :: (__attr: *pthread_attr_t, __inherit: s32) -> s32 #foreign libc;
pthread_attr_setschedparam :: (__attr: *pthread_attr_t, __param: *sched_param) -> s32 #foreign libc;
pthread_attr_setschedpolicy :: (__attr: *pthread_attr_t, __policy: s32) -> s32 #foreign libc;
pthread_attr_setscope :: (__attr: *pthread_attr_t, __scope: s32) -> s32 #foreign libc;
pthread_attr_setstack :: (__attr: *pthread_attr_t, __stackaddr: *void, __stacksize: size_t) -> s32 #foreign libc;
pthread_attr_setstackaddr :: (__attr: *pthread_attr_t, __stackaddr: *void) -> s32 #foreign libc;
pthread_attr_setstacksize :: (__attr: *pthread_attr_t, __stacksize: size_t) -> s32 #foreign libc;
pthread_barrier_destroy :: (__barrier: *pthread_barrier_t) -> s32 #foreign libc;
pthread_barrier_init :: (__barrier: *pthread_barrier_t, __attr: *pthread_barrierattr_t, __count: u32) -> s32 #foreign libc;
pthread_barrier_wait :: (__barrier: *pthread_barrier_t) -> s32 #foreign libc;
pthread_barrierattr_destroy :: (__attr: *pthread_barrierattr_t) -> s32 #foreign libc;
pthread_barrierattr_getpshared :: (__attr: *pthread_barrierattr_t, __pshared: *s32) -> s32 #foreign libc;
pthread_barrierattr_init :: (__attr: *pthread_barrierattr_t) -> s32 #foreign libc;
pthread_barrierattr_setpshared :: (__attr: *pthread_barrierattr_t, __pshared: s32) -> s32 #foreign libc;
pthread_cancel :: (__th: pthread_t) -> s32 #foreign libc;
pthread_cond_timedwait :: (__cond: *pthread_cond_t, __mutex: *pthread_mutex_t, __abstime: *timespec) -> s32 #foreign libc;
pthread_condattr_destroy :: (__attr: *pthread_condattr_t) -> s32 #foreign libc;
pthread_condattr_getclock :: (__attr: *pthread_condattr_t, __clock_id: *__clockid_t) -> s32 #foreign libc;
pthread_condattr_getpshared :: (__attr: *pthread_condattr_t, __pshared: *s32) -> s32 #foreign libc;
pthread_condattr_init :: (__attr: *pthread_condattr_t) -> s32 #foreign libc;
pthread_condattr_setclock :: (__attr: *pthread_condattr_t, __clock_id: __clockid_t) -> s32 #foreign libc;
pthread_condattr_setpshared :: (__attr: *pthread_condattr_t, __pshared: s32) -> s32 #foreign libc;
pthread_equal :: (__thread1: pthread_t, __thread2: pthread_t) -> s32 #foreign libc;
pthread_exit :: (__retval: *void) -> void #foreign libc;
pthread_getaffinity_np :: (__th: pthread_t, __cpusetsize: size_t, __cpuset: *cpu_set_t) -> s32 #foreign libc;
pthread_getattr_default_np :: (__attr: *pthread_attr_t) -> s32 #foreign libc;
pthread_getattr_np :: (__th: pthread_t, __attr: *pthread_attr_t) -> s32 #foreign libc;
pthread_getconcurrency :: () -> s32 #foreign libc;
pthread_getcpuclockid :: (__thread_id: pthread_t, __clock_id: *__clockid_t) -> s32 #foreign libc;
pthread_getname_np :: (__target_thread: pthread_t, __buf: *u8, __buflen: size_t) -> s32 #foreign libc;
pthread_getschedparam :: (__target_thread: pthread_t, __policy: *s32, __param: *sched_param) -> s32 #foreign libc;
pthread_getspecific :: (__key: pthread_key_t) -> *void #foreign libc;
pthread_key_create :: (__key: *pthread_key_t, __destr_function: #type (*void) -> void #c_call) -> s32 #foreign libc;
pthread_key_delete :: (__key: pthread_key_t) -> s32 #foreign libc;
pthread_mutex_consistent :: (__mutex: *pthread_mutex_t) -> s32 #foreign libc;
pthread_mutex_getprioceiling :: (__mutex: *pthread_mutex_t, __prioceiling: *s32) -> s32 #foreign libc;
pthread_mutex_setprioceiling :: (__mutex: *pthread_mutex_t, __prioceiling: s32, __old_ceiling: *s32) -> s32 #foreign libc;
pthread_mutex_timedlock :: (__mutex: *pthread_mutex_t, __abstime: *timespec) -> s32 #foreign libc;
pthread_mutexattr_destroy :: (__attr: *pthread_mutexattr_t) -> s32 #foreign libc;
pthread_mutexattr_getprioceiling :: (__attr: *pthread_mutexattr_t, __prioceiling: *s32) -> s32 #foreign libc;
pthread_mutexattr_getprotocol :: (__attr: *pthread_mutexattr_t, __protocol: *s32) -> s32 #foreign libc;
pthread_mutexattr_getpshared :: (__attr: *pthread_mutexattr_t, __pshared: *s32) -> s32 #foreign libc;
pthread_mutexattr_getrobust :: (__attr: *pthread_mutexattr_t, __robustness: *s32) -> s32 #foreign libc;
pthread_mutexattr_gettype :: (__attr: *pthread_mutexattr_t, __kind: *s32) -> s32 #foreign libc;
pthread_mutexattr_init :: (__attr: *pthread_mutexattr_t) -> s32 #foreign libc;
pthread_mutexattr_setprioceiling :: (__attr: *pthread_mutexattr_t, __prioceiling: s32) -> s32 #foreign libc;
pthread_mutexattr_setprotocol :: (__attr: *pthread_mutexattr_t, __protocol: s32) -> s32 #foreign libc;
pthread_mutexattr_setpshared :: (__attr: *pthread_mutexattr_t, __pshared: s32) -> s32 #foreign libc;
pthread_mutexattr_setrobust :: (__attr: *pthread_mutexattr_t, __robustness: s32) -> s32 #foreign libc;
pthread_mutexattr_settype :: (__attr: *pthread_mutexattr_t, __kind: s32) -> s32 #foreign libc;
pthread_once :: (__once_control: *pthread_once_t, __init_routine: #type () -> void #c_call) -> s32 #foreign libc;
pthread_rwlock_destroy :: (__rwlock: *pthread_rwlock_t) -> s32 #foreign libc;
pthread_rwlock_init :: (__rwlock: *pthread_rwlock_t, __attr: *pthread_rwlockattr_t) -> s32 #foreign libc;
pthread_rwlock_rdlock :: (__rwlock: *pthread_rwlock_t) -> s32 #foreign libc;
pthread_rwlock_timedrdlock :: (__rwlock: *pthread_rwlock_t, __abstime: *timespec) -> s32 #foreign libc;
pthread_rwlock_timedwrlock :: (__rwlock: *pthread_rwlock_t, __abstime: *timespec) -> s32 #foreign libc;
pthread_rwlock_tryrdlock :: (__rwlock: *pthread_rwlock_t) -> s32 #foreign libc;
pthread_rwlock_trywrlock :: (__rwlock: *pthread_rwlock_t) -> s32 #foreign libc;
pthread_rwlock_unlock :: (__rwlock: *pthread_rwlock_t) -> s32 #foreign libc;
pthread_rwlock_wrlock :: (__rwlock: *pthread_rwlock_t) -> s32 #foreign libc;
pthread_rwlockattr_destroy :: (__attr: *pthread_rwlockattr_t) -> s32 #foreign libc;
pthread_rwlockattr_getkind_np :: (__attr: *pthread_rwlockattr_t, __pref: *s32) -> s32 #foreign libc;
pthread_rwlockattr_getpshared :: (__attr: *pthread_rwlockattr_t, __pshared: *s32) -> s32 #foreign libc;
pthread_rwlockattr_init :: (__attr: *pthread_rwlockattr_t) -> s32 #foreign libc;
pthread_rwlockattr_setkind_np :: (__attr: *pthread_rwlockattr_t, __pref: s32) -> s32 #foreign libc;
pthread_rwlockattr_setpshared :: (__attr: *pthread_rwlockattr_t, __pshared: s32) -> s32 #foreign libc;
pthread_setaffinity_np :: (__th: pthread_t, __cpusetsize: size_t, __cpuset: *cpu_set_t) -> s32 #foreign libc;
pthread_setattr_default_np :: (__attr: *pthread_attr_t) -> s32 #foreign libc;
pthread_setcancelstate :: (__state: s32, __oldstate: *s32) -> s32 #foreign libc;
pthread_setcanceltype :: (__type: s32, __oldtype: *s32) -> s32 #foreign libc;
pthread_setconcurrency :: (__level: s32) -> s32 #foreign libc;
pthread_setname_np :: (__target_thread: pthread_t, __name: *u8) -> s32 #foreign libc;
pthread_setschedparam :: (__target_thread: pthread_t, __policy: s32, __param: *sched_param) -> s32 #foreign libc;
pthread_setschedprio :: (__target_thread: pthread_t, __prio: s32) -> s32 #foreign libc;
pthread_setspecific :: (__key: pthread_key_t, __pointer: *void) -> s32 #foreign libc;
pthread_spin_destroy :: (__lock: *pthread_spinlock_t) -> s32 #foreign libc;
pthread_spin_init :: (__lock: *pthread_spinlock_t, __pshared: s32) -> s32 #foreign libc;
pthread_spin_lock :: (__lock: *pthread_spinlock_t) -> s32 #foreign libc;
pthread_spin_trylock :: (__lock: *pthread_spinlock_t) -> s32 #foreign libc;
pthread_spin_unlock :: (__lock: *pthread_spinlock_t) -> s32 #foreign libc;
pthread_testcancel :: () -> void #foreign libc;
pthread_timedjoin_np :: (__th: pthread_t, __thread_return: **void, __abstime: *timespec) -> s32 #foreign libc;
pthread_tryjoin_np :: (__th: pthread_t, __thread_return: **void) -> s32 #foreign libc;
putc :: (__c: s32, __stream: *FILE) -> s32 #foreign libc;
putc_unlocked :: (__c: s32, __stream: *FILE) -> s32 #foreign libc;
putchar :: (__c: s32) -> s32 #foreign libc;
putchar_unlocked :: (__c: s32) -> s32 #foreign libc;
putpwent :: (__p: *passwd, __f: *FILE) -> s32 #foreign libc;
puts :: (__s: *u8) -> s32 #foreign libc;
putw :: (__w: s32, __stream: *FILE) -> s32 #foreign libc;
pwrite :: (__fd: s32, __buf: *void, __n: size_t, __offset: __off_t) -> ssize_t #foreign libc;
pwrite64 :: (__fd: s32, __buf: *void, __n: size_t, __offset: __off64_t) -> ssize_t #foreign libc;
pwritev :: (__fd: s32, __iovec: *iovec, __count: s32, __offset: __off_t) -> ssize_t #foreign libc;
pwritev2 :: (__fd: s32, __iodev: *iovec, __count: s32, __offset: __off_t, __flags: s32) -> ssize_t #foreign libc;
pwritev64 :: (__fd: s32, __iovec: *iovec, __count: s32, __offset: __off64_t) -> ssize_t #foreign libc;
pwritev64v2 :: (__fd: s32, __iodev: *iovec, __count: s32, __offset: __off64_t, __flags: s32) -> ssize_t #foreign libc;
raise :: (__sig: s32) -> s32 #foreign libc;
readahead :: (__fd: s32, __offset: __off64_t, __count: size_t) -> __ssize_t #foreign libc;
readdir64 :: (__dirp: *DIR) -> *dirent64 #foreign libc;
readdir64_r :: (__dirp: *DIR, __entry: *dirent64, __result: **dirent64) -> s32 #foreign libc;
readdir_r :: (__dirp: *DIR, __entry: *dirent, __result: **dirent) -> s32 #foreign libc;
readlinkat :: (__fd: s32, __path: *u8, __buf: *u8, __len: size_t) -> ssize_t #foreign libc;
readv :: (__fd: s32, __iovec: *iovec, __count: s32) -> ssize_t #foreign libc;
remap_file_pages :: (__start: *void, __size: size_t, __prot: s32, __pgoff: size_t, __flags: s32) -> s32 #foreign libc;
renameat :: (__oldfd: s32, __old: *u8, __newfd: s32, __new: *u8) -> s32 #foreign libc;
revoke :: (__file: *u8) -> s32 #foreign libc;
rewind :: (__stream: *FILE) -> void #foreign libc;
rewinddir :: (__dirp: *DIR) -> void #foreign libc;
sbrk :: (__delta: intptr_t) -> *void #foreign libc;
scandir :: (__dir: *u8, __namelist: ***dirent, __selector: #type (*dirent) -> s32 #c_call, __cmp: #type (**dirent, **dirent) -> s32 #c_call) -> s32 #foreign libc;
scandir64 :: (__dir: *u8, __namelist: ***dirent64, __selector: #type (*dirent64) -> s32 #c_call, __cmp: #type (**dirent64, **dirent64) -> s32 #c_call) -> s32 #foreign libc;
scandirat :: (__dfd: s32, __dir: *u8, __namelist: ***dirent, __selector: #type (*dirent) -> s32 #c_call, __cmp: #type (**dirent, **dirent) -> s32 #c_call) -> s32 #foreign libc;
scandirat64 :: (__dfd: s32, __dir: *u8, __namelist: ***dirent64, __selector: #type (*dirent64) -> s32 #c_call, __cmp: #type (**dirent64, **dirent64) -> s32 #c_call) -> s32 #foreign libc;
scanf :: (__format: *u8, __args: ..Any) -> s32 #foreign libc;
sched_get_priority_max :: (__algorithm: s32) -> s32 #foreign libc;
sched_get_priority_min :: (__algorithm: s32) -> s32 #foreign libc;
sched_getaffinity :: (__pid: __pid_t, __cpusetsize: size_t, __cpuset: *cpu_set_t) -> s32 #foreign libc;
sched_getcpu :: () -> s32 #foreign libc;
sched_getparam :: (__pid: __pid_t, __param: *sched_param) -> s32 #foreign libc;
sched_getscheduler :: (__pid: __pid_t) -> s32 #foreign libc;
sched_rr_get_interval :: (__pid: __pid_t, __t: *timespec) -> s32 #foreign libc;
sched_setaffinity :: (__pid: __pid_t, __cpusetsize: size_t, __cpuset: *cpu_set_t) -> s32 #foreign libc;
sched_setparam :: (__pid: __pid_t, __param: *sched_param) -> s32 #foreign libc;
sched_setscheduler :: (__pid: __pid_t, __policy: s32, __param: *sched_param) -> s32 #foreign libc;
sched_yield :: () -> s32 #foreign libc;
seekdir :: (__dirp: *DIR, __pos: s64) -> void #foreign libc;
sem_close :: (__sem: *sem_t) -> s32 #foreign libc;
sem_destroy :: (__sem: *sem_t) -> s32 #foreign libc;
sem_getvalue :: (__sem: *sem_t, __sval: *s32) -> s32 #foreign libc;
sem_init :: (__sem: *sem_t, __pshared: s32, __value: u32) -> s32 #foreign libc;
sem_open :: (__name: *u8, __oflag: s32, __args: ..Any) -> *sem_t #foreign libc;
sem_post :: (__sem: *sem_t) -> s32 #foreign libc;
sem_timedwait :: (__sem: *sem_t, __abstime: *timespec) -> s32 #foreign libc;
sem_trywait :: (__sem: *sem_t) -> s32 #foreign libc;
sem_unlink :: (__name: *u8) -> s32 #foreign libc;
sem_wait :: (__sem: *sem_t) -> s32 #foreign libc;
setbuf :: (__stream: *FILE, __buf: *u8) -> void #foreign libc;
setbuffer :: (__stream: *FILE, __buf: *u8, __size: size_t) -> void #foreign libc;
setdomainname :: (__name: *u8, __len: size_t) -> s32 #foreign libc;
setegid :: (__gid: __gid_t) -> s32 #foreign libc;
setenv :: (__name: *u8, __value: *u8, __replace: s32) -> s32 #foreign libc;
seteuid :: (__uid: __uid_t) -> s32 #foreign libc;
setgid :: (__gid: __gid_t) -> s32 #foreign libc;
sethostid :: (__id: s64) -> s32 #foreign libc;
sethostname :: (__name: *u8, __len: size_t) -> s32 #foreign libc;
setitimer :: (__which: __itimer_which_t, __new: *itimerval, __old: *itimerval) -> s32 #foreign libc;
setjmp :: (__env: jmp_buf) -> s32 #foreign libc;
setlinebuf :: (__stream: *FILE) -> void #foreign libc;
setlogin :: (__name: *u8) -> s32 #foreign libc;
setns :: (__fd: s32, __nstype: s32) -> s32 #foreign libc;
setpgid :: (__pid: __pid_t, __pgid: __pid_t) -> s32 #foreign libc;
setpgrp :: () -> s32 #foreign libc;
setpriority :: (__which: __priority_which_t, __who: id_t, __prio: s32) -> s32 #foreign libc;
setpwent :: () -> void #foreign libc;
setregid :: (__rgid: __gid_t, __egid: __gid_t) -> s32 #foreign libc;
setresgid :: (__rgid: __gid_t, __egid: __gid_t, __sgid: __gid_t) -> s32 #foreign libc;
setresuid :: (__ruid: __uid_t, __euid: __uid_t, __suid: __uid_t) -> s32 #foreign libc;
setreuid :: (__ruid: __uid_t, __euid: __uid_t) -> s32 #foreign libc;
setrlimit :: (__resource: __rlimit_resource_t, __rlimits: *rlimit) -> s32 #foreign libc;
setrlimit64 :: (__resource: __rlimit_resource_t, __rlimits: *rlimit64) -> s32 #foreign libc;
setsid :: () -> __pid_t #foreign libc;
settimeofday :: (__tv: *timeval, __tz: *timezone) -> s32 #foreign libc;
setuid :: (__uid: __uid_t) -> s32 #foreign libc;
setusershell :: () -> void #foreign libc;
setvbuf :: (__stream: *FILE, __buf: *u8, __modes: s32, __n: size_t) -> s32 #foreign libc;
shmat :: (__shmid: s32, __shmaddr: *void, __shmflg: s32) -> *void #foreign libc;
shmctl :: (__shmid: s32, __cmd: s32, __buf: *shmid_ds) -> s32 #foreign libc;
shmdt :: (__shmaddr: *void) -> s32 #foreign libc;
shmget :: (__key: key_t, __size: size_t, __shmflg: s32) -> s32 #foreign libc;
sigaction :: (__sig: s32, __act: *sigaction_t, __oact: *sigaction_t) -> s32 #foreign libc;
sigaddset :: (__set: *sigset_t, __signo: s32) -> s32 #foreign libc;
sigaltstack :: (__ss: *stack_t, __oss: *stack_t) -> s32 #foreign libc;
sigandset :: (__set: *sigset_t, __left: *sigset_t, __right: *sigset_t) -> s32 #foreign libc;
sigblock :: (__mask: s32) -> s32 #foreign libc;
sigdelset :: (__set: *sigset_t, __signo: s32) -> s32 #foreign libc;
sigemptyset :: (__set: *sigset_t) -> s32 #foreign libc;
sigfillset :: (__set: *sigset_t) -> s32 #foreign libc;
siggetmask :: () -> s32 #foreign libc;
sighold :: (__sig: s32) -> s32 #foreign libc;
sigignore :: (__sig: s32) -> s32 #foreign libc;
siginterrupt :: (__sig: s32, __interrupt: s32) -> s32 #foreign libc;
sigisemptyset :: (__set: *sigset_t) -> s32 #foreign libc;
sigismember :: (__set: *sigset_t, __signo: s32) -> s32 #foreign libc;
siglongjmp :: (__env: sigjmp_buf, __val: s32) -> void #foreign libc;
signal :: (__sig: s32, __handler: __sighandler_t) -> __sighandler_t #foreign libc;
signalfd :: (__fd: s32, __mask: *sigset_t, __flags: s32) -> s32 #foreign libc;
sigorset :: (__set: *sigset_t, __left: *sigset_t, __right: *sigset_t) -> s32 #foreign libc;
sigpause :: (__sig: s32) -> s32 #foreign libc;
sigpending :: (__set: *sigset_t) -> s32 #foreign libc;
sigprocmask :: (__how: s32, __set: *sigset_t, __oset: *sigset_t) -> s32 #foreign libc;
sigqueue :: (__pid: __pid_t, __sig: s32, __val: sigval) -> s32 #foreign libc;
sigrelse :: (__sig: s32) -> s32 #foreign libc;
sigreturn :: (__scp: *sigcontext) -> s32 #foreign libc;
sigset :: (__sig: s32, __disp: __sighandler_t) -> __sighandler_t #foreign libc;
sigsetmask :: (__mask: s32) -> s32 #foreign libc;
sigsuspend :: (__set: *sigset_t) -> s32 #foreign libc;
sigtimedwait :: (__set: *sigset_t, __info: *siginfo_t, __timeout: *timespec) -> s32 #foreign libc;
sigwait :: (__set: *sigset_t, __sig: *s32) -> s32 #foreign libc;
sigwaitinfo :: (__set: *sigset_t, __info: *siginfo_t) -> s32 #foreign libc;
sleep :: (__seconds: u32) -> u32 #foreign libc;
snprintf :: (__s: *u8, __maxlen: size_t, __format: *u8, __args: ..Any) -> s32 #foreign libc;
splice :: (__fdin: s32, __offin: *__off64_t, __fdout: s32, __offout: *__off64_t, __len: size_t, __flags: u32) -> __ssize_t #foreign libc;
sprintf :: (__s: *u8, __format: *u8, __args: ..Any) -> s32 #foreign libc;
sscanf :: (__s: *u8, __format: *u8, __args: ..Any) -> s32 #foreign libc;
ssignal :: (__sig: s32, __handler: __sighandler_t) -> __sighandler_t #foreign libc;
strftime :: (__s: *u8, __maxsize: size_t, __format: *u8, __tp: *tm) -> size_t #foreign libc;
strftime_l :: (__s: *u8, __maxsize: size_t, __format: *u8, __tp: *tm, __loc: locale_t) -> size_t #foreign libc;
strptime :: (__s: *u8, __fmt: *u8, __tp: *tm) -> *u8 #foreign libc;
strptime_l :: (__s: *u8, __fmt: *u8, __tp: *tm, __loc: locale_t) -> *u8 #foreign libc;
strsignal :: (__sig: s32) -> *u8 #foreign libc;
swab :: (__from: *void, __to: *void, __n: ssize_t) -> void #foreign libc;
symlink :: (__from: *u8, __to: *u8) -> s32 #foreign libc;
symlinkat :: (__from: *u8, __tofd: s32, __to: *u8) -> s32 #foreign libc;
sync :: () -> void #foreign libc;
sync_file_range :: (__fd: s32, __offset: __off64_t, __count: __off64_t, __flags: u32) -> s32 #foreign libc;
syncfs :: (__fd: s32) -> s32 #foreign libc;
syscall :: (__sysno: s64, __args: ..Any) -> s64 #foreign libc;
sysv_signal :: (__sig: s32, __handler: __sighandler_t) -> __sighandler_t #foreign libc;
tcgetpgrp :: (__fd: s32) -> __pid_t #foreign libc;
tcsetpgrp :: (__fd: s32, __pgrp_id: __pid_t) -> s32 #foreign libc;
tee :: (__fdin: s32, __fdout: s32, __len: size_t, __flags: u32) -> __ssize_t #foreign libc;
telldir :: (__dirp: *DIR) -> s64 #foreign libc;
tempnam :: (__dir: *u8, __pfx: *u8) -> *u8 #foreign libc;
time :: (__timer: *time_t) -> time_t #foreign libc;
timer_create :: (__clock_id: clockid_t, __evp: *sigevent, __timerid: *timer_t) -> s32 #foreign libc;
timer_delete :: (__timerid: timer_t) -> s32 #foreign libc;
timer_getoverrun :: (__timerid: timer_t) -> s32 #foreign libc;
timer_gettime :: (__timerid: timer_t, __value: *itimerspec) -> s32 #foreign libc;
timer_settime :: (__timerid: timer_t, __flags: s32, __value: *itimerspec, __ovalue: *itimerspec) -> s32 #foreign libc;
timerfd_create :: (__clock_id: __clockid_t, __flags: s32) -> s32 #foreign libc;
timerfd_gettime :: (__ufd: s32, __otmr: *itimerspec) -> s32 #foreign libc;
timerfd_settime :: (__ufd: s32, __flags: s32, __utmr: *itimerspec, __otmr: *itimerspec) -> s32 #foreign libc;
timespec_get :: (__ts: *timespec, __base: s32) -> s32 #foreign libc;
tmpfile :: () -> *FILE #foreign libc;
tmpfile64 :: () -> *FILE #foreign libc;
tmpnam :: (__arg0: *u8) -> *u8 #foreign libc;
tmpnam_r :: (__s: *u8) -> *u8 #foreign libc;
truncate :: (__file: *u8, __length: __off_t) -> s32 #foreign libc;
truncate64 :: (__file: *u8, __length: __off64_t) -> s32 #foreign libc;
ttyname :: (__fd: s32) -> *u8 #foreign libc;
ttyname_r :: (__fd: s32, __buf: *u8, __buflen: size_t) -> s32 #foreign libc;
ttyslot :: () -> s32 #foreign libc;
tzset :: () -> void #foreign libc;
ualarm :: (__value: __useconds_t, __interval: __useconds_t) -> __useconds_t #foreign libc;
umask :: (__mask: __mode_t) -> __mode_t #foreign libc;
uname :: (__name: *utsname) -> s32 #foreign libc;
ungetc :: (__c: s32, __stream: *FILE) -> s32 #foreign libc;
unlinkat :: (__fd: s32, __name: *u8, __flag: s32) -> s32 #foreign libc;
unsetenv :: (__name: *u8) -> s32 #foreign libc;
unshare :: (__flags: s32) -> s32 #foreign libc;
usleep :: (__useconds: __useconds_t) -> s32 #foreign libc;
utimensat :: (__fd: s32, __path: *u8, __times: *timespec, __flags: s32) -> s32 #foreign libc;
utimes :: (__file: *u8, __tvp: *timeval) -> s32 #foreign libc;
versionsort :: (__e1: **dirent, __e2: **dirent) -> s32 #foreign libc;
versionsort64 :: (__e1: **dirent64, __e2: **dirent64) -> s32 #foreign libc;
vfork :: () -> s32 #foreign libc;
vhangup :: () -> s32 #foreign libc;
vmsplice :: (__fdout: s32, __iov: *iovec, __count: size_t, __flags: u32) -> __ssize_t #foreign libc;
wait :: (__stat_loc: *s32) -> __pid_t #foreign libc;
wait3 :: (__stat_loc: *s32, __options: s32, __usage: *rusage) -> __pid_t #foreign libc;
wait4 :: (__pid: __pid_t, __stat_loc: *s32, __options: s32, __usage: *rusage) -> __pid_t #foreign libc;
waitid :: (__idtype: idtype_t, __id: __id_t, __infop: *siginfo_t, __options: s32) -> s32 #foreign libc;
writev :: (__fd: s32, __iovec: *iovec, __count: s32) -> ssize_t #foreign libc;

// ---------------------------------------------------------- the overrides ---

clockid_t :: enum u32 {
    REALTIME           :: 0;
    MONOTONIC          :: 1;
    PROCESS_CPUTIME_ID :: 2;
    THREAD_CPUTIME_ID  :: 3;
    MONOTONIC_RAW      :: 4;
    REALTIME_COARSE    :: 5;
    MONOTONIC_COARSE   :: 6;
    BOOTTIME           :: 7;
    REALTIME_ALARM     :: 8;
    BOOTTIME_ALARM     :: 9;
    TAI                :: 11;

    CLOCK_REALTIME           :: REALTIME;
    CLOCK_MONOTONIC          :: MONOTONIC;
    CLOCK_PROCESS_CPUTIME_ID :: PROCESS_CPUTIME_ID;
    CLOCK_THREAD_CPUTIME_ID  :: THREAD_CPUTIME_ID;
    CLOCK_MONOTONIC_RAW      :: MONOTONIC_RAW;
    CLOCK_REALTIME_COARSE    :: REALTIME_COARSE;
    CLOCK_MONOTONIC_COARSE   :: MONOTONIC_COARSE;
    CLOCK_BOOTTIME           :: BOOTTIME;
    CLOCK_REALTIME_ALARM     :: REALTIME_ALARM;
    CLOCK_BOOTTIME_ALARM     :: BOOTTIME_ALARM;
    CLOCK_TAI                :: TAI;
}

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
