# examples

The acceptance suite (`docs/spec.md` §8). Each program is a whole compilation —
its `#load`s, its `#import`s, Preload and Runtime_Support — and each prints
something a reader can check by eye. `<name>.out` beside it is what it must
print, byte for byte; `crates/driver/tests/examples.rs` builds every one of
them, runs it, and compares.

They are also documentation: read in order, `010` through `230` is a tour of
the language, and every one of them is a program that really runs.

Nothing here is a port of anyone else's example. The output of each was checked
against the reference compiler while one was still on hand, which is what makes
these goldens evidence rather than a record of what orangejuice happened to do.
