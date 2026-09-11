//
// `#import` brings a module in; `#load` splices a file into this one. What a
// module exports is decided by `#scope_export` and `#scope_module` inside it
// (**L§11**).
//

#import "Basic";

// A named import keeps the module's names behind a prefix, which is how two
// modules that both declare `min` are used in one file.
Math   :: #import "Math";
Sort   :: #import "Sort";
String :: #import "String";

// `#load` is not an import: the file becomes part of *this* one, sharing its
// scope. `support/shout.jai` declares `EXCLAIM` and `shout`.
#load "support/shout.jai";

main :: () {
    print("% % %\n", Math.PI, Math.floor(2.7), Math.sqrt(16.0));
    // `max` is Basic's, and it is polymorphic: the same one serves both.
    print("% %\n", max(3, 9), max(1.5, 0.5));

    numbers := int.[5, 2, 9, 1];
    sorted := numbers;
    Sort.quick_sort(sorted, (a: int, b: int) -> int { return a - b; });
    print("%\n", sorted);

    print("%\n", String.join("a", "b", "c",, allocator = temp));

    // From the loaded file, as though it had been written here.
    print("%\n", shout("ready"));
    print("%\n", EXCLAIM);

    reset_temporary_storage();
}
