/* The Darwin surface, header for header against the glibc list in
   `../posix/headers.c`. What is missing from it is what Darwin does not have —
   epoll, inotify, signalfd, timerfd, eventfd, sendfile, prctl, sysinfo — and
   what is added is what it has instead: kqueue, sysctl, mach time, and the
   dyld call that says where the running image came from.

   `_XOPEN_SOURCE` is what `<ucontext.h>` insists on; `_DARWIN_C_SOURCE` puts
   back the BSD members that asking for XSI alone would hide. */
#define _XOPEN_SOURCE 700
#define _DARWIN_C_SOURCE 1
#include <dirent.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <grp.h>
#include <libgen.h>
#include <locale.h>
#include <poll.h>
#include <pthread.h>
#include <pwd.h>
#include <sched.h>
#include <semaphore.h>
#include <setjmp.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <termios.h>
#include <time.h>
#include <ucontext.h>
#include <unistd.h>
#include <utime.h>
#include <sys/event.h>
#include <sys/file.h>
#include <sys/ioctl.h>
#include <sys/ipc.h>
#include <sys/mman.h>
#include <sys/msg.h>
#include <sys/resource.h>
#include <sys/select.h>
#include <sys/sem.h>
#include <sys/shm.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <sys/syscall.h>
#include <sys/sysctl.h>
#include <sys/time.h>
#include <sys/times.h>
#include <sys/types.h>
#include <sys/uio.h>
#include <sys/un.h>
#include <sys/utsname.h>
#include <sys/wait.h>
#include <fnmatch.h>
#include <execinfo.h>
#include <glob.h>
#include <net/if.h>
#include <netinet/in.h>
#include <mach/mach_time.h>
#include <mach-o/dyld.h>
