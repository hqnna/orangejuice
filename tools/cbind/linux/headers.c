#define _GNU_SOURCE 1
#include <sys/epoll.h>
#include <sys/inotify.h>
#include <sys/sysinfo.h>
#include <sys/stat.h>
#include <linux/input.h>
#include <linux/io_uring.h>
#include <fcntl.h>
#include <unistd.h>
