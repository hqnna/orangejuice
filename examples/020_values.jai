//
// The built-in types and what `print` makes of them.
//
// `%` takes the next argument whatever its type is: the compiler passes an
// `Any`, which is a pointer to the value and a pointer to its `Type_Info`, so
// printing is a library routine rather than a compiler feature (**L§8.7**).
//

#import "Basic";

main :: () {
    n: int = -7;
    u: u16 = 65535;
    f: float = 1.5;
    d: float64 = 0.125;
    flag := true;
    text := "a string";

    print("% % % % % %\n", n, u, f, d, flag, text);

    // An integer literal has no type of its own until something needs it to
    // have one, so the same `1` is three different widths here.
    small: s8 = 1;
    wide: s64 = 1;
    print("% % %\n", size_of(type_of(small)), size_of(type_of(wide)), size_of(type_of(1)));

    // Division truncates on integers and does not on floats.
    print("% %\n", 7 / 2, 7.0 / 2.0);

    // A cast is written, never implied, where the conversion can lose
    // something (**L§5.12**).
    print("%\n", cast(int) 2.75);
}
