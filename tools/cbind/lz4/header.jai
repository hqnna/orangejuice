//
// The lz4 surface: the types, constants and procedures that `lz4/module.jai`
// does not declare by hand.
//
// Generated from lz4's own headers by `tools/cbind` — see
// `tools/cbind/README.md`. The library itself is not vendored; the `#library`
// in `module.jai` says where it is found.
//

#scope_module

// The reference vendors liblz4; we bind the system one instead, so nothing
// binary lives in the tree.
lz4lib :: #system_library "lz4";

#scope_export

