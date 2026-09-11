//
// Loaded by `140_modules.jai`. A `#load`ed file shares the loader's scope, so
// this needs no imports of its own — the String_Builder is the one Basic put
// there for the file that loaded this one.
//

EXCLAIM :: "!";

shout :: (text: string) -> string {
    builder: String_Builder;
    for 0..text.count - 1 {
        byte := text[it];
        if byte >= #char "a" && byte <= #char "z"  byte -= 32;
        append(*builder, byte);
    }
    append(*builder, EXCLAIM);
    return builder_to_string(*builder,, allocator = temp);
}
