//
// A small helper for parsing a file format of one's own.
//
// It hands out the file a line at a time with the blank lines and the comments
// already gone, and it keeps the line number and the filename so that an error
// message can say where it happened without every parsing routine having to
// pass them around.
//
// A file may carry a version number as `[3]` on its first line. It is in square
// brackets because a bare number at the top of a file is easy to mistake for
// data and easy to delete by accident.
//
// `failed` is the other idea here: rather than checking a result after every
// call, a parser can blast through a file and look at `failed` once at the end.
// Nothing ever sets it back to false.
//

#import "Basic";
#import "File";
#import "String";
#import "Hash_Table";

Text_File_Handler :: struct {
    // For the caller's own reference; nothing here uses it.
    short_name: string;
    // What an error message names.
    full_path:  string;

    // May be changed at any time. One byte, not an extended sequence.
    comment_character: u8 = #char "#";

    do_version_number                 := true;
    // False lets the comment character appear mid-line without cutting the
    // rest of it off.
    strip_comments_from_ends_of_lines := true;
    auto_skip_blank_lines             := true;

    // What `deinit` frees. Empty when the data came from memory.
    file_data_to_free: string;
    // What is left to parse.
    file_data:         string;

    failed  := false;
    // Filled in when `do_version_number`.
    version := -1;

    line_number := 0;
}

deinit :: (using x: *Text_File_Handler) {
    free(file_data_to_free);
}

// Reads the file and parses its version number, if any. `optional` keeps a
// missing file from being logged, though it still sets `failed`.
start_file :: (handler: *Text_File_Handler, short_name: string, full_path: string, optional := false, loc := #caller_location) {
    data, success := read_entire_file(full_path, log_errors = !optional);

    if !success {
        handler.failed = true;
        return;
    }

    handler.file_data_to_free = data;

    start_from_memory(handler, short_name, full_path, data, loc);
}

// The same for data that is already in memory — part of a bigger package, say.
// None of the strings are copied: they have to outlive the handler.
start_from_memory :: (using handler: *Text_File_Handler, _short_name: string, _full_path: string, _file_data: string, loc := #caller_location) {
    short_name = _short_name;
    full_path  = _full_path;

    file_data = _file_data;

    if !do_version_number  return;

    line, found := consume_next_line(*file_data);
    line_number += 1;

    if !found {
        log_error("Unable to find a version number at the top of file '%'!\n", full_path);
        failed = true;
        return;
    }

    assert(line.count > 0);
    if line[0] != #char "[" {
        log_error("Expected '[' at the top of file '%', but did not get it!\n", full_path);
        failed = true;
        return;
    }

    advance(*line, 1);

    parsed, version_success, remainder := string_to_int(line);
    if !version_success {
        log_error("Unable to parse the version number at the top of file '%'!\n", full_path);
        failed = true;
        return;
    }

    if !remainder.count || remainder[0] != #char "]" {
        log_error("Expected ']' after version number in file '%', but did not get it! (Remainder is: %)\n", full_path, remainder);
        failed = true;
        return;
    }

    handler.version = parsed;
}

// The next line with something on it: blank lines and comments are skipped and
// the spaces at both ends are gone.
consume_next_line :: (handler: *Text_File_Handler) -> (line: string, found: bool) {
    while true {
        line, found := consume_next_line(*handler.file_data);
        if !found  return "", false;

        handler.line_number += 1;

        line = eat_spaces(line);

        if !line {
            if handler.auto_skip_blank_lines  continue;
            return "", true;
        }

        if handler.strip_comments_from_ends_of_lines {
            cut, left := split_from_left(line, handler.comment_character);
            if cut {
                line = left;
                if line.count == 0  continue;
            }
        } else {
            if line[0] == handler.comment_character  continue;
        }

        line = eat_trailing_spaces(line);
        assert(line.count > 0);

        return line, found;
    }
    return "", false;
}

// One raw line out of a string, advancing it past what was taken. A trailing
// carriage return comes off, so a file written on either system reads the same.
consume_next_line :: (sp: *string) -> (line: string, found: bool) {
    s := sp.*;
    found, result, right := split_from_left(s, 10);

    if !found {
        // The last line of a file with no newline after it is still a line.
        sp.* = "";
        return s, s.count > 0;
    }

    advance(sp, result.count + 1);

    if result && result[result.count - 1] == 13  result.count -= 1;

    return result, true;
}

// Splits at the first run of whitespace: `left` is what came before it, `right`
// what comes after, with the spaces trimmed off both.
break_by_spaces :: (line: string) -> (left: string, right: string) {
    left  := line;
    right := eat_until_space(line);
    left.count -= right.count;

    right = eat_spaces(right);

    return left, right;
}

// Log with the file and line already in the message.
error :: (handler: Text_File_Handler, format: string, args: .. Any, flags := Log_Flags.NONE, loc := #caller_location) {
    new_format := tprint("Line % of '%'! %", handler.line_number, handler.full_path, format);
    log(new_format, ..args, flags = Log_Flags.ERROR | .CONTENT | flags, loc = loc);
} @PrintLike

warn :: (handler: Text_File_Handler, format: string, args: .. Any, flags := Log_Flags.NONE, loc := #caller_location) {
    new_format := tprint("Line % of '%': %", handler.line_number, handler.full_path, format);
    log(new_format, ..args, flags = Log_Flags.WARNING | .CONTENT | flags, loc = loc);
} @PrintLike

// Every meaningful line of a file, in an array of copies. With
// `use_data_from_memory` the filename is only used for error messages.
file_to_array :: (filename: string, do_trim := true, skip_blank_lines := true, do_version_number := false, use_data_from_memory := false, data_from_memory := "", loc := #caller_location) -> (success: bool, array: [..] string) {
    handler: Text_File_Handler;
    handler.do_version_number = do_version_number;

    array: [..] string;

    if use_data_from_memory  start_from_memory(*handler, filename, filename, data_from_memory, loc);
    else                     start_file(*handler, filename, filename, loc = loc);

    if handler.failed  return false, array;

    while true {
        line, found := consume_next_line(*handler);
        if !found  break;

        if do_trim  line = trim(line);
        if skip_blank_lines && !line  continue;

        array_add(*array, copy_string(line));
    }

    return true, array;
}

// The same as a set, for a file that is a list of names to look things up in.
file_to_table :: (filename: string, do_trim := true, skip_blank_lines := true, do_version_number := false, use_data_from_memory := false, data_from_memory := "", loc := #caller_location) -> (success: bool, table: Table(string, void)) {
    handler: Text_File_Handler;
    handler.do_version_number = do_version_number;

    table: Table(string, void);

    if use_data_from_memory  start_from_memory(*handler, filename, filename, data_from_memory, loc);
    else                     start_file(*handler, filename, filename, loc = loc);

    if handler.failed  return false, table;

    while true {
        line, found := consume_next_line(*handler);
        if !found  break;

        if do_trim  line = trim(line);
        if skip_blank_lines && !line  continue;

        nothing: void;
        table_set(*table, copy_string(line), nothing);
    }

    return true, table;
}
