//
// TCP sockets.
//
// The reference vendors thousands of lines of generated binding for this;
// what a program needs is a listener, a connection, and reading and writing
// bytes. Everything reports failure rather than asserting: a refused
// connection is an ordinary thing to discover.
//
// Every send and receive loops, because a socket is free to move fewer bytes
// than asked for and a caller that ignores that has a bug which only appears
// under load.
//

#import "Basic";
#import "POSIX";
#import "String";

#load "generated.jai";

// `Socket` is declared in generated.jai as the file descriptor it is on this
// platform, which is what the reference does too.

is_open :: (s: Socket) -> bool {
    return s >= 0;
}

close_socket :: (s: *Socket) {
    if s.* >= 0  close(s.*);
    s.* = INVALID_SOCKET;
}

// ------------------------------------------------------------ listening ---

// A listening socket on `port`. `SO_REUSEADDR` is set so that restarting a
// server does not fail for the minute the old socket spends in TIME_WAIT.
listen_on_port :: (port: u16, backlog := 16) -> (listener: Socket, ok: bool) {
    result: Socket = INVALID_SOCKET;

    fd := socket(AF_INET, SOCK_STREAM, 0);
    if fd < 0  return result, false;

    reuse: s32 = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, *reuse, size_of(s32));

    address: sockaddr_in;
    address.sin_family = AF_INET;
    address.sin_port = htons(port);
    address.sin_addr.s_addr = 0;              // INADDR_ANY

    if bind(fd, cast(*sockaddr) *address, size_of(sockaddr_in)) != 0 {
        close(fd);
        return result, false;
    }
    if listen(fd, cast(s32) backlog) != 0 {
        close(fd);
        return result, false;
    }

    result = fd;
    return result, true;
}

// Waits for a connection. Blocks until one arrives.
accept_connection :: (listener: Socket) -> (connection: Socket, ok: bool) {
    result: Socket = INVALID_SOCKET;
    if listener < 0  return result, false;

    address: sockaddr_in;
    length: u32 = size_of(sockaddr_in);
    while true {
        fd := accept(listener, cast(*sockaddr) *address, *length);
        if fd >= 0 {
            result = fd;
            return result, true;
        }
        // A signal can interrupt the wait; that is not a failure.
        if errno() != EINTR  return result, false;
    }
    return result, false;
}

// The port a listener actually got, which is what a caller that asked for 0
// needs in order to tell anyone where to connect.
get_port :: (s: Socket) -> (port: u16, ok: bool) {
    if s < 0  return 0, false;
    address: sockaddr_in;
    length: u32 = size_of(sockaddr_in);
    if getsockname(s, cast(*sockaddr) *address, *length) != 0  return 0, false;
    return ntohs(address.sin_port), true;
}

// ----------------------------------------------------------- connecting ---

// Connects to a dotted-quad address. Name resolution is deliberately not here:
// it wants DNS, and a module this size should not pretend to do it.
connect_to :: (address_text: string, port: u16) -> (connection: Socket, ok: bool) {
    result: Socket = INVALID_SOCKET;

    c_text := to_c_string(address_text, temp);
    if !c_text  return result, false;

    raw := inet_addr(c_text);
    // INADDR_NONE, which is also 255.255.255.255 — a distinction nothing here
    // needs to make.
    if raw == 0xffff_ffff  return result, false;

    fd := socket(AF_INET, SOCK_STREAM, 0);
    if fd < 0  return result, false;

    target: sockaddr_in;
    target.sin_family = AF_INET;
    target.sin_port = htons(port);
    target.sin_addr.s_addr = raw;

    if connect(fd, cast(*sockaddr) *target, size_of(sockaddr_in)) != 0 {
        close(fd);
        return result, false;
    }
    result = fd;
    return result, true;
}

// ------------------------------------------------------------- transfer ---

// Sends the whole buffer, looping over short writes.
send_all :: (s: Socket, bytes: [] u8) -> bool {
    if s < 0  return false;
    sent := 0;
    while sent < bytes.count {
        n := send(s, bytes.data + sent, cast(u64) (bytes.count - sent), 0);
        if n < 0 {
            if errno() == EINTR  continue;
            return false;
        }
        if n == 0  return false;
        sent += n;
    }
    return true;
}

send_all :: (s: Socket, text: string) -> bool {
    bytes: [] u8;
    bytes.data = text.data;
    bytes.count = text.count;
    return send_all(s, bytes);
}

// Whatever has arrived, up to the size of `buffer`. Zero with `ok` true means
// the other end closed, which is not an error.
receive :: (s: Socket, buffer: [] u8) -> (count: s64, ok: bool) {
    if s < 0  return 0, false;
    while true {
        n := recv(s, buffer.data, cast(u64) buffer.count, 0);
        if n >= 0  return n, true;
        if errno() != EINTR  return 0, false;
    }
    return 0, false;
}

// Everything until the other end closes, in temporary storage.
receive_all :: (s: Socket) -> (data: string, ok: bool) {
    builder: String_Builder;
    builder.allocator = temp;

    buffer: [4096] u8;
    while true {
        n, ok := receive(s, buffer);
        if !ok  return "", false;
        if n == 0  break;
        piece: string;
        piece.data = buffer.data;
        piece.count = n;
        append(*builder, piece);
    }
    return builder_to_string(*builder,, allocator = temp), true;
}
