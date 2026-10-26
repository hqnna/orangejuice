//
// freetype: a binding to the system's FreeType, the library that opens
// TrueType, OpenType, Type 1 and bitmap fonts and renders their glyphs.
//
// Everything in `generated.jai` is projected from FreeType's own public
// headers by `tools/cbind`; the C macros FreeType's API leans on
// (`FT_HAS_KERNING`, `FT_LOAD_TARGET_`) are procedures at its end. Anonymous
// enums are named for their members' common prefix (`FT_Err.Ok`); a tagged
// one keeps its tag (`FT_Render_Mode_`, with the typedef `FT_Render_Mode`
// beside it) and loses its members' prefix, which stays reachable as an
// alias: `.NORMAL` and `.FT_RENDER_MODE_NORMAL` are one value.
//
// The library is linked as `#library,system`, so `pkg-config freetype2` is
// what finds it (**C§11**). Nothing binary is shipped.
//

#scope_module

libfreetype :: #library,system "libfreetype";

#scope_export

#load "generated.jai";
