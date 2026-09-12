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

/// Every layout `POSIX/macos.jai` and the crash handler fix for Darwin,
/// measured against the system's own headers.
///
/// The Linux side of `POSIX` has a generated binding behind it: a struct's
/// layout there is whatever the C compiler laid out, because the C compiler is
/// what produced the file (`tools/cbind/README.md`). Darwin has no generated
/// binding, so these were written from the manual pages — and a hand-written
/// `struct stat` is exactly the kind of thing that is wrong by eight bytes and
/// says nothing about it until a crash handler prints the wrong address.
///
/// So the two are compared directly: a C program prints what `<sys/stat.h>`
/// and the rest say, a Jai program prints what the front end made of our
/// declarations, and the two outputs have to be the same text.
#[test]
fn a_darwin_layout_is_what_the_c_compiler_says() {
  if !oj_types::Target::HOST.is_darwin() {
    eprintln!("skipping: these are Darwin's layouts and this is not Darwin");
    return;
  }
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to measure with");
    return;
  }

  let directory = tempfile::tempdir().expect("a temporary directory");
  let c_path = directory.path().join("measure.c");
  std::fs::write(&c_path, DARWIN_LAYOUT_C).expect("the C source is writable");
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

  let from_jai = build_and_run(DARWIN_LAYOUT_JAI).expect("the Jai side should build and run");

  assert!(
    from_c.lines().count() > 20,
    "the C side measured nothing:\n{from_c}"
  );
  // Line by line, so a mismatch names the member rather than the file.
  for (c, jai) in from_c.lines().zip(from_jai.lines()) {
    assert_eq!(
      c, jai,
      "\n--- C said ---\n{from_c}\n--- we say ---\n{from_jai}"
    );
  }
  assert_eq!(
    from_c.lines().count(),
    from_jai.lines().count(),
    "\n--- C said ---\n{from_c}\n--- we say ---\n{from_jai}"
  );
}

const DARWIN_LAYOUT_C: &str = r#"
/* `<ucontext.h>` is XSI, and `_DARWIN_C_SOURCE` puts back the BSD members
   that asking for XSI alone would hide — `d_namlen`, `pw_change` and the
   rest. Both together are what the modules are written against. */
#define _XOPEN_SOURCE 700
#define _DARWIN_C_SOURCE 1
#include <stdio.h>
#include <stddef.h>
#include <dirent.h>
#include <poll.h>
#include <pthread.h>
#include <pwd.h>
#include <signal.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <netinet/in.h>
#include <ucontext.h>

#define SIZE(name, type)          printf("%s %zu\n", name, sizeof(type))
#define OFF(name, type, member)   printf("%s %zu\n", name, offsetof(type, member))

int main(void) {
  SIZE("stat_t", struct stat);
  OFF("stat_t.st_mode", struct stat, st_mode);
  OFF("stat_t.st_ino", struct stat, st_ino);
  OFF("stat_t.st_mtime", struct stat, st_mtimespec);
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
  SIZE("pthread_mutexattr_t", pthread_mutexattr_t);

  /* The three the crash handler reads a saved register out of. */
  printf("UC_MCONTEXT %zu\n", offsetof(ucontext_t, uc_mcontext));
  printf("MCONTEXT_FP %zu\n",
    offsetof(struct __darwin_mcontext64, __ss)
      + offsetof(struct __darwin_arm_thread_state64, __fp));
  printf("MCONTEXT_PC %zu\n",
    offsetof(struct __darwin_mcontext64, __ss)
      + offsetof(struct __darwin_arm_thread_state64, __pc));
  return 0;
}
"#;

const DARWIN_LAYOUT_JAI: &str = r#"
#import "Basic";
#import "POSIX";
Crash :: #import "Runtime_Support_Crash_Handler";

off :: (base: *void, member: *void) -> s64 {
    return cast,no_check(s64) member - cast,no_check(s64) base;
}

main :: () {
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
    print("pthread_mutexattr_t %\n", size_of(pthread_mutexattr_t));

    print("UC_MCONTEXT %\n", Crash.UC_MCONTEXT);
    print("MCONTEXT_FP %\n", Crash.MCONTEXT_FP);
    print("MCONTEXT_PC %\n", Crash.MCONTEXT_PC);
}
"#;
