//
// Builds the static archive an stb module binds against, out of the C source
// the module ships beside it.
//
// The stb libraries are single public-domain C headers, and orangejuice ships
// no binaries: what is in the tree is `<name>.h`, vendored as Sean Barrett
// published it, and `<name>.c`, which compiles its implementation. The first
// compilation that imports the module runs this from a `#run`, with the C
// compiler the link already needs (`$CC`, else `cc`) and `ar` (`$AR`, else
// `ar`), and leaves `.build/<cpu>-<os>/<name>.a` beside the source. Every
// later one finds it there and does nothing, unless the source is newer.
//
// The archive is built under a name of its own and renamed into place, so two
// compilations that race to build it each see either no archive or a whole
// one. An installed distribution is not writable, so it is built when the
// distribution is installed instead (`nix/package.nix`); finding it missing
// and the directory read-only is reported as the error it is.
//
// `stb_image_write` and `stb_image_resize` `#load` this file from here.
//

#scope_module

#import "Basic";

C_LIBRARY_SUBDIRECTORY :: #run c_library_subdirectory();

c_library_subdirectory :: () -> string {
    cpu := ifx CPU == .ARM64 then "aarch64" else "x86_64";
    os  := ifx OS == .MACOS then "darwin" else "linux";
    return sprint("%-%", cpu, os);
}

build_c_library :: (directory: string, name: string) {
    Compiler   :: #import "Compiler";
    Process    :: #import "Process";
    Utilities  :: #import "File_Utilities";
    File       :: #import "File";
    POSIX      :: #import "POSIX";

    source  := tprint("%1%2.c", directory, name);
    header  := tprint("%1%2.h", directory, name);
    output  := tprint("%1.build/%2", directory, C_LIBRARY_SUBDIRECTORY);
    archive := tprint("%1/%2.a", output, name);

    source_time, _, source_exists := Utilities.file_modtime_and_size(source);
    header_time, _, header_exists := Utilities.file_modtime_and_size(header);
    archive_time, _, archive_exists := Utilities.file_modtime_and_size(archive);
    if !source_exists || !header_exists {
        Compiler.compiler_report(tprint("% is missing its C source: % and % should be there.", name, source, header));
        return;
    }
    if archive_exists && !(archive_time < source_time) && !(archive_time < header_time)  return;

    if !File.make_directory_if_it_does_not_exist(output, recursive = true) {
        Compiler.compiler_report(tprint("% has not been built, and % cannot be created to build it in. An installed distribution builds it when it is installed.", archive, output));
        return;
    }

    marker: u8;
    unique := tprint("%.%.%", POSIX.getpid(), to_nanoseconds(current_time_monotonic()), cast(u64) *marker);
    object  := tprint("%/%.%.o", output, name, unique);
    staging := tprint("%/%.%.a", output, name, unique);

    cc := environment_or("CC", "cc");
    ar := environment_or("AR", "ar");

    built, _, errors := Process.run_command(cc, "-O2", "-fPIC", "-w", "-c", source, "-o", object, capture_and_return_output = true);
    if built.exit_code != 0 || built.type != .EXITED {
        Compiler.compiler_report(tprint("Building % with '%' failed:\n%", archive, cc, errors));
        return;
    }
    archived, _, archive_errors := Process.run_command(ar, "rcs", staging, object, capture_and_return_output = true);
    File.file_delete(object);
    if archived.exit_code != 0 || archived.type != .EXITED {
        File.file_delete(staging);
        Compiler.compiler_report(tprint("Archiving % with '%' failed:\n%", archive, ar, archive_errors));
        return;
    }
    if !File.file_move(staging, archive) {
        File.file_delete(staging);
        Compiler.compiler_report(tprint("% was built but could not be moved into place.", archive));
    }
}

environment_or :: (name: string, fallback: string) -> string {
    POSIX :: #import "POSIX";
    value := POSIX.getenv(temp_c_string(name));
    if !value || !value.*  return fallback;
    return to_string(value);
}

#scope_export
