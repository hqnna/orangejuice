//
// Reading line-oriented text files: config, data tables, simple formats.
//
// Comments, blank lines and surrounding whitespace are handled once here
// rather than in every caller. A file that starts with a version number can
// have it checked on the way in, which is what stops an old file being read as
// a new one and silently misunderstood.
//

#import "Basic";
#import "File";
#import "String";

Text_File_Handler :: struct {
    full_path:      string;
    short_name:     string;
    // What is left to read.
    remaining:      string;
    // The whole file, so `remaining` can point into it.
    contents:       string;

    line_number:    s64;
    // The version the file declared, when it was asked to declare one.
    version:        s64 = -1;

    failed:         bool;
    // What is treated as starting a comment. Empty means none.
    comment_prefix: string = "#";
    // Whether a `#` inside a line ends it, or only one that starts it.
    strip_trailing_comments := true;

    do_version_number := false;
    allocator: Allocator;
}

// Opens a file for line-by-line reading. `optional` decides whether a missing
// file is a failure or simply an empty handler.
start_file :: (handler: *Text_File_Handler, full_path: string, optional := false, allocator := context.allocator) -> bool {
    handler.allocator = allocator;
    handler.full_path = full_path;
    handler.short_name = path_filename(full_path);
    handler.line_number = 0;
    handler.failed = false;

    contents, ok := read_entire_file(full_path, allocator);
    if !ok {
        handler.failed = !optional;
        return false;
    }
    handler.contents = contents;
    handler.remaining = contents;

    if handler.do_version_number {
        line, got := consume_next_line(handler);
        if !got {
            log_error_here(handler, "the file is empty, so it has no version number");
            return false;
        }
        version, parsed := string_to_int(trim(line));
        if !parsed {
            log_error_here(handler, "the first line should be a version number");
            return false;
        }
        handler.version = version;
    }
    return true;
}

deinit :: (handler: *Text_File_Handler) {
    if handler.contents.data  free(handler.contents.data, handler.allocator);
    handler.contents = "";
    handler.remaining = "";
}

// The next line with something on it: blank lines and comments are skipped,
// and what comes back is trimmed. `found` is false at the end of the file.
consume_next_line :: (handler: *Text_File_Handler) -> (line: string, found: bool) {
    while handler.remaining.count {
        line := take_line(handler);
        handler.line_number += 1;

        if handler.comment_prefix.count {
            if begins_with(trim_left(line), handler.comment_prefix)  continue;
            if handler.strip_trailing_comments {
                at := find_index_from_left(line, handler.comment_prefix);
                if at >= 0  line = slice(line, 0, at);
            }
        }

        line = trim(line);
        if !line.count  continue;
        return line, true;
    }
    return "", false;
}

// The next line split at its first run of whitespace, which is what a
// `key value` file wants.
consume_next_line_as_key_value :: (handler: *Text_File_Handler) -> (key: string, value: string, found: bool) {
    line, got := consume_next_line(handler);
    if !got  return "", "", false;

    at := 0;
    while at < line.count && !is_space(line[at])  at += 1;
    key := slice(line, 0, at);
    value := trim(slice(line, at, line.count - at));
    return key, value, true;
}

// Reports a problem naming the file and the line it was on, which is the
// whole reason to keep the line number.
log_error_here :: (handler: *Text_File_Handler, format: string, args: ..Any) {
    handler.failed = true;
    message := tprint(format, ..args);
    log_error("%:%: %", handler.full_path, handler.line_number, message);
}

#scope_module

// One line, without its terminator, advancing `remaining` past it. Handles
// both LF and CRLF, since a file written on another platform is still a file.
take_line :: (handler: *Text_File_Handler) -> string {
    text := handler.remaining;
    at := find_index_from_left(text, #char "\n");
    if at < 0 {
        handler.remaining = "";
        return text;
    }
    line := slice(text, 0, at);
    if line.count && line[line.count - 1] == #char "\r"  line.count -= 1;
    handler.remaining = slice(text, at + 1, text.count - at - 1);
    return line;
}
