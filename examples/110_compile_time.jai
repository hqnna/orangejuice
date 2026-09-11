//
// `#run` executes at compile time, in the same compiler that is compiling the
// program (**L§14**). What it returns is a constant in the executable.
//

#import "Basic";

factorial :: (n: int) -> int {
    if n <= 1  return 1;
    return n * factorial(n - 1);
}

// The call runs while the program is being compiled; what lands in the
// executable is the number.
TWELVE_FACTORIAL :: #run factorial(12);

// A `#run` may build a table rather than compute one number.
SQUARES :: #run -> [8] int {
    table: [8] int;
    for * table  it.* = it_index * it_index;
    return table;
};

// `#insert` splices source the compiler produced. Here a `#run` writes it.
accessors :: () -> string {
    builder: String_Builder;
    for name: string.["x", "y", "z"] {
        print_to_builder(*builder, "get_%1 :: (v: Vec3) -> float { return v.%1; }\n", name);
    }
    return builder_to_string(*builder);
}

Vec3 :: struct { x: float; y: float; z: float; }
#insert #run accessors();

main :: () {
    print("12! = %\n", TWELVE_FACTORIAL);
    print("%\n", SQUARES);

    v := Vec3.{1.0, 2.0, 3.0};
    print("% % %\n", get_x(v), get_y(v), get_z(v));

    // `#assert` is checked while compiling and never reaches the executable.
    #assert size_of(Vec3) == 12;

    // The compiler tells a program about itself.
    print("compiled at %:%\n", #location().fully_pathed_filename.count > 0, #location().line_number);
}
