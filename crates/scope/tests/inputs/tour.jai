#import "Widget";
Helper :: #import "Widget";

MAX :: 4;

#if OS == .LINUX {
    PLATFORM :: "linux";
} else {
    PLATFORM :: "other";
}

using Color :: enum {
    RED;
    GREEN;
}

Point :: struct {
    x, y: float;

    Nested :: struct {
        depth: int;
    }
}

Pair :: struct (Key: Type, Value: Type) {
    key:   Key;
    value: Value;
}

#placeholder GENERATED;

#scope_file

private_state: int;

distance :: (a: Point, b: Point) -> float {
    dx := a.x - b.x;
    dy := a.y - b.y;
    return dx * dx + dy * dy;
}

sum :: (values: [] int) -> total: int {
    total = 0;
    for values {
        total += it;
        if it_index > MAX  break;
    }
    return total;
}

#scope_export

main :: () {
    p: Point;
    shade := RED;
    private_state = sum(.[1, 2, 3]);
    distance(p, p);
    widget();
    Helper.internal();
    use(PLATFORM);
}

use :: (s: string) {}
