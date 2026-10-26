
// ------------------------------------------------------------- the macros ---

// What `freetype.h` writes as function-like macros, as procedures.

FT_HAS_HORIZONTAL       :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_HORIZONTAL) != 0; }
FT_HAS_VERTICAL         :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_VERTICAL) != 0; }
FT_HAS_KERNING          :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_KERNING) != 0; }
FT_IS_SCALABLE          :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_SCALABLE) != 0; }
FT_IS_SFNT              :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_SFNT) != 0; }
FT_IS_FIXED_WIDTH       :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_FIXED_WIDTH) != 0; }
FT_HAS_FIXED_SIZES      :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_FIXED_SIZES) != 0; }
FT_HAS_GLYPH_NAMES      :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_GLYPH_NAMES) != 0; }
FT_HAS_MULTIPLE_MASTERS :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_MULTIPLE_MASTERS) != 0; }
FT_IS_NAMED_INSTANCE    :: (face: FT_Face) -> bool { return (face.face_index & 0x7FFF0000) != 0; }
FT_IS_VARIATION         :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_VARIATION) != 0; }
FT_IS_CID_KEYED         :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_CID_KEYED) != 0; }
FT_IS_TRICKY            :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_TRICKY) != 0; }
FT_HAS_COLOR            :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_COLOR) != 0; }
FT_HAS_SVG              :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_SVG) != 0; }
FT_HAS_SBIX             :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_SBIX) != 0; }
FT_HAS_SBIX_OVERLAY     :: (face: FT_Face) -> bool { return (face.face_flags & FT_FACE_FLAG_SBIX_OVERLAY) != 0; }

// `FT_LOAD_TARGET_(mode)` puts a render mode in bits 16 to 19 of the load
// flags, and `FT_LOAD_TARGET_MODE` takes it back out.
FT_LOAD_TARGET_ :: (mode: FT_Render_Mode) -> s32 { return (cast(s32) mode & 15) << 16; }
FT_LOAD_TARGET_MODE :: (flags: s32) -> FT_Render_Mode { return cast(FT_Render_Mode) ((flags >> 16) & 15); }

// The `FT_LOAD_TARGET_*` values as an enum, for code that picks one.
FT_Load_Target :: enum s32 {
    NORMAL :: (cast(s32) FT_Render_Mode.NORMAL) << 16;
    LIGHT  :: (cast(s32) FT_Render_Mode.LIGHT)  << 16;
    MONO   :: (cast(s32) FT_Render_Mode.MONO)   << 16;
    LCD    :: (cast(s32) FT_Render_Mode.LCD)    << 16;
    LCD_V  :: (cast(s32) FT_Render_Mode.LCD_V)  << 16;
}

