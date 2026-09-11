//
// A path as an array of words rather than as a string.
//
// Most programs manipulate paths with string operations, which is how a
// trailing slash or a `..` turns into a bug. These routines give the thing a
// shape: a header (`C:` or `https:`), a count of leading slashes, the words
// between the separators, and whether there was a slash at the end.
//
// Nothing here copies. `parse_path` points into the string it was given and
// `reduce_path` into its input's strings, so a caller should push a temporary
// or pool allocator before using them and call `copy_to_permanent` for a path
// it means to keep.
//
// Because nothing owns anything, one `Path` may be assigned to another with
// plain `=`.
//

Path :: struct {
    // A drive letter on Windows, or `https:`, or anything else ending in a
    // colon before the first slash.
    header_string: string;
    number_of_leading_slashes: int;

    trailing_slash: bool;

    // `/a/b/hello.txt` is `["a", "b", "hello.txt"]`.
    words: [..] string;
}

// `reduce` folds `.` and `..` away. It is on by default, which is usually what
// is wanted and is occasionally wrong: `a/b/../c` is not `a/c` when `b` is a
// symlink.
parse_path :: (_s: string, reduce := true) -> Path {
    s := _s;
    result: Path;

    // A colon before the first slash is a header.
    prefix_length: int;
    for i: 0..s.count - 1 {
        if s[i] == #char "/"  break;
        if s[i] == #char ":"  prefix_length = i + 1;
    }

    if prefix_length {
        result.header_string = s;
        result.header_string.count = prefix_length;
        advance(*s, prefix_length);
    }

    while s && s[0] == #char "/" {
        result.number_of_leading_slashes += 1;
        advance(*s, 1);
    }

    words := split(s, "/");
    if words && words[words.count - 1] == "" {
        result.trailing_slash = true;
        words.count -= 1;
    }

    result.words = resizable(words, false);
    result.words.allocator = temp;

    if reduce  return reduce_path(result);
    return result;
}

// `.` and the empty word go away, and `..` takes the word before it with it —
// unless there is nothing before it, in which case it stays, since the path
// backs up past where it started.
reduce_path :: (path: Path) -> Path {
    result := path;

    words: [..] string;
    words.allocator = temp;
    for path.words {
        if it == "." {
        } else if it == ".." {
            if words.count  words.count -= 1;
            else            array_add(*words, it);
        } else if it == "" {
        } else {
            array_add(*words, it);
        }
    }

    result.words = words;

    return result;
}

// Whether `words` appears in the path as a run, and where it starts.
path_contains :: (path: Path, words: [] string) -> (contains: bool, position: s32) {
    for outer_loop, i: path.words {
        if i + words.count > path.words.count  break;

        for word, j: words {
            if path.words[i + j] != word  continue outer_loop;
        }

        return true, cast(s32) i;
    }

    return false, 0;
}

// Back to a string. `max_words` stops early, for naming a prefix of the path.
path_to_string :: (path: Path, max_words := -1) -> string {
    builder: String_Builder;
    init_string_builder(*builder);

    append(*builder, path.header_string);

    for 1..path.number_of_leading_slashes  append(*builder, "/");

    for path.words {
        if it_index == max_words  break;

        append(*builder, it);
        if it_index != path.words.count - 1 || path.trailing_slash  append(*builder, "/");
    }

    return builder_to_string(*builder);
}

// How far down the path something is really there. `how_much_exists` is a
// count of words, not an index, so zero means nothing at all.
how_much_of_path_exists_on_the_local_filesystem :: (path: Path) -> (fully_exists: bool, how_much_exists: s32) {
    push_allocator(temp);
    path_so_far := "";

    last_existing_index: s32 = -1;
    for path.words {
        if it == "." || it == "" {
            last_existing_index = cast(s32) it_index;
            continue;
        }

        if it_index == 0 {
            n := path.number_of_leading_slashes;
            if n {
                lead := talloc_string(n);
                for 0..n - 1  lead[it] = #char "/";
                path_so_far = join(lead, it);
            } else {
                path_so_far = it;
            }
        } else {
            path_so_far = join(path_so_far, "/", it);
        }

        fs: stat_t;
        if stat(to_c_string(path_so_far, temp), *fs) != 0  break;

        last_existing_index = cast(s32) it_index;
        // A file cannot have anything under it, so this has to be the end.
        if !S_ISDIR(fs.st_mode)  break;
    }

    // Everything exists only if the *last* word did; a path with more after a
    // file does not fully exist.
    fully_exists := path.words && last_existing_index == path.words.count - 1;
    return fully_exists, last_existing_index + 1;
}

// A path of its own, for keeping past the allocator it was parsed under. Call
// these with a non-temporary allocator active.
copy_to_permanent :: (path: Path) -> Path {
    result: Path;

    result.header_string = copy_string(path.header_string);

    result.number_of_leading_slashes = path.number_of_leading_slashes;
    result.trailing_slash            = path.trailing_slash;

    array_copy(*result.words, path.words);
    for * result.words  it.* = copy_string(it.*);

    return result;
}

free_permanent :: (using path: Path) {
    free(header_string);
    for words  free(it);
    free(words.data);
}

#scope_file

#import "POSIX";
