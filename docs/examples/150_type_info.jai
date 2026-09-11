//
// The type table: what the compiler knows about a type, available to the
// running program (**L§17**).
//

#import "Basic";

Config :: struct {
    name:    string;
    retries: int = 3;
    verbose: bool;
}

Level :: enum { LOW; MEDIUM; HIGH; }

// A procedure that works on any struct by reading its `Type_Info`, which is
// how serializers and inspectors are written.
describe :: (value: Any) {
    info := value.type;
    print("% (% bytes)\n", info.type, info.runtime_size);
    if info.type != .STRUCT  return;

    record := cast(*Type_Info_Struct) info;
    for record.members {
        print("  % : % at offset %\n", it.name, it.type.type, it.offset_in_bytes);
    }
}

main :: () {
    c := Config.{name = "server", verbose = true};
    describe(c);
    describe(42);

    // `type_info` is the same thing asked for by type rather than by value.
    level := type_info(Level);
    for level.names  print("% = %\n", it, level.values[it_index]);

    // An `Any` carries the value and its type together, which is what lets
    // `print` handle anything at all.
    values: [3] Any;
    values[0] = 1;
    values[1] = "two";
    values[2] = 3.0;
    for values  print("% ", it);
    print("\n");
}
