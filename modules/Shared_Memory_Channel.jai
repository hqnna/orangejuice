//
// A one-way byte channel between processes, over shared memory.
//
// One side creates the channel and the other opens it by name. Messages are
// written into a ring buffer with a length in front, so a reader takes whole
// messages rather than whatever happened to have arrived.
//
// The head and tail indices are the only shared mutable state, and they are
// moved with compare-and-swap — there is no mutex, because a mutex in shared
// memory dies with whichever process was holding it.
//
// One writer and one reader. More of either needs a design that can survive a
// participant being killed mid-write, which this deliberately is not.
//

#import "Basic";
#import "POSIX";
#import "Atomics";

// @Volatile: both processes map this, so its layout is the protocol.
Channel_Header :: struct {
    // Bumped when the layout changes, so a stale peer is told rather than
    // reading nonsense.
    version:  u32;
    capacity: u32;
    // Where the writer will put the next byte, and where the reader will take
    // it from. Both only ever increase; the ring position is the remainder.
    write_at: u64;
    read_at:  u64;
    // Set when either side goes away.
    closed:   u32;
    pad:      u32;
}

CHANNEL_VERSION :: 1;

Shared_Memory_Channel :: struct {
    name:     string;
    fd:       s32 = -1;
    header:   *Channel_Header;
    data:     *u8;
    capacity: s64;
    mapped:   s64;
    // Whether this process made it, and so should unlink it at the end.
    owner:    bool;
}

// Creates a channel, replacing any left behind by a process that died.
create :: (channel: *Shared_Memory_Channel, name: string, capacity: s64 = 65536) -> bool {
    c_name := to_c_string(name, temp);
    if !c_name  return false;

    // A stale one from a crashed run would otherwise be opened and trusted.
    shm_unlink(c_name);

    fd := shm_open(c_name, O_RDWR | O_CREAT | O_EXCL, 0x180);   // 0o600
    if fd < 0  return false;

    total := size_of(Channel_Header) + capacity;
    if ftruncate(fd, total) != 0 {
        close(fd);
        shm_unlink(c_name);
        return false;
    }

    if !attach(channel, fd, total, name)  return false;

    channel.header.version  = CHANNEL_VERSION;
    channel.header.capacity = cast(u32) capacity;
    channel.header.write_at = 0;
    channel.header.read_at  = 0;
    channel.header.closed   = 0;
    channel.capacity = capacity;
    channel.owner = true;
    return true;
}

// Opens a channel somebody else created.
connect :: (channel: *Shared_Memory_Channel, name: string) -> bool {
    c_name := to_c_string(name, temp);
    if !c_name  return false;

    fd := shm_open(c_name, O_RDWR, 0);
    if fd < 0  return false;

    // The header has to be mapped before the size it names can be known, so
    // this maps it twice: once to read the capacity, once for the whole thing.
    header_only := size_of(Channel_Header);
    peek := mmap(null, cast(u64) header_only, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if !peek  { close(fd); return false; }

    header := cast(*Channel_Header) peek;
    version := header.version;
    capacity := cast(s64) header.capacity;
    munmap(peek, cast(u64) header_only);

    if version != CHANNEL_VERSION || capacity <= 0 {
        close(fd);
        return false;
    }

    if !attach(channel, fd, header_only + capacity, name)  return false;
    channel.capacity = capacity;
    channel.owner = false;
    return true;
}

close_channel :: (channel: *Shared_Memory_Channel) {
    if channel.header  channel.header.closed = 1;
    if channel.header  munmap(channel.header, cast(u64) channel.mapped);
    if channel.fd >= 0 close(channel.fd);

    if channel.owner && channel.name.count {
        c_name := to_c_string(channel.name, temp);
        if c_name  shm_unlink(c_name);
    }
    channel.header = null;
    channel.data = null;
    channel.fd = -1;
}

is_closed :: (channel: *Shared_Memory_Channel) -> bool {
    if !channel.header  return true;
    return channel.header.closed != 0;
}

// How many bytes are waiting to be read.
bytes_available :: (channel: *Shared_Memory_Channel) -> s64 {
    if !channel.header  return 0;
    return cast(s64) (channel.header.write_at - channel.header.read_at);
}

// Writes one message. False when it would not fit in what is free, so a
// caller can wait rather than lose it.
send :: (channel: *Shared_Memory_Channel, message: string) -> bool {
    if !channel.header || is_closed(channel)  return false;

    needed := size_of(u32) + message.count;
    used := bytes_available(channel);
    if used + needed > channel.capacity  return false;

    length: u32 = cast(u32) message.count;
    // Both writes are placed relative to the *unadvanced* index, so the
    // payload has to be offset past the length rather than landing on it.
    write_bytes(channel, cast(*u8) *length, size_of(u32), 0);
    if message.count  write_bytes(channel, message.data, message.count, size_of(u32));

    // The reader must not see the length before the bytes it counts, so the
    // index only moves once everything is in place.
    memory_barrier();
    channel.header.write_at += cast(u64) needed;
    return true;
}

// The next whole message, in `allocator`. `ok` is false when there is not a
// complete one waiting — which is not an error, just nothing yet.
receive :: (channel: *Shared_Memory_Channel, allocator := context.allocator) -> (message: string, ok: bool) {
    if !channel.header  return "", false;

    available := bytes_available(channel);
    if available < size_of(u32)  return "", false;

    length: u32;
    read_bytes(channel, cast(*u8) *length, size_of(u32), 0);

    total := size_of(u32) + cast(s64) length;
    // The writer has moved its index but not finished the bytes yet.
    if available < total  return "", false;
    if cast(s64) length > channel.capacity  return "", false;

    result: string;
    if length {
        result.data = cast(*u8) alloc(cast(s64) length, allocator);
        if !result.data  return "", false;
        result.count = cast(s64) length;
        read_bytes(channel, result.data, result.count, size_of(u32));
    }

    memory_barrier();
    channel.header.read_at += cast(u64) total;
    return result, true;
}

#scope_module

attach :: (channel: *Shared_Memory_Channel, fd: s32, total: s64, name: string) -> bool {
    memory := mmap(null, cast(u64) total, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if !memory {
        close(fd);
        return false;
    }
    channel.fd = fd;
    channel.header = cast(*Channel_Header) memory;
    channel.data = cast(*u8) memory + size_of(Channel_Header);
    channel.mapped = total;
    channel.name = copy_string(name);
    return true;
}

// Both of these wrap at the end of the ring, which is why they are not a
// single `memcpy`.
write_bytes :: (channel: *Shared_Memory_Channel, source: *u8, count: s64, offset: s64) {
    start := cast(s64) ((channel.header.write_at + cast(u64) offset) % cast(u64) channel.capacity);
    first := min(count, channel.capacity - start);
    memcpy(channel.data + start, source, first);
    if first < count  memcpy(channel.data, source + first, count - first);
}

read_bytes :: (channel: *Shared_Memory_Channel, destination: *u8, count: s64, offset: s64) {
    start := cast(s64) ((channel.header.read_at + cast(u64) offset) % cast(u64) channel.capacity);
    first := min(count, channel.capacity - start);
    memcpy(destination, channel.data + start, first);
    if first < count  memcpy(destination + first, channel.data, count - first);
}
