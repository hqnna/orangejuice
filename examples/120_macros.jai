//
// A macro is spliced into the caller rather than called, so it can see the
// caller's locals and return from the caller's procedure (**L§7.13**).
//

#import "Basic";

// `` `x `` names something in the *caller's* scope. This macro declares `sum`
// there, so the code after it can read it.
accumulate :: (values: [] int) #expand {
    `sum = 0;
    for values  `sum += it;
}

// `` `return `` returns from the procedure the macro was expanded into, which
// is how an early-out is written as a macro.
require :: (condition: bool, message: string) #expand {
    if !condition {
        print("rejected: %\n", message);
        `return -1;
    }
}

checked_total :: (values: [] int) -> int {
    require(values.count > 0, "no values");
    require(values.count < 10, "too many values");

    sum: int;
    accumulate(values);
    return sum;
}

// `Code` is a quoted expression. A `$c: Code` bakes what the call site wrote,
// and `#insert` splices it where it is written.
twice :: ($c: Code) #expand {
    #insert c;
    #insert c;
}

main :: () {
    print("%\n", checked_total(int.[1, 2, 3]));
    print("%\n", checked_total(int.[]));

    count := 0;
    twice(#code { count += 5; });
    print("%\n", count);

    // A `#code` may be held in a constant and passed on.
    greeting :: #code print("hello twice\n");
    twice(greeting);
}
