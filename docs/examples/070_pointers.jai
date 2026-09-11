//
// Pointers, `defer`, and where memory comes from.
//

#import "Basic";

Node :: struct {
    value: int;
    next:  *Node;
}

// `*T` is a pointer; `*x` takes an address and `x.*` reads through one. A
// pointer is dereferenced by `.` without writing `.*` first (**L§5.6**).
bump :: (n: *int) {
    n.* += 1;
}

main :: () {
    n := 41;
    bump(*n);
    print("%\n", n);

    // `new` allocates through the context allocator; `free` gives it back.
    // `defer` runs at the end of the enclosing block, which is what keeps the
    // free beside the allocation rather than at the end of the procedure
    // (**L§6.8**).
    head := New(Node);
    defer free(head);
    head.value = 1;

    second := New(Node);
    defer free(second);
    second.value = 2;
    head.next = second;

    at := head;
    while at {
        print("% ", at.value);
        at = at.next;
    }
    print("\n");

    // Defers run in reverse order, and they run whatever way the block is
    // left.
    {
        defer print("third\n");
        defer print("second\n");
        print("first\n");
    }

    // Temporary storage is a bump allocator that a program resets when it
    // likes, which is what `tprint` allocates from.
    message := tprint("% and %", "this", "that");
    print("%\n", message);
    reset_temporary_storage();
}
