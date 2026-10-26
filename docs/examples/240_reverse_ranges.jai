//
// Counting down: `for <` over a range visits it from the far end to the near
// one (**L§6.6**). This is what the reference has done since 0.2.019, which
// made the `#v2` that used to ask for it the default; `#v2` is still
// accepted and changes nothing.
//

#import "Basic";

main :: () {
    for < 1..5  print("% ", it);
    print("\n");

    // Exactly the numbers the forward loop visits, in the other order.
    for #v2 < 1..5  print("% ", it);
    print("\n");

    // A range that is empty forwards is empty backwards too.
    visited := 0;
    for < 5..1  visited += 1;
    print("visited %\n", visited);

    // The usual reason to want it: walking a buffer from its end, which is
    // how a copy into an overlapping one stays correct.
    buffer := u8.[1, 2, 3, 4, 5, 0, 0];
    tail := 5;
    for < 0..tail-1  buffer[it + 2] = buffer[it];
    print("%\n", buffer);

    // An unsigned counter stops at zero rather than wrapping past it.
    n: u32 = 3;
    for < 0..n  print("% ", it);
    print("\n");
}
