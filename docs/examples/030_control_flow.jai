//
// `if`, `while`, `for`, and the `if x == { case … }` that replaces `switch`.
//

#import "Basic";

classify :: (n: int) -> string {
    // A case does not fall through, so no `break` is written to stop it; a
    // `#through` is what asks for the other behaviour (**L§6.5**).
    if n == {
        case 0;  return "zero";
        case 1;  return "one";
        case;    return ifx n < 0 then "negative" else "many";
    }
}

main :: () {
    for n: -1..2  print("% is %\n", n, classify(n));

    // `for` over a range counts `it`; over a container it also counts
    // `it_index` (**L§6.4**).
    total := 0;
    for 1..10  total += it;
    print("sum 1..10 = %\n", total);

    names := string.["ada", "grace", "barbara"];
    for names  print("%: %\n", it_index, it);

    // A reverse iteration is written `<`, not by counting down by hand.
    for < names  print("% ", it);
    print("\n");

    // `break` and `continue` may name the loop they mean — by its iterator,
    // since that is the name the loop already has (**L§6.6**).
    found := 0;
    for a: 1..5 {
        for b: 1..5 {
            if a * b == 12 { found = a * 10 + b; break a; }
        }
    }
    print("first product of 12: %\n", found);

    countdown := 3;
    while countdown > 0 {
        print("% ", countdown);
        countdown -= 1;
    }
    print("liftoff\n");
}
