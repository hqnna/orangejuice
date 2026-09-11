//
// Files and the operating system. What is printed is the program's own doing,
// so the golden does not depend on what is on the machine.
//

#import "Basic";
#import "File";
#import "File_Utilities";
String :: #import "String";
System :: #import "System";

main :: () {
    // Beside the executable, so the example writes where it was put rather
    // than wherever it happens to be run from.
    here := String.path_strip_filename(System.get_path_of_running_executable());
    directory := tprint("%/oj_example_190", here);
    make_directory_if_it_does_not_exist(directory);
    defer delete_directory(directory);

    path := tprint("%/notes.txt", directory);
    defer file_delete(path);

    written := write_entire_file(path, "one\ntwo\nthree\n");
    print("written: %\n", written);

    contents, ok := read_entire_file(path);
    print("read back %: % bytes\n", ok, contents.count);
    for String.split(contents, "\n",, allocator = temp) {
        if it.count  print("  line %: %\n", it_index, it);
    }
    free(contents);

    print("exists: %\n", file_exists(path));

    // Appending, through a handle rather than in one go.
    file, opened := file_open(path, for_writing = true, keep_existing_content = true);
    if opened {
        file_seek(file, 0, .END);
        file_write(*file, "four\n");
        file_close(*file);
    }
    again := read_entire_file(path,, allocator = temp);
    print("now % bytes\n", again.count);

    reset_temporary_storage();
}
