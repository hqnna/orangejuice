//! The distribution orangejuice ships (`modules/`).
//!
//! `oj` is a Jai distribution of its own: `modules/` sits at the root of this
//! repository, and the compiler finds it from its own binary rather than from
//! anything a caller has to set. What is asserted here is that the modules a
//! program cannot do without — Preload, Basic, Runtime_Support — hold up under
//! a real build.

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

/// `OJ_MODULES` is process-wide and the tests in this binary run on threads,
/// so the one that points the compiler at a distribution of its own and the
/// one that asserts the compiler finds its own without being pointed anywhere
/// cannot overlap. Whoever writes the variable holds this, and puts it back
/// before letting go.
static MODULES_ENV_GUARD: Mutex<()> = Mutex::new(());

fn linker_is_available() -> bool {
  let driver = oj_link::driver(oj_types::Target::HOST);
  Command::new(&driver)
    .arg("--version")
    .output()
    .is_ok_and(|output| output.status.success())
}

/// Builds `body` against orangejuice's own modules and runs it.
fn build_and_run(body: &str) -> Option<String> {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return None;
  }

  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, body).expect("the input should be writable");

  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe {
    oj_testsupport::use_own_modules();
  }

  let options = oj_driver::BuildOptions::new();
  let report = oj_driver::run(&path, &options, oj_driver::Stage::Executable, None);
  assert!(
    !report.failed,
    "the program should build against our own modules, but:\n{}",
    report.diagnostics.join("")
  );

  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  assert!(
    output.status.success(),
    "the program should run, but exited {:?}",
    output.status.code()
  );
  Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[test]
fn a_program_builds_and_runs_with_no_jai_distribution() {
  // Preload, Runtime_Support and Basic are ours, and nothing the reference
  // ships is involved.
  let Some(output) = build_and_run(
    "#import \"Basic\";\n\
     main :: () { print(\"Hello, %!\\n\", \"world\"); }\n",
  ) else {
    return;
  };
  assert_eq!(output, "Hello, world!\n");
}

#[test]
fn print_formats_what_the_type_table_describes() {
  // `print` decides what to write by walking the `Type_Info` the compiler put
  // in the table, so this is as much a test of the table as of the module.
  let Some(output) = build_and_run(
    "#import \"Basic\";\n\
     Colour :: enum { RED; GREEN; BLUE; }\n\
     Point :: struct { x: int; y: int; name: string; }\n\
     main :: () {\n  \
       print(\"% % %\\n\", 42, -17, 9223372036854775807);\n  \
       print(\"% %\\n\", true, false);\n  \
       print(\"% %\\n\", Colour.GREEN, Colour.BLUE);\n  \
       print(\"%\\n\", Point.{3, 4, \"origin\"});\n  \
       print(\"%\\n\", int.[1, 2, 3]);\n  \
       print(\"100%%\\n\");\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(
    output,
    "42 -17 9223372036854775807\n\
     true false\n\
     GREEN BLUE\n\
     {3, 4, \"origin\"}\n\
     [1, 2, 3]\n\
     100%\n"
  );
}

#[test]
fn a_context_member_a_module_added_keeps_the_default_it_declared() {
  // `__jai_runtime_init` builds the context from a declaration without a
  // `__jai_runtime_init` builds the context from a declaration without a
  // value, so `#add_context` members arrive with their defaults rather than
  // zeroed (**L§4.6**, **L§10.2**). `Print_Style`'s own defaults are nested
  // two structs deep, which is what makes this worth checking: the pointer
  // base of `default_format_absolute_pointer` is set in the declaration.
  let Some(output) = build_and_run(
    "#import \"Basic\";\n\
     main :: () {\n  \
       print(\"%\\n\", context.print_style.default_format_absolute_pointer.base);\n  \
       print(\"%\\n\", context.print_style.default_format_array.stop_printing_after_this_many_elements);\n  \
       print(\"%\\n\", 0.5);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(output, "16\n100\n0.5\n");
}

#[test]
fn a_resizable_array_grows_through_the_context_allocator() {
  let Some(output) = build_and_run(
    "#import \"Basic\";\n\
     main :: () {\n  \
       xs: [..] int;\n  \
       for 1..100  array_add(*xs, it);\n  \
       total := 0;\n  \
       for xs  total += it;\n  \
       print(\"% %\\n\", xs.count, total);\n  \
       array_free(*xs);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(output, "100 5050\n");
}

#[test]
fn temporary_storage_is_reset_in_one_go() {
  // `tprint` allocates out of the arena; resetting hands it all back, so a
  // loop that prints a great many strings does not grow without bound
  // (**L§10.4**).
  let Some(output) = build_and_run(
    "#import \"Basic\";\n\
     main :: () {\n  \
       for 1..1000 {\n    \
         s := tprint(\"%-%\", it, it * 2);\n    \
         if it == 1000  print(\"%\\n\", s);\n    \
         reset_temporary_storage();\n  \
       }\n  \
       print(\"%\\n\", context.temporary_storage.total_bytes_occupied);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(output, "1000-2000\n0\n");
}

#[test]
fn the_distribution_is_found_without_being_pointed_at() {
  // What makes `oj` usable on its own: the modules are found from the
  // executable rather than from an environment variable.
  let _guard = MODULES_ENV_GUARD
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  let found = oj_driver::distribution();
  assert!(
    found.is_some_and(|directory| directory.join("modules").join("Preload.jai").is_file()),
    "orangejuice should find the modules it ships"
  );
}

/// The declarations the compiler reads out of Preload by name (**L§17**). A
/// rename here is a rename the compiler cannot follow, so it is worth failing
/// loudly rather than as a mystery somewhere in the type table.
#[test]
fn preload_declares_what_the_compiler_looks_up() {
  let preload = oj_testsupport::modules().join("Preload.jai");
  let text = std::fs::read_to_string(&preload).expect("Preload should be readable");
  for name in [
    "Allocator",
    "Type_Info_Tag",
    "Type_Info_Struct_Member",
    "Stack_Trace_Node",
    "Stack_Trace_Procedure_Info",
    "Source_Code_Location",
    "Any_Struct",
    "__reg",
    "memcmp",
  ] {
    assert!(
      text.contains(name),
      "Preload should declare '{name}', which the compiler looks up by name"
    );
  }
  for field in [
    "runtime_size",
    "offset_in_bytes",
    "offset_into_constant_storage",
    "polymorph_source_struct",
    "array_count",
    "internal_type",
  ] {
    assert!(
      text.contains(field),
      "Preload's type info should carry '{field}', which the type table image writes by name"
    );
  }
  let _: &Path = preload.as_path();
}

#[test]
fn a_crash_names_itself_and_walks_the_stack() {
  // A program that dies of a signal should say what happened rather than
  // leaving a silent core file (**C§13**). The generated entry point installs
  // `Runtime_Support_Crash_Handler` unless `-no_backtrace_on_crash` asks it
  // not to, and the wording is the reference's.
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }

  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("crash.jai");
  std::fs::write(
    &path,
    "#import \"Basic\";\n\
     read :: (p: *int) -> int { return p.*; }\n\
     main :: () {\n  \
       print(\"before\\n\");\n  \
       p: *int;\n  \
       print(\"%\\n\", read(p));\n\
     }\n",
  )
  .expect("the input should be writable");

  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe {
    oj_testsupport::use_own_modules();
  }

  let options = oj_driver::BuildOptions::new();
  let report = oj_driver::run(&path, &options, oj_driver::Stage::Executable, None);
  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );

  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  assert!(
    !output.status.success(),
    "the program should die of the signal it got"
  );

  let errors = String::from_utf8_lossy(&output.stderr);
  // The kernel's own name for what it caught. Linux calls address zero
  // unmapped and Darwin calls it unreadable; the program made the same mistake
  // either way, which is what the line under it says.
  let expected = match oj_types::Target::HOST.is_darwin() {
    true => "Program Exception: SEGV_ACCERR",
    false => "Program Exception: SEGV_MAPERR",
  };
  assert!(
    errors.contains(expected),
    "the fault should be named, but stderr was:\n{errors}"
  );
  assert!(
    errors.contains("Null Pointer Exception. Attempt to dereference a null pointer."),
    "a null dereference should say so, but stderr was:\n{errors}"
  );
  // `backtrace_symbols_fd` writes one line per frame, in the format the C
  // library it came from uses: glibc brackets the address, Darwin's columns
  // it.
  let frame = match oj_types::Target::HOST.is_darwin() {
    true => " 0x0000",
    false => "[0x",
  };
  assert!(
    errors.lines().filter(|line| line.contains(frame)).count() >= 2,
    "the stack should be walked, but stderr was:\n{errors}"
  );
}

/// Running the compiler from the directory its own `modules/` sits in is the
/// shape the portable tarball unpacks into, and it used to fail: the import
/// path holds `<dir of the first file>/modules` and `<distribution>/modules`,
/// which are then the same directory spelled relatively and absolutely. The
/// compiler imports `Runtime_Support` on the program's behalf under one
/// spelling and the program imports it under the other, and two copies of one
/// module declare two nominal types of the same name — which surfaces far from
/// the cause, as `Type wanted: *Temporary_Storage; type given:
/// *Temporary_Storage`.
#[test]
fn a_module_reached_two_ways_is_one_module() {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  // A distribution laid out the way the release tarball is: `modules/` beside
  // the program being compiled.
  let modules = directory.path().join("modules");
  std::os::unix::fs::symlink(oj_testsupport::modules(), &modules)
    .expect("the modules link should be creatable");
  let path = directory.path().join("main.jai");
  std::fs::write(
    &path,
    "#import \"Basic\";\nmain :: () { print(\"one module\\n\"); }\n",
  )
  .expect("the input should be writable");

  // SAFETY: the variable is process-wide and read by
  // `the_distribution_is_found_without_being_pointed_at`, so the write is held
  // apart from that test rather than merely from other writers.
  let _guard = MODULES_ENV_GUARD
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  unsafe { std::env::set_var(oj_testsupport::MODULES_ENV, &modules) };

  let options = oj_driver::BuildOptions {
    output_path: Some(directory.path().to_path_buf()),
    ..oj_driver::BuildOptions::new()
  };
  let report = oj_driver::run(&path, &options, oj_driver::Stage::Executable, None);
  // The build is over, and `directory` is about to be deleted with the
  // temporary directory: nothing may be left naming a path inside it.
  // SAFETY: as above, and the guard is still held.
  unsafe { oj_testsupport::use_own_modules() };

  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "one module\n");
}

/// Every layout and every constant `POSIX` and the crash handler fix, measured
/// against the system's own headers, on whichever machine this runs on.
///
/// Most of `POSIX` is hand-written: `module.jai` for what every Unix agrees
/// on, `linux.jai` and `macos.jai` for what a kernel decides. Only the glibc
/// half has a generated binding behind it, and even that is generated on *one*
/// architecture — `struct stat` is arranged differently on arm64 than on
/// x86-64, glibc sizes its opaque pthread objects differently, and arm64
/// overrides `O_DIRECTORY`.
///
/// So they are compared directly: a C program prints what `<sys/stat.h>` and
/// the rest say, a Jai program prints what the front end made of our
/// declarations, and every fact has to agree. The constants matter as much as
/// the layouts and are easier to get wrong quietly — a `struct stat` that is
/// eight bytes off announces itself, where an `SO_REUSEADDR` that is wrong by
/// two just stops working, and `_SC_GETPW_R_SIZE_MAX` written as 70 where
/// Darwin says 71 only sizes a buffer wrong.
#[test]
fn the_posix_declarations_match_the_system_headers() {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to measure with");
    return;
  }

  let directory = tempfile::tempdir().expect("a temporary directory");
  let c_path = directory.path().join("measure.c");
  std::fs::write(&c_path, LAYOUT_C).expect("the C source is writable");
  let measure = directory.path().join("measure");
  let built = Command::new(oj_link::driver(oj_types::Target::HOST))
    .arg("-o")
    .arg(&measure)
    .arg(&c_path)
    .output()
    .expect("the C driver should run");
  // Not a skip: a measuring program that does not build is this check quietly
  // switching itself off, which is the one outcome worse than a mismatch.
  assert!(
    built.status.success(),
    "the measuring program should compile, but:\n{}",
    String::from_utf8_lossy(&built.stderr)
  );
  let from_c = Command::new(&measure)
    .output()
    .expect("the measuring program should run");
  let from_c = String::from_utf8_lossy(&from_c.stdout).into_owned();

  let from_jai = build_and_run(LAYOUT_JAI).expect("the Jai side should build and run");

  assert!(
    from_c.lines().count() > 20,
    "the C side measured nothing:\n{from_c}"
  );
  // Every difference at once, named: fixing these one panic at a time is a
  // recompile per constant.
  let ours: std::collections::BTreeMap<&str, &str> = from_jai
    .lines()
    .filter_map(|line| line.split_once(' '))
    .collect();
  let mut wrong = Vec::new();
  for line in from_c.lines() {
    let Some((name, theirs)) = line.split_once(' ') else {
      continue;
    };
    match ours.get(name) {
      Some(mine) if *mine == theirs => {}
      Some(mine) => wrong.push(format!("  {name}: the system says {theirs}, we say {mine}")),
      None => wrong.push(format!("  {name}: the system says {theirs}, we do not say")),
    }
  }
  assert!(
    wrong.is_empty(),
    "{} of {} facts do not match the system headers:\n{}",
    wrong.len(),
    from_c.lines().count(),
    wrong.join("\n")
  );
}

const LAYOUT_C: &str = r##"
/* `<ucontext.h>` is XSI on Darwin, and `_DARWIN_C_SOURCE` puts back the BSD
   members that asking for XSI alone would hide — `d_namlen`, `pw_change` and
   the rest. `_GNU_SOURCE` is glibc's equivalent, and is what `REG_RIP` needs. */
#ifdef __APPLE__
  #define _XOPEN_SOURCE 700
  #define _DARWIN_C_SOURCE 1
#else
  #define _GNU_SOURCE 1
#endif
#include <stdio.h>
#include <stddef.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <pthread.h>
#include <pwd.h>
#include <signal.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>
#include <netinet/in.h>
#include <ucontext.h>
#ifdef __APPLE__
  #include <sys/event.h>
  #include <sys/sysctl.h>
  #include <sys/param.h>
  #include <mach/mach_time.h>
#endif

#define SIZE(name, type)          printf("%s %zu\n", name, sizeof(type))
#define OFF(name, type, member)   printf("%s %zu\n", name, offsetof(type, member))
#define VAL(name, expr)           printf("%s %lld\n", name, (long long)(expr))

int main(void) {
  /* The flag numbers, error numbers and socket constants. A layout that is
     wrong by eight bytes announces itself; `SO_REUSEADDR` that is wrong by two
     just stops working, so these are measured too. */
  VAL("O_RDONLY", O_RDONLY);
  VAL("O_WRONLY", O_WRONLY);
  VAL("O_RDWR", O_RDWR);
  VAL("O_NONBLOCK", O_NONBLOCK);
  VAL("O_APPEND", O_APPEND);
  VAL("O_CREAT", O_CREAT);
  VAL("O_TRUNC", O_TRUNC);
  VAL("O_EXCL", O_EXCL);
  VAL("O_DIRECTORY", O_DIRECTORY);
  VAL("O_CLOEXEC", O_CLOEXEC);

  VAL("SEEK_SET", SEEK_SET);
  VAL("SEEK_CUR", SEEK_CUR);
  VAL("SEEK_END", SEEK_END);

  VAL("S_IFMT", S_IFMT);
  VAL("S_IFDIR", S_IFDIR);
  VAL("S_IFREG", S_IFREG);
  VAL("S_IFLNK", S_IFLNK);

  VAL("EPERM", EPERM);
  VAL("ENOENT", ENOENT);
  VAL("EINTR", EINTR);
  VAL("EIO", EIO);
  VAL("EBADF", EBADF);
  VAL("ENOMEM", ENOMEM);
  VAL("EACCES", EACCES);
  VAL("EEXIST", EEXIST);
  VAL("ENOTDIR", ENOTDIR);
  VAL("EISDIR", EISDIR);
  VAL("EINVAL", EINVAL);
  VAL("ERANGE", ERANGE);
  VAL("EAGAIN", EAGAIN);
  VAL("EWOULDBLOCK", EWOULDBLOCK);
  VAL("ETIMEDOUT", ETIMEDOUT);
  VAL("ELOOP", ELOOP);

  VAL("PROT_READ", PROT_READ);
  VAL("PROT_WRITE", PROT_WRITE);
  VAL("MAP_SHARED", MAP_SHARED);
  VAL("MAP_PRIVATE", MAP_PRIVATE);
  VAL("MAP_ANONYMOUS", MAP_ANONYMOUS);

  VAL("AF_INET", AF_INET);
  VAL("AF_INET6", AF_INET6);
  VAL("SOCK_STREAM", SOCK_STREAM);
  VAL("SOCK_DGRAM", SOCK_DGRAM);
  VAL("SOL_SOCKET", SOL_SOCKET);
  VAL("SO_REUSEADDR", SO_REUSEADDR);
  VAL("SHUT_RD", SHUT_RD);
  VAL("SHUT_WR", SHUT_WR);
  VAL("SHUT_RDWR", SHUT_RDWR);

  VAL("POLLIN", POLLIN);
  VAL("POLLOUT", POLLOUT);
  VAL("POLLERR", POLLERR);
  VAL("POLLHUP", POLLHUP);

  VAL("DT.DIR", DT_DIR);
  VAL("DT.REG", DT_REG);
  VAL("DT.LNK", DT_LNK);

  VAL("CLOCK.REALTIME", CLOCK_REALTIME);
  VAL("CLOCK.MONOTONIC", CLOCK_MONOTONIC);
  VAL("CLOCK.MONOTONIC_RAW", CLOCK_MONOTONIC_RAW);

#ifdef __APPLE__
  VAL("PTHREAD_MUTEX.RECURSIVE_NP", PTHREAD_MUTEX_RECURSIVE);
#else
  VAL("PTHREAD_MUTEX.RECURSIVE_NP", PTHREAD_MUTEX_RECURSIVE_NP);
#endif

  VAL("_SC_NPROCESSORS_ONLN", _SC_NPROCESSORS_ONLN);
  VAL("_SC_NPROCESSORS_CONF", _SC_NPROCESSORS_CONF);
  VAL("_SC_PAGESIZE", _SC_PAGESIZE);
  VAL("_SC_GETPW_R_SIZE_MAX", _SC_GETPW_R_SIZE_MAX);

  VAL("SIGQUIT", SIGQUIT);
  VAL("SIGILL", SIGILL);
  VAL("SIGTRAP", SIGTRAP);
  VAL("SIGABRT", SIGABRT);
  VAL("SIGBUS", SIGBUS);
  VAL("SIGFPE", SIGFPE);
  VAL("SIGSEGV", SIGSEGV);
  VAL("SIGINT", SIGINT);
  VAL("SIGKILL", SIGKILL);
  VAL("SIGTERM", SIGTERM);

  VAL("SEGV_MAPERR", SEGV_MAPERR);
  VAL("SEGV_ACCERR", SEGV_ACCERR);
  VAL("SA_ONSTACK", SA_ONSTACK);
  VAL("SA_RESTART", SA_RESTART);
  VAL("SA_SIGINFO", SA_SIGINFO);
  VAL("SA_RESTART_ONSTACK_SIGINFO", SA_ONSTACK | SA_RESTART | SA_SIGINFO);

  SIZE("stat_t", struct stat);
  OFF("stat_t.st_mode", struct stat, st_mode);
  OFF("stat_t.st_ino", struct stat, st_ino);
#ifdef __APPLE__
  OFF("stat_t.st_mtime", struct stat, st_mtimespec);
#else
  OFF("stat_t.st_mtime", struct stat, st_mtim);
#endif
  OFF("stat_t.st_size", struct stat, st_size);
  OFF("stat_t.st_blocks", struct stat, st_blocks);

  SIZE("dirent", struct dirent);
  OFF("dirent.d_ino", struct dirent, d_ino);
  OFF("dirent.d_reclen", struct dirent, d_reclen);
  OFF("dirent.d_type", struct dirent, d_type);
  OFF("dirent.d_name", struct dirent, d_name);

  SIZE("siginfo_t", siginfo_t);
  OFF("siginfo_t.si_code", siginfo_t, si_code);
  OFF("siginfo_t.si_addr", siginfo_t, si_addr);

  SIZE("sigaction_t", struct sigaction);
  OFF("sigaction_t.sa_mask", struct sigaction, sa_mask);
  OFF("sigaction_t.sa_flags", struct sigaction, sa_flags);

  SIZE("stack_t", stack_t);
  OFF("stack_t.ss_size", stack_t, ss_size);
  OFF("stack_t.ss_flags", stack_t, ss_flags);

  SIZE("timeval", struct timeval);
  OFF("timeval.tv_usec", struct timeval, tv_usec);

  SIZE("passwd", struct passwd);
  OFF("passwd.pw_uid", struct passwd, pw_uid);
  OFF("passwd.pw_dir", struct passwd, pw_dir);
  OFF("passwd.pw_shell", struct passwd, pw_shell);

  SIZE("pollfd", struct pollfd);
  OFF("pollfd.revents", struct pollfd, revents);

  SIZE("sockaddr_in", struct sockaddr_in);
  OFF("sockaddr_in.sin_family", struct sockaddr_in, sin_family);
  OFF("sockaddr_in.sin_port", struct sockaddr_in, sin_port);
  OFF("sockaddr_in.sin_addr", struct sockaddr_in, sin_addr);

  SIZE("pthread_mutex_t", pthread_mutex_t);
  SIZE("pthread_cond_t", pthread_cond_t);
  SIZE("pthread_attr_t", pthread_attr_t);

#ifdef __APPLE__
  /* What only Darwin has, which `modules/Darwin` binds. */
  SIZE("kevent_t", struct kevent);
  OFF("kevent_t.filter", struct kevent, filter);
  OFF("kevent_t.flags", struct kevent, flags);
  OFF("kevent_t.fflags", struct kevent, fflags);
  OFF("kevent_t.data", struct kevent, data);
  OFF("kevent_t.udata", struct kevent, udata);
  SIZE("mach_timebase_info_data_t", mach_timebase_info_data_t);
  VAL("EVFILT_READ", EVFILT_READ);
  VAL("EVFILT_WRITE", EVFILT_WRITE);
  VAL("EV_ADD", EV_ADD);
  VAL("EV_DELETE", EV_DELETE);
  VAL("EV_CLEAR", EV_CLEAR);
  VAL("NOTE_WRITE", NOTE_WRITE);
  VAL("CTL_HW", CTL_HW);
  VAL("HW_NCPU", HW_NCPU);
  VAL("MAXPATHLEN", MAXPATHLEN);
#endif

  /* Where the crash handler reads the saved program counter from. Darwin
     keeps a pointer to the machine state in the `ucontext_t`; Linux keeps the
     state inline, and both numbers are the architecture's. */
  VAL("UC_MCONTEXT", offsetof(ucontext_t, uc_mcontext));
#if defined(__APPLE__)
  VAL("MCONTEXT_FP", offsetof(struct __darwin_mcontext64, __ss)
                       + offsetof(struct __darwin_arm_thread_state64, __fp));
  VAL("MCONTEXT_PC", offsetof(struct __darwin_mcontext64, __ss)
                       + offsetof(struct __darwin_arm_thread_state64, __pc));
#elif defined(__aarch64__)
  VAL("MCONTEXT_FP", offsetof(struct sigcontext, regs) + 29 * 8);
  VAL("MCONTEXT_PC", offsetof(struct sigcontext, pc));
#else
  VAL("MCONTEXT_PC", offsetof(mcontext_t, gregs) + REG_RIP * 8);
#endif
  return 0;
}
"##;

const LAYOUT_JAI: &str = r##"
#import "Basic";
#import "POSIX";
Crash :: #import "Runtime_Support_Crash_Handler";

off :: (base: *void, member: *void) -> s64 {
    return cast,no_check(s64) member - cast,no_check(s64) base;
}

main :: () {
    print("O_RDONLY %\n", O_RDONLY);
    print("O_WRONLY %\n", O_WRONLY);
    print("O_RDWR %\n", O_RDWR);
    print("O_NONBLOCK %\n", O_NONBLOCK);
    print("O_APPEND %\n", O_APPEND);
    print("O_CREAT %\n", O_CREAT);
    print("O_TRUNC %\n", O_TRUNC);
    print("O_EXCL %\n", O_EXCL);
    print("O_DIRECTORY %\n", O_DIRECTORY);
    print("O_CLOEXEC %\n", O_CLOEXEC);

    print("SEEK_SET %\n", SEEK_SET);
    print("SEEK_CUR %\n", SEEK_CUR);
    print("SEEK_END %\n", SEEK_END);

    print("S_IFMT %\n", S_IFMT);
    print("S_IFDIR %\n", S_IFDIR);
    print("S_IFREG %\n", S_IFREG);
    print("S_IFLNK %\n", S_IFLNK);

    print("EPERM %\n", EPERM);
    print("ENOENT %\n", ENOENT);
    print("EINTR %\n", EINTR);
    print("EIO %\n", EIO);
    print("EBADF %\n", EBADF);
    print("ENOMEM %\n", ENOMEM);
    print("EACCES %\n", EACCES);
    print("EEXIST %\n", EEXIST);
    print("ENOTDIR %\n", ENOTDIR);
    print("EISDIR %\n", EISDIR);
    print("EINVAL %\n", EINVAL);
    print("ERANGE %\n", ERANGE);
    print("EAGAIN %\n", EAGAIN);
    print("EWOULDBLOCK %\n", EWOULDBLOCK);
    print("ETIMEDOUT %\n", ETIMEDOUT);
    print("ELOOP %\n", ELOOP);

    print("PROT_READ %\n", PROT_READ);
    print("PROT_WRITE %\n", PROT_WRITE);
    print("MAP_SHARED %\n", MAP_SHARED);
    print("MAP_PRIVATE %\n", MAP_PRIVATE);
    print("MAP_ANONYMOUS %\n", MAP_ANONYMOUS);

    print("AF_INET %\n", AF_INET);
    print("AF_INET6 %\n", AF_INET6);
    print("SOCK_STREAM %\n", SOCK_STREAM);
    print("SOCK_DGRAM %\n", SOCK_DGRAM);
    print("SOL_SOCKET %\n", SOL_SOCKET);
    print("SO_REUSEADDR %\n", SO_REUSEADDR);
    print("SHUT_RD %\n", SHUT_RD);
    print("SHUT_WR %\n", SHUT_WR);
    print("SHUT_RDWR %\n", SHUT_RDWR);

    print("POLLIN %\n", POLLIN);
    print("POLLOUT %\n", POLLOUT);
    print("POLLERR %\n", POLLERR);
    print("POLLHUP %\n", POLLHUP);

    print("DT.DIR %\n", cast(s64) DT.DIR);
    print("DT.REG %\n", cast(s64) DT.REG);
    print("DT.LNK %\n", cast(s64) DT.LNK);

    print("CLOCK.REALTIME %\n", cast(s64) clockid_t.REALTIME);
    print("CLOCK.MONOTONIC %\n", cast(s64) clockid_t.MONOTONIC);
    print("CLOCK.MONOTONIC_RAW %\n", cast(s64) clockid_t.MONOTONIC_RAW);

    print("PTHREAD_MUTEX.RECURSIVE_NP %\n", cast(s64) PTHREAD_MUTEX.RECURSIVE_NP);

    print("_SC_NPROCESSORS_ONLN %\n", cast(s64) _SC_NPROCESSORS_ONLN);
    print("_SC_NPROCESSORS_CONF %\n", cast(s64) _SC_NPROCESSORS_CONF);
    print("_SC_PAGESIZE %\n", cast(s64) _SC_PAGESIZE);
    print("_SC_GETPW_R_SIZE_MAX %\n", cast(s64) _SC_GETPW_R_SIZE_MAX);

    print("SIGQUIT %\n", Crash.SIGQUIT);
    print("SIGILL %\n", Crash.SIGILL);
    print("SIGTRAP %\n", Crash.SIGTRAP);
    print("SIGABRT %\n", Crash.SIGABRT);
    print("SIGBUS %\n", Crash.SIGBUS);
    print("SIGFPE %\n", Crash.SIGFPE);
    print("SIGSEGV %\n", Crash.SIGSEGV);
    print("SIGINT %\n", SIGINT);
    print("SIGKILL %\n", SIGKILL);
    print("SIGTERM %\n", SIGTERM);

    print("SEGV_MAPERR %\n", Crash.SEGV_MAPERR);
    print("SEGV_ACCERR %\n", Crash.SEGV_ACCERR);
    print("SA_ONSTACK %\n", Crash.SA_ONSTACK);
    print("SA_RESTART %\n", Crash.SA_RESTART);
    print("SA_SIGINFO %\n", Crash.SA_SIGINFO);
    print("SA_RESTART_ONSTACK_SIGINFO %\n", Crash.SA_RESTART_ONSTACK_SIGINFO);

    s: stat_t;
    print("stat_t %\n", size_of(stat_t));
    print("stat_t.st_mode %\n", off(*s, *s.st_mode));
    print("stat_t.st_ino %\n", off(*s, *s.st_ino));
    print("stat_t.st_mtime %\n", off(*s, *s.st_mtime));
    print("stat_t.st_size %\n", off(*s, *s.st_size));
    print("stat_t.st_blocks %\n", off(*s, *s.st_blocks));

    d: dirent;
    print("dirent %\n", size_of(dirent));
    print("dirent.d_ino %\n", off(*d, *d.d_ino));
    print("dirent.d_reclen %\n", off(*d, *d.d_reclen));
    print("dirent.d_type %\n", off(*d, *d.d_type));
    print("dirent.d_name %\n", off(*d, *d.d_name));

    i: Crash.siginfo_t;
    print("siginfo_t %\n", size_of(Crash.siginfo_t));
    print("siginfo_t.si_code %\n", off(*i, *i.si_code));
    print("siginfo_t.si_addr %\n", off(*i, *i.si_addr));

    a: Crash.sigaction_t;
    print("sigaction_t %\n", size_of(Crash.sigaction_t));
    print("sigaction_t.sa_mask %\n", off(*a, *a.sa_mask));
    print("sigaction_t.sa_flags %\n", off(*a, *a.sa_flags));

    k: Crash.stack_t;
    print("stack_t %\n", size_of(Crash.stack_t));
    print("stack_t.ss_size %\n", off(*k, *k.ss_size));
    print("stack_t.ss_flags %\n", off(*k, *k.ss_flags));

    t: timeval;
    print("timeval %\n", size_of(timeval));
    print("timeval.tv_usec %\n", off(*t, *t.tv_usec));

    p: passwd;
    print("passwd %\n", size_of(passwd));
    print("passwd.pw_uid %\n", off(*p, *p.pw_uid));
    print("passwd.pw_dir %\n", off(*p, *p.pw_dir));
    print("passwd.pw_shell %\n", off(*p, *p.pw_shell));

    f: pollfd;
    print("pollfd %\n", size_of(pollfd));
    print("pollfd.revents %\n", off(*f, *f.revents));

    n: sockaddr_in;
    print("sockaddr_in %\n", size_of(sockaddr_in));
    print("sockaddr_in.sin_family %\n", off(*n, *n.sin_family));
    print("sockaddr_in.sin_port %\n", off(*n, *n.sin_port));
    print("sockaddr_in.sin_addr %\n", off(*n, *n.sin_addr));

    print("pthread_mutex_t %\n", size_of(pthread_mutex_t));
    print("pthread_cond_t %\n", size_of(pthread_cond_t));
    print("pthread_attr_t %\n", size_of(pthread_attr_t));

    #if OS == .MACOS {
        Darwin :: #import "Darwin";
        ev: Darwin.kevent_t;
        print("kevent_t %\n", size_of(Darwin.kevent_t));
        print("kevent_t.filter %\n", off(*ev, *ev.filter));
        print("kevent_t.flags %\n", off(*ev, *ev.flags));
        print("kevent_t.fflags %\n", off(*ev, *ev.fflags));
        print("kevent_t.data %\n", off(*ev, *ev.data));
        print("kevent_t.udata %\n", off(*ev, *ev.udata));
        print("mach_timebase_info_data_t %\n", size_of(Darwin.mach_timebase_info_data_t));
        print("EVFILT_READ %\n", Darwin.EVFILT_READ);
        print("EVFILT_WRITE %\n", Darwin.EVFILT_WRITE);
        print("EV_ADD %\n", cast(s64) Darwin.EV_ADD);
        print("EV_DELETE %\n", cast(s64) Darwin.EV_DELETE);
        print("EV_CLEAR %\n", cast(s64) Darwin.EV_CLEAR);
        print("NOTE_WRITE %\n", cast(s64) Darwin.NOTE_WRITE);
        print("CTL_HW %\n", Darwin.CTL_HW);
        print("HW_NCPU %\n", Darwin.HW_NCPU);
        print("MAXPATHLEN %\n", Darwin.MAXPATHLEN);
    }

    print("UC_MCONTEXT %\n", Crash.UC_MCONTEXT);
    #if OS == .MACOS || CPU == .ARM64 {
        print("MCONTEXT_FP %\n", Crash.MCONTEXT_FP);
    }
    print("MCONTEXT_PC %\n", Crash.MCONTEXT_PC);
}
"##;
