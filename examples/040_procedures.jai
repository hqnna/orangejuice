//
// Procedures: defaults, named arguments, several return values, and overloads.
//

#import "Basic";

// A default makes the argument optional; a named argument at the call site
// says which parameter it is for, so the order stops mattering (**L§7.2**).
greet :: (name: string, greeting := "Hello", punctuation := ".") -> string {
    return tprint("%1, %2%3", greeting, name, punctuation);
}

// Several return values are returned, not packed into a struct. The names are
// for the reader and for a caller that wants to be explicit (**L§7.4**).
divide :: (numerator: int, denominator: int) -> (quotient: int, remainder: int, ok: bool) {
    if denominator == 0  return 0, 0, false;
    return numerator / denominator, numerator % denominator, true;
}

// Two procedures may share a name when their parameters tell them apart
// (**L§7.6**).
describe :: (n: int)    -> string { return tprint("the integer %", n); }
describe :: (s: string) -> string { return tprint("the string \"%\"", s); }

main :: () {
    print("%\n", greet("world"));
    print("%\n", greet("world", greeting = "Good morning"));
    print("%\n", greet(punctuation = "!", name = "everyone"));

    q, r, ok := divide(17, 5);
    print("17/5 = % remainder %, ok=%\n", q, r, ok);

    // A caller may take fewer values than are returned; the rest are dropped.
    only_quotient := divide(17, 5);
    print("just the quotient: %\n", only_quotient);

    _, _, divided := divide(1, 0);
    print("dividing by zero worked: %\n", divided);

    print("%\n", describe(42));
    print("%\n", describe("42"));
}
