//
// Walking and copying files.
//
// `visit_files` is the one everything else is built on: it walks a directory,
// optionally into its subdirectories, and calls a procedure for each entry
// with a `File_Visit_Info` describing it. The callback may set
// `descend_into_directory` to false to prune a subtree.
//
// `path.jai` beside this treats a path as an array of words rather than as a
// string, which is what keeps a program from doing path arithmetic with string
// operations and getting it subtly wrong.
//

#load "path.jai";
#load "os/unix.jai";

// Copies a directory and everything under it.
copy_directory :: (source: string, dest: string) {
    names: [2] string;
    names[0] = source;
    names[1] = dest;

    visitor :: (info: *File_Visit_Info, names: [2] string) {
        // The path under `source`, joined onto `dest`.
        dest_name := advance(info.full_name, names[0].count);
        dest_name = join(names[1], dest_name);
        if info.is_directory  make_directory_if_it_does_not_exist(dest_name);
        else                  copy_file(info.full_name, dest_name);
    }

    make_directory_if_it_does_not_exist(dest);
    visit_files(source, true, names, visitor, visit_directories = true);
}

// Every file under `path`, as copies.
file_list :: (path: string, recursive := false, follow_directory_symlinks := true) -> [] string #must {
    files: [..] string;

    visitor :: (info: *File_Visit_Info, user_data: *[..] string) {
        array_add(user_data, copy_string(info.full_name));
    }

    visit_files(path, recursive, *files, visitor, follow_directory_symlinks = follow_directory_symlinks);

    return files;
}

// What the callback is handed. The strings are temporary: they live only for
// the length of the call.
File_Visit_Info :: struct {
    short_name: string;
    full_name:  string;

    modification_time: Apollo_Time;

    // Whether something about this entry could not be found out — no
    // permission to look, say.
    had_error:    bool;
    is_symlink:   bool;
    is_directory: bool;

    // The visitor may set this to false to keep the walk out of a directory.
    descend_into_directory: bool;
}

#scope_module

#import "Basic";
#import "File";
#import "String";
