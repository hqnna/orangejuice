//
// A metaprogram: a program that compiles another one (**C§3**).
//
// This is not a build script run beside the compiler — it *is* compiled by the
// compiler, at compile time, and it drives the compilation from inside. The
// `#run` below happens while this file is being compiled; everything it does
// happens before this program's own `main` exists.
//

#import "Basic";
#import "Compiler";
String :: #import "String";

#run inspect();

// The program the metaprogram compiles, handed over as a string rather than
// read from a file so that the example is one file.
PROGRAM :: #string DONE
#import "Basic";

Point :: struct { x: int; y: int; }

distance_squared :: (a: Point, b: Point) -> int {
    dx := a.x - b.x;
    dy := a.y - b.y;
    return dx * dx + dy * dy;
}

main :: () {
    print("%\n", distance_squared(.{0, 0}, .{3, 4}));
}
DONE

inspect :: () {
    // A workspace is a compilation of its own, with its own options and its
    // own message stream.
    w := compiler_create_workspace("Inspected");

    options := get_build_options(w);
    options.output_type = .NO_OUTPUT;
    set_build_options(options, w);

    // Nothing of the workspace's own output should be printed here.
    compiler_begin_intercept(w);
    add_build_string(PROGRAM, w);

    files: [..] string;
    procedures: [..] string;
    defer array_free(files);
    defer array_free(procedures);

    while true {
        message := compiler_wait_for_message();
        if !message  break;

        if message.kind == {
            case .FILE;
                file := cast(*Message_File) message;
                // Only the string that was added: the modules behind it are
                // not what this example is about.
                if file.from_a_string  array_add(*files, "a string");

            case .TYPECHECKED;
                batch := cast(*Message_Typechecked) message;
                for batch.procedure_headers {
                    header := it.expression;
                    if !header.name  continue;
                    // Every module the workspace pulled in typechecks too, so
                    // what came from the string is picked out by the file each
                    // node was written in.
                    file := header.enclosing_load;
                    if file && file.from_a_string  array_add(*procedures, header.name);
                }

            case .COMPLETE;
                break;
        }
    }
    compiler_end_intercept(w);

    print("compiled % source string(s)\n", files.count);

    // Sorted, because the order typechecking finishes in is the compiler's
    // business and not a thing to depend on.
    for 0..procedures.count - 1 {
        for 0..procedures.count - 2 {
            if String.compare(procedures[it], procedures[it + 1]) > 0 {
                procedures[it], procedures[it + 1] = procedures[it + 1], procedures[it];
            }
        }
    }
    print("procedures it declared: %\n", procedures);
}

main :: () {
    print("and this program's own main ran afterwards\n");
}
