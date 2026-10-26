#include <setjmp.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>

enum { OJ_GUARD_EXITED = 1000 };

static __thread sigjmp_buf *oj_guard_target;
static __thread int oj_guard_exit_status;
static __thread void *oj_guard_fault_address;

static void oj_guard_handler(int sig, siginfo_t *info, void *context) {
  (void)context;
  sigjmp_buf *target = oj_guard_target;
  if (target) {
    oj_guard_target = 0;
    oj_guard_fault_address = info ? info->si_addr : 0;
    siglongjmp(*target, sig);
  }
  signal(sig, SIG_DFL);
  raise(sig);
}

void oj_guard_exit(int status) {
  sigjmp_buf *target = oj_guard_target;
  if (target) {
    oj_guard_target = 0;
    oj_guard_exit_status = status;
    siglongjmp(*target, OJ_GUARD_EXITED);
  }
  exit(status);
}

int oj_guarded_call(void (*entry)(void *), void *argument, int *exit_status, void **fault_address) {
  static const int signals[] = {SIGSEGV, SIGBUS, SIGFPE, SIGILL};
  enum { COUNT = sizeof signals / sizeof signals[0] };
  struct sigaction previous_actions[COUNT];
  struct sigaction action;
  memset(&action, 0, sizeof action);
  action.sa_sigaction = oj_guard_handler;
  action.sa_flags = SA_SIGINFO | SA_ONSTACK | SA_NODEFER;
  sigemptyset(&action.sa_mask);
  for (int i = 0; i < COUNT; i++) sigaction(signals[i], &action, &previous_actions[i]);

  sigjmp_buf target;
  sigjmp_buf *outer = oj_guard_target;
  int caught = sigsetjmp(target, 1);
  if (caught == 0) {
    oj_guard_target = &target;
    entry(argument);
  }
  oj_guard_target = outer;
  for (int i = 0; i < COUNT; i++) sigaction(signals[i], &previous_actions[i], 0);

  if (caught == OJ_GUARD_EXITED) *exit_status = oj_guard_exit_status;
  if (caught != 0 && caught != OJ_GUARD_EXITED) *fault_address = oj_guard_fault_address;
  return caught;
}
