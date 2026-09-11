
// ---------------------------------------------------------- hand-written ---
//
// What no generator can read out of a header: C's macros, and the handful of
// conveniences the reference builds on top of the raw calls.

// On Linux a socket is a file descriptor; Windows has a handle type of its
// own, which is why this is a name rather than `s32` everywhere it is used.
Socket :: s32;

INVALID_SOCKET     :: cast,trunc(Socket) (~0);
SOCKET_ERROR       :: -1;
SOCKET_WOULDBLOCK  :: EWOULDBLOCK;

// ------------------------------------------------------------- fd_set ---
//
// `select` takes a bitmap of descriptors. C does the bit arithmetic in macros;
// these are the same arithmetic, with the bounds checked so that a descriptor
// past the end of the set is ignored rather than corrupting memory.

FD_BITS_ELEMENT_TYPE :: type_of(fd_set.fds_bits[0]);
BITS_PER_FD_SET_MASK :: size_of(FD_BITS_ELEMENT_TYPE) * 8;

check_fd_set :: (fd: s32, set: *fd_set) -> bool {
    if !set  return false;
    return fd >= 0 && fd / BITS_PER_FD_SET_MASK < set.fds_bits.count;
}

FD_SET :: (fd: s32, set: *fd_set) {
    if !check_fd_set(fd, set)  return;
    set.fds_bits[fd / BITS_PER_FD_SET_MASK] |= cast,no_check(FD_BITS_ELEMENT_TYPE) (cast(u64) 1 << (cast(u64) fd % BITS_PER_FD_SET_MASK));
}

FD_CLR :: (fd: s32, set: *fd_set) {
    if !check_fd_set(fd, set)  return;
    set.fds_bits[fd / BITS_PER_FD_SET_MASK] &= ~cast,no_check(FD_BITS_ELEMENT_TYPE) (cast(u64) 1 << (cast(u64) fd % BITS_PER_FD_SET_MASK));
}

FD_ISSET :: (fd: s32, set: *fd_set) -> bool {
    if !check_fd_set(fd, set)  return false;
    mask := cast,no_check(FD_BITS_ELEMENT_TYPE) (cast(u64) 1 << (cast(u64) fd % BITS_PER_FD_SET_MASK));
    return (set.fds_bits[fd / BITS_PER_FD_SET_MASK] & mask) != 0;
}

// ----------------------------------------------------------- ancillary ---
//
// Control messages travel in a buffer as a chain of headers, each padded out
// to a pointer boundary. These walk it.

CMSG_ALIGN :: (length: u64) -> u64 {
    return (length + size_of(u64) - 1) & ~cast(u64) (size_of(u64) - 1);
}

// How much buffer a payload of `length` needs, header and padding included.
CMSG_SIZE :: (length: u64) -> u64 {
    return CMSG_ALIGN(cast(u64) size_of(cmsghdr)) + length;
}

// What goes in `cmsg_len`: the header is counted unpadded, since the padding
// belongs to whatever follows.
CMSG_LEN :: (length: u64) -> u64 {
    return CMSG_ALIGN(cast(u64) size_of(cmsghdr)) + length;
}

// The payload just past a header.
CMSG_DATA :: (header: *cmsghdr) -> *u8 {
    return cast(*u8) header + CMSG_ALIGN(cast(u64) size_of(cmsghdr));
}

// The first header in a message's control buffer, or null when there is none.
CMSG_FIRSTHDR :: (message: *msghdr) -> *cmsghdr {
    if !message || message.msg_controllen < cast(u64) size_of(cmsghdr)  return null;
    return cast(*cmsghdr) message.msg_control;
}

// The well-known addresses, which C declares as globals.
in6addr_any      : in6_addr #elsewhere libc;
in6addr_loopback : in6_addr #elsewhere libc;

// `if_nameindex` is a struct and a function in C, and Jai has one namespace
// for both. The struct keeps the name, so the call gets the trailing
// underscore the reference gives it.
if_nameindex_ :: () -> *if_nameindex #foreign libc "if_nameindex";

// ------------------------------------------------------- the conveniences ---
//
// What the reference builds on top of the raw calls. On Linux most of these
// have nothing platform-specific to do; they exist so that a program written
// against the module runs on Windows too, where they do.

// Winsock needs starting; Unix does not.
socket_init :: () -> bool {
    return true;
}

get_last_socket_error :: () -> OS_Error_Code {
    return cast(OS_Error_Code) errno();
}

// Turns the descriptor's non-blocking flag on or off, leaving the rest alone.
set_blocking :: (s: Socket, blocking: bool) -> bool {
    value := fcntl(s, F_GETFL, 0);
    if value < 0  return false;

    if blocking  value &= ~O_NONBLOCK;
    else         value |= O_NONBLOCK;
    return fcntl(s, F_SETFL, value) == 0;
}

// Keep-alive probes on a connection, and how patient they are. A zero leaves
// that particular timer at the system default.
set_keepalive :: (s: Socket, keepalive: bool, idle_seconds_before_keepalive: s32 = 0, keepalive_interval_seconds: s32 = 0, num_keepalive_attempts: s32 = 0) -> bool {
    on: s32 = ifx keepalive then 1 else 0;
    if setsockopt(s, SOL_SOCKET, SO_KEEPALIVE, *on, size_of(s32)) != 0  return false;
    if !keepalive  return true;

    if idle_seconds_before_keepalive {
        value := idle_seconds_before_keepalive;
        if setsockopt(s, xx IPPROTO.TCP, TCP_KEEPIDLE, *value, size_of(s32)) != 0  return false;
    }
    if keepalive_interval_seconds {
        value := keepalive_interval_seconds;
        if setsockopt(s, xx IPPROTO.TCP, TCP_KEEPINTVL, *value, size_of(s32)) != 0  return false;
    }
    if num_keepalive_attempts {
        value := num_keepalive_attempts;
        if setsockopt(s, xx IPPROTO.TCP, TCP_KEEPCNT, *value, size_of(s32)) != 0  return false;
    }
    return true;
}

// Closes and marks the handle invalid, so that a double close is a no-op
// rather than a close of whatever descriptor was reused in the meantime.
close_and_reset :: (s: *Socket, loc := #caller_location) {
    if s.* == INVALID_SOCKET  return;
    if close(s.*) != 0 {
        log_error("%:%,%: Could not close socket descriptor %: %\n",
                  loc.fully_pathed_filename, loc.line_number, loc.character_number, s.*, errno());
    }
    s.* = INVALID_SOCKET;
}

// "1.2.3.4:80", in temporary storage.
to_string :: (addr: sockaddr_in) -> string {
    builder: String_Builder;
    builder.allocator = temp;
    append_address(*builder, addr.sin_addr.s_addr);
    print_to_builder(*builder, ":%", ntohs(addr.sin_port));
    return builder_to_string(*builder,, allocator = temp);
}

// A dotted quad. The address is in network order, so the first octet is the
// low byte.
append_address :: (builder: *String_Builder, address: u32) {
    host := ntohl(address);
    print_to_builder(*builder.*, "%.%.%.%",
                     (host >> 24) & 0xff, (host >> 16) & 0xff, (host >> 8) & 0xff, host & 0xff);
}

// `accept` over IPv6, handing back the peer's address as well.
accept_v6 :: (s: Socket) -> (accepted: Socket, address: sockaddr_in6) {
    address: sockaddr_in6;
    length: socklen_t = size_of(sockaddr_in6);
    accepted := accept(s, cast(*sockaddr) *address, *length);
    return accepted, address;
}
