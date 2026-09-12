/* What Darwin has that no other Unix does, header for header against the
   Linux list in `../linux/headers.c`. kqueue is there where Linux has epoll
   and inotify, `sysctl` where Linux has `/proc` and `sysinfo`, the Mach clock
   where Linux has `clock_gettime` alone, and dyld, which is how a program
   finds the image it was loaded from. */
#define _DARWIN_C_SOURCE 1
#include <sys/event.h>
#include <sys/sysctl.h>
#include <sys/attr.h>
#include <sys/mount.h>
#include <mach/mach_time.h>
#include <mach-o/dyld.h>
#include <libproc.h>
#include <fcntl.h>
#include <unistd.h>
