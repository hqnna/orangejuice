//
// How a program says something went wrong: several return values, `assert`,
// and the stack trace a crash prints.
//

#import "Basic";

// The usual shape: the value, and a flag saying whether to believe it. The
// caller cannot use the value without seeing the flag.
parse_int :: (text: string) -> (value: int, ok: bool) {
    if !text.count  return 0, false;

    result := 0;
    negative := text[0] == #char "-";
    start := ifx negative then 1 else 0;
    if start >= text.count  return 0, false;

    for i: start..text.count - 1 {
        byte := text[i];
        if byte < #char "0" || byte > #char "9"  return 0, false;
        result = result * 10 + (byte - #char "0");
    }
    return ifx negative then -result else result, true;
}

main :: () {
    for text: string.["42", "-7", "", "12x"] {
        value, ok := parse_int(text);
        if ok  print("\"%\" -> %\n", text, value);
        else   print("\"%\" is not a number\n", text);
    }

    // `assert` takes a format string and its arguments, so what failed is in
    // the message rather than only in the line number.
    n := 5;
    assert(n > 0, "n should be positive, but is %", n);
    print("assertion held\n");

    // A `#must` return may not be dropped on the floor: the compiler rejects
    // a call that ignores it.
    keep := must_be_used();
    print("%\n", keep);
}

must_be_used :: () -> int #must {
    return 99;
}
