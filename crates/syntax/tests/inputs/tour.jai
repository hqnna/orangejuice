// A tour of the grammar of L§18, for the `oj dump ast` snapshots.
#import "Basic";
Math :: #import "Math"(FAST = true)(ENABLE_ASSERT = false);
#load "other.jai";
libc :: #library,system "libc";

#scope_module

Mode :: enum u8 #specified { QUIET :: 0; LOUD :: 1; }
Flags :: enum_flags @Hi { A; B :: 0x40; }

Vector2 :: struct {
    x, y: float;
}

Entity :: struct (T: Type, N: s64 = 3) #no_padding {
    #as using position: Vector2;
    name := "unnamed";
    values: [N] T;
    handle: u32 #align 4;
    union { i: s32; f: float; }
    #place x;
    overlapping: float;
    Nested :: struct { k: int; }
    K :: 16;
    #if OS == .LINUX {
        fd: s32;
    }
} @Serialized

TAU :: 6.2831853071795864769;
NAMES :: string.["a", "b"];
ORIGIN :: Vector2.{0, 0};
USAGE :: #string DONE
usage: tour [-quiet]
DONE

environ: *u8 #elsewhere libc "environ";
#no_reset counter: int;
#program_export "oj_start" start :: () {
}

sum :: (x, total) => x + total;
square :: x => x * x;

clamp :: inline (value: $T, low: T, high: T) -> T #modify {
    return true;
} {
    if value < low then return low;
    if value > high then return high;
    return value;
}

find :: (haystack: [] $T/interface Comparable, needle: T, $$exact := true) -> (index: s64, found: bool #must) {
    for < * haystack {
        if it.* == needle then return it_index, true;
    }
    return -1, false;
}

printf :: (format: *u8, args: ..Any) -> s32 #foreign libc "printf";
memcpy :: (dst: *void, src: *void, count: s64) #intrinsic "llvm.memcpy";

operator + :: (a: Vector2, b: Vector2) -> Vector2 #symmetric {
    return .{a.x + b.x, a.y + b.y};
}

loop :: (n: int, body: Code) #expand {
    for 1..n {
        #insert(break=break it, continue=continue it) body;
    }
}

for_expansion :: (container: *Entity(int, 3), body: Code, flags: For_Flags) #expand {
    for *=cast(bool)(flags & .POINTER) entry, index: container.values {
        `it := entry;
        `it_index := index;
        #insert body;
    }
}

main :: () {
    entity: Entity(int, 3);
    p := *entity;
    total, count := 0, 0;
    total, count = count, total;
    ok:, value = compute();

    while running := next() {
        defer close(running);
        if running == {
            case .QUIET;
                total += 1;
                #through;
            case .LOUD;
                total <<= 2;
            case;
                break running;
        }
    }

    for index: 0..9 {
        if index & 1 then continue;
        total += index;
    }

    for :named v, k: table {
        remove v;
    }

    name := ifx total > 3 then "many" else "few";
    style := ifx total {
        log("counting");
        "some";
    } else {
        "none";
    }

    bytes := cast,trunc(u8) total;
    address := cast(*u8, p, force);
    view := p.(*Vector2).*;
    auto: float = xx total;

    push_context,defer_pop new_context;
    push_context .{allocator = temp} {
        print("hello %, % %\n", name, style, ..NAMES,, allocator = temp);
    }

    #asm { mov.64 a:, 10; add a, a; }
    #bytes .[0x90, 0x90];
    #run print("compile time");
    #assert size_of(Vector2) == 8 "Vector2 must be two floats";
    #insert -> string {
        return "helper :: () {}";
    }
    x := #run,stallable compute();
    code :: #code,typed total + 1;
    where :: #location(#this);
    who :: #procedure_name();
    here :: #file;
    exists :: #exists(Vector2.x);
    handle := #bake_arguments printf(format = "%");
    which := #procedure_of_call clamp(1, 0, 2);
    is_ct := #compile_time;
    ctx: #Context;
}
