//
// Operators a program declares itself, and `for` over a container of one's own
// (**L§6.7**, **L§7.14**).
//

#import "Basic";

Vec2 :: struct { x: float; y: float; }

operator + :: (a: Vec2, b: Vec2) -> Vec2 { return .{a.x + b.x, a.y + b.y}; }
operator - :: (a: Vec2, b: Vec2) -> Vec2 { return .{a.x - b.x, a.y - b.y}; }

// A scalar multiply that works either way round: `#symmetric` is what says so
// rather than writing the mirror overload.
operator * :: (v: Vec2, k: float) -> Vec2 #symmetric { return .{v.x * k, v.y * k}; }

operator == :: (a: Vec2, b: Vec2) -> bool { return a.x == b.x && a.y == b.y; }

// A ring buffer, and the `for_expansion` that makes `for` work over it: the
// loop body arrives as `Code` and is `#insert`ed into a loop of our own.
Ring :: struct (T: Type, N: int) {
    items: [N] T;
    start: int;
    count: int;
}

push :: (ring: *Ring, value: ring.T) {
    if ring.count == ring.N {
        ring.items[ring.start] = value;
        ring.start = (ring.start + 1) % ring.N;
        return;
    }
    ring.items[(ring.start + ring.count) % ring.N] = value;
    ring.count += 1;
}

operator [] :: (ring: Ring, index: int) -> ring.T {
    return ring.items[(ring.start + index) % ring.N];
}

for_expansion :: (ring: *Ring, body: Code, flags: For_Flags) #expand {
    for index: 0..ring.count - 1 {
        `it_index := index;
        `it := ring.items[(ring.start + index) % ring.N];
        #insert (break = break index, continue = continue index) body;
    }
}

main :: () {
    a := Vec2.{1.0, 2.0};
    b := Vec2.{0.5, 0.5};
    print("% % %\n", a + b, a - b, a * 2.0);
    print("%\n", 2.0 * a);
    print("% %\n", a == a, a == b);

    ring: Ring(int, 3);
    for 1..5  push(*ring, it);

    // The oldest two were pushed out, so what is left is 3, 4, 5.
    for ring  print("%:% ", it_index, it);
    print("\n");
    print("%\n", ring[0]);
}
