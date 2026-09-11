# Examples

The acceptance suite, and a tour of the language: 23 programs, each with the
output it must print. `010` through `230` covers values, control flow,
procedures, structs, arrays, pointers, enums, strings, polymorphism,
compile-time execution, macros, the context, modules, type info, errors,
operator overloading, `using` and variants, files, threads, C interop, inline
assembly and writing a metaprogram.

Read in order, they are the shortest path through Jai. Every one of them is a
program that really runs.

```console
$ oj docs/examples/010_hello.jai -- run    # run one
$ cargo test -p oj-driver --test examples  # build and check them all
```

`<name>.out` beside each program is what it must print, byte for byte;
`crates/driver/tests/examples.rs` builds every one, runs it, and compares.

Nothing here is a port of anyone else's example. Each golden was compared
against the reference compiler, byte for byte, when the example was written —
which is what makes them evidence of Jai's behaviour rather than a record of
what orangejuice happened to do.

## Adding one

Write the program, then record its golden and *read* it to check it is right:

```console
$ OJ_BLESS_EXAMPLES=1 cargo test -p oj-driver --test examples
```

Never bless to make a failing example pass. A golden with no program beside it
fails the suite, so a check cannot be deleted by deleting its program.

`support/` holds files the examples `#load`; the runner only looks at the top
level, so nothing in there is mistaken for a program of its own.
