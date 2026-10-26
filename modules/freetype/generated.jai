//
// The FreeType surface: the types, constants, enums and procedures of
// `<freetype/freetype.h>` and the public headers beside it.
//
// Generated from the system's own headers by `tools/cbind` — see
// `tools/cbind/README.md`. `module.jai` declares `libfreetype`.
//


// -------------------------------------------------------------- constants ---

FREETYPE_MAJOR :: 2;
FREETYPE_MINOR :: 14;
FREETYPE_PATCH :: 3;
FT_ADVANCE_FLAG_FAST_ONLY :: 536870912;
FT_ANGLE_2PI :: 23592960;
FT_ANGLE_PI :: 11796480;
FT_ANGLE_PI2 :: 5898240;
FT_ANGLE_PI4 :: 2949120;
FT_AUTOHINTER_SCRIPT_CJK :: 2;
FT_AUTOHINTER_SCRIPT_INDIC :: 3;
FT_AUTOHINTER_SCRIPT_LATIN :: 1;
FT_AUTOHINTER_SCRIPT_NONE :: 0;
FT_CFF_HINTING_ADOBE :: 1;
FT_CFF_HINTING_FREETYPE :: 0;
FT_CHAR_BIT :: 8;
FT_CURVE_TAG_CONIC :: 0;
FT_CURVE_TAG_CUBIC :: 2;
FT_CURVE_TAG_HAS_SCANMODE :: 4;
FT_CURVE_TAG_ON :: 1;
FT_CURVE_TAG_TOUCH_BOTH :: 24;
FT_CURVE_TAG_TOUCH_X :: 8;
FT_CURVE_TAG_TOUCH_Y :: 16;
FT_Curve_Tag_Conic :: 0;
FT_Curve_Tag_Cubic :: 2;
FT_Curve_Tag_On :: 1;
FT_Curve_Tag_Touch_X :: 8;
FT_Curve_Tag_Touch_Y :: 16;
FT_DEBUG_HOOK_TRUETYPE :: 0;
FT_FACE_FLAG_CID_KEYED :: 4096;
FT_FACE_FLAG_COLOR :: 16384;
FT_FACE_FLAG_EXTERNAL_STREAM :: 1024;
FT_FACE_FLAG_FAST_GLYPHS :: 128;
FT_FACE_FLAG_FIXED_SIZES :: 2;
FT_FACE_FLAG_FIXED_WIDTH :: 4;
FT_FACE_FLAG_GLYPH_NAMES :: 512;
FT_FACE_FLAG_HINTER :: 2048;
FT_FACE_FLAG_HORIZONTAL :: 16;
FT_FACE_FLAG_KERNING :: 64;
FT_FACE_FLAG_MULTIPLE_MASTERS :: 256;
FT_FACE_FLAG_SBIX :: 131072;
FT_FACE_FLAG_SBIX_OVERLAY :: 262144;
FT_FACE_FLAG_SCALABLE :: 1;
FT_FACE_FLAG_SFNT :: 8;
FT_FACE_FLAG_SVG :: 65536;
FT_FACE_FLAG_TRICKY :: 8192;
FT_FACE_FLAG_VARIATION :: 32768;
FT_FACE_FLAG_VERTICAL :: 32;
FT_FSTYPE_BITMAP_EMBEDDING_ONLY :: 512;
FT_FSTYPE_EDITABLE_EMBEDDING :: 8;
FT_FSTYPE_INSTALLABLE_EMBEDDING :: 0;
FT_FSTYPE_NO_SUBSETTING :: 256;
FT_FSTYPE_PREVIEW_AND_PRINT_EMBEDDING :: 4;
FT_FSTYPE_RESTRICTED_LICENSE_EMBEDDING :: 2;
FT_HINTING_ADOBE :: 1;
FT_HINTING_FREETYPE :: 0;
FT_INT64_ZERO :: 0;
FT_INT_MAX :: 2147483647;
FT_INT_MIN :: -2147483648;
FT_LCD_FILTER_FIVE_TAPS :: 5;
FT_LLONG_MAX :: 9223372036854775807;
FT_LLONG_MIN :: -9223372036854775808;
FT_LOAD_ADVANCE_ONLY :: 256;
FT_LOAD_BITMAP_METRICS_ONLY :: 4194304;
FT_LOAD_COLOR :: 1048576;
FT_LOAD_COMPUTE_METRICS :: 2097152;
FT_LOAD_CROP_BITMAP :: 64;
FT_LOAD_DEFAULT :: 0;
FT_LOAD_FORCE_AUTOHINT :: 32;
FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH :: 512;
FT_LOAD_IGNORE_TRANSFORM :: 2048;
FT_LOAD_LINEAR_DESIGN :: 8192;
FT_LOAD_MONOCHROME :: 4096;
FT_LOAD_NO_AUTOHINT :: 32768;
FT_LOAD_NO_BITMAP :: 8;
FT_LOAD_NO_HINTING :: 2;
FT_LOAD_NO_RECURSE :: 1024;
FT_LOAD_NO_SCALE :: 1;
FT_LOAD_NO_SVG :: 16777216;
FT_LOAD_PEDANTIC :: 128;
FT_LOAD_RENDER :: 4;
FT_LOAD_SBITS_ONLY :: 16384;
FT_LOAD_SVG_ONLY :: 8388608;
FT_LOAD_TARGET_LCD :: 196608;
FT_LOAD_TARGET_LCD_V :: 262144;
FT_LOAD_TARGET_LIGHT :: 65536;
FT_LOAD_TARGET_MONO :: 131072;
FT_LOAD_TARGET_NORMAL :: 0;
FT_LOAD_VERTICAL_LAYOUT :: 16;
FT_LONG_MAX :: 9223372036854775807;
FT_LONG_MIN :: -9223372036854775808;
FT_MAX_MODULES :: 32;
FT_MODULE_DRIVER_HAS_HINTER :: 1024;
FT_MODULE_DRIVER_HINTS_LIGHTLY :: 2048;
FT_MODULE_DRIVER_NO_OUTLINES :: 512;
FT_MODULE_DRIVER_SCALABLE :: 256;
FT_MODULE_FONT_DRIVER :: 1;
FT_MODULE_HINTER :: 4;
FT_MODULE_RENDERER :: 2;
FT_MODULE_STYLER :: 8;
FT_OPEN_DRIVER :: 8;
FT_OPEN_MEMORY :: 1;
FT_OPEN_PARAMS :: 16;
FT_OPEN_PATHNAME :: 4;
FT_OPEN_STREAM :: 2;
FT_OUTLINE_CONTOURS_MAX :: 65535;
FT_OUTLINE_EVEN_ODD_FILL :: 2;
FT_OUTLINE_HIGH_PRECISION :: 256;
FT_OUTLINE_IGNORE_DROPOUTS :: 8;
FT_OUTLINE_INCLUDE_STUBS :: 32;
FT_OUTLINE_NONE :: 0;
FT_OUTLINE_OVERLAP :: 64;
FT_OUTLINE_OWNER :: 1;
FT_OUTLINE_POINTS_MAX :: 65535;
FT_OUTLINE_REVERSE_FILL :: 4;
FT_OUTLINE_SINGLE_PASS :: 512;
FT_OUTLINE_SMART_DROPOUTS :: 16;
FT_PALETTE_FOR_DARK_BACKGROUND :: 2;
FT_PALETTE_FOR_LIGHT_BACKGROUND :: 1;
FT_PARAM_TAG_IGNORE_PREFERRED_FAMILY :: 1768386662;
FT_PARAM_TAG_IGNORE_PREFERRED_SUBFAMILY :: 1768386675;
FT_PARAM_TAG_IGNORE_SBIX :: 1769169528;
FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_FAMILY :: 1768386662;
FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_SUBFAMILY :: 1768386675;
FT_PARAM_TAG_INCREMENTAL :: 1768842098;
FT_PARAM_TAG_LCD_FILTER_WEIGHTS :: 1818453094;
FT_PARAM_TAG_RANDOM_SEED :: 1936024932;
FT_PARAM_TAG_STEM_DARKENING :: 1684107883;
FT_PARAM_TAG_UNPATENTED_HINTING :: 1970172001;
FT_RASTER_FLAG_AA :: 1;
FT_RASTER_FLAG_CLIP :: 4;
FT_RASTER_FLAG_DEFAULT :: 0;
FT_RASTER_FLAG_DIRECT :: 2;
FT_RASTER_FLAG_SDF :: 8;
FT_RENDER_POOL_SIZE :: 16384;
FT_SIZEOF_INT :: 4;
FT_SIZEOF_LONG :: 8;
FT_SIZEOF_LONG_LONG :: 8;
FT_STYLE_FLAG_BOLD :: 2;
FT_STYLE_FLAG_ITALIC :: 1;
FT_SUBGLYPH_FLAG_2X2 :: 128;
FT_SUBGLYPH_FLAG_ARGS_ARE_WORDS :: 1;
FT_SUBGLYPH_FLAG_ARGS_ARE_XY_VALUES :: 2;
FT_SUBGLYPH_FLAG_ROUND_XY_TO_GRID :: 4;
FT_SUBGLYPH_FLAG_SCALE :: 8;
FT_SUBGLYPH_FLAG_USE_MY_METRICS :: 512;
FT_SUBGLYPH_FLAG_XY_SCALE :: 64;
FT_UINT_MAX :: 4294967295;
FT_ULLONG_MAX :: -1;
FT_ULONG_MAX :: -1;
FT_USHORT_MAX :: 65535;
FT_VAR_AXIS_FLAG_HIDDEN :: 1;
T1_MAX_CHARSTRINGS_OPERANDS :: 256;
T1_MAX_DICT_DEPTH :: 5;
T1_MAX_MM_AXIS :: 4;
T1_MAX_MM_DESIGNS :: 16;
T1_MAX_MM_MAP_POINTS :: 20;
T1_MAX_SUBRS_CALLS :: 16;
TT_CONFIG_OPTION_MAX_RUNNABLE_OPCODES :: 1000000;
TT_INTERPRETER_VERSION_35 :: 35;
TT_INTERPRETER_VERSION_38 :: 38;
TT_INTERPRETER_VERSION_40 :: 40;

// -------------------------------------------------------------- the types ---

ptrdiff_t :: s64;
size_t :: u64;
FT_Int16 :: s16;
FT_UInt16 :: u16;
FT_Int32 :: s32;
FT_UInt32 :: u32;
FT_Fast :: s32;
FT_UFast :: u32;
FT_Int64 :: s64;
FT_UInt64 :: u64;
FT_Memory :: *FT_MemoryRec_;
FT_Alloc_Func :: #type (FT_Memory, s64) -> *void #c_call;
FT_Free_Func :: #type (FT_Memory, *void) -> void #c_call;
FT_Realloc_Func :: #type (FT_Memory, s64, s64, *void) -> *void #c_call;
FT_MemoryRec_ :: struct {
    user: *void;
    alloc: FT_Alloc_Func;
    free: FT_Free_Func;
    realloc: FT_Realloc_Func;
}

FT_Stream :: *FT_StreamRec_;
FT_StreamDesc :: union {
    value: s64;
    pointer: *void;
}

FT_Stream_IoFunc :: #type (FT_Stream, u64, *u8, u64) -> u64 #c_call;
FT_Stream_CloseFunc :: #type (FT_Stream) -> void #c_call;
FT_StreamRec_ :: struct {
    base: *u8;
    size: u64;
    pos: u64;
    descriptor: FT_StreamDesc;
    pathname: FT_StreamDesc;
    read: FT_Stream_IoFunc;
    close: FT_Stream_CloseFunc;
    memory: FT_Memory;
    cursor: *u8;
    limit: *u8;
}

FT_StreamRec :: FT_StreamRec_;

FT_Pos :: s64;
FT_Vector_ :: struct {
    x: FT_Pos;
    y: FT_Pos;
}

FT_Vector :: FT_Vector_;

FT_BBox_ :: struct {
    xMin: FT_Pos;
    yMin: FT_Pos;
    xMax: FT_Pos;
    yMax: FT_Pos;
}

FT_BBox :: FT_BBox_;

FT_Bitmap_ :: struct {
    rows: u32;
    width: u32;
    pitch: s32;
    buffer: *u8;
    num_grays: u16;
    pixel_mode: u8;
    palette_mode: u8;
    palette: *void;
}

FT_Bitmap :: FT_Bitmap_;

FT_Outline_ :: struct {
    n_contours: u16;
    n_points: u16;
    points: *FT_Vector;
    tags: *u8;
    contours: *u16;
    flags: s32;
}

FT_Outline :: FT_Outline_;

FT_Outline_MoveToFunc :: #type (*FT_Vector, *void) -> s32 #c_call;
FT_Outline_LineToFunc :: #type (*FT_Vector, *void) -> s32 #c_call;
FT_Outline_ConicToFunc :: #type (*FT_Vector, *FT_Vector, *void) -> s32 #c_call;
FT_Outline_CubicToFunc :: #type (*FT_Vector, *FT_Vector, *FT_Vector, *void) -> s32 #c_call;
FT_Outline_Funcs_ :: struct {
    move_to: FT_Outline_MoveToFunc;
    line_to: FT_Outline_LineToFunc;
    conic_to: FT_Outline_ConicToFunc;
    cubic_to: FT_Outline_CubicToFunc;
    shift: s32;
    delta: FT_Pos;
}

FT_Outline_Funcs :: FT_Outline_Funcs_;

FT_Span_ :: struct {
    x: u16;
    len: u16;
    coverage: u8;
}

FT_Span :: FT_Span_;

FT_SpanFunc :: #type (s32, s32, *FT_Span, *void) -> void #c_call;
FT_Raster_BitTest_Func :: #type (s32, s32, *void) -> s32 #c_call;
FT_Raster_BitSet_Func :: #type (s32, s32, *void) -> void #c_call;
FT_Raster_Params_ :: struct {
    target: *FT_Bitmap;
    source: *void;
    flags: s32;
    gray_spans: FT_SpanFunc;
    black_spans: FT_SpanFunc;
    bit_test: FT_Raster_BitTest_Func;
    bit_set: FT_Raster_BitSet_Func;
    user: *void;
    clip_box: FT_BBox;
}

FT_Raster_Params :: FT_Raster_Params_;

FT_Raster :: *FT_RasterRec_;
FT_Raster_NewFunc :: #type (*void, *FT_Raster) -> s32 #c_call;
FT_Raster_DoneFunc :: #type (FT_Raster) -> void #c_call;
FT_Raster_ResetFunc :: #type (FT_Raster, *u8, u64) -> void #c_call;
FT_Raster_SetModeFunc :: #type (FT_Raster, u64, *void) -> s32 #c_call;
FT_Raster_RenderFunc :: #type (FT_Raster, *FT_Raster_Params) -> s32 #c_call;
FT_Raster_Funcs_ :: struct {
    glyph_format: FT_Glyph_Format;
    raster_new: FT_Raster_NewFunc;
    raster_reset: FT_Raster_ResetFunc;
    raster_set_mode: FT_Raster_SetModeFunc;
    raster_render: FT_Raster_RenderFunc;
    raster_done: FT_Raster_DoneFunc;
}

FT_Raster_Funcs :: FT_Raster_Funcs_;

FT_Bool :: u8;
FT_FWord :: s16;
FT_UFWord :: u16;
FT_Char :: u8;
FT_Byte :: u8;
FT_Bytes :: *FT_Byte;
FT_Tag :: FT_UInt32;
FT_String :: u8;
FT_Short :: s16;
FT_UShort :: u16;
FT_Int :: s32;
FT_UInt :: u32;
FT_Long :: s64;
FT_ULong :: u64;
FT_F2Dot14 :: s16;
FT_F26Dot6 :: s64;
FT_Fixed :: s64;
FT_Error :: s32;
FT_Pointer :: *void;
FT_Offset :: size_t;
FT_PtrDist :: ptrdiff_t;
FT_UnitVector_ :: struct {
    x: FT_F2Dot14;
    y: FT_F2Dot14;
}

FT_UnitVector :: FT_UnitVector_;

FT_Matrix_ :: struct {
    _xx: FT_Fixed;
    xy: FT_Fixed;
    yx: FT_Fixed;
    yy: FT_Fixed;
}

FT_Matrix :: FT_Matrix_;

FT_Data_ :: struct {
    pointer: *FT_Byte;
    length: FT_UInt;
}

FT_Data :: FT_Data_;

FT_Generic_Finalizer :: #type (*void) -> void #c_call;
FT_Generic_ :: struct {
    data: *void;
    finalizer: FT_Generic_Finalizer;
}

FT_Generic :: FT_Generic_;

FT_ListNode :: *FT_ListNodeRec_;
FT_List :: *FT_ListRec_;
FT_ListNodeRec_ :: struct {
    prev: FT_ListNode;
    next: FT_ListNode;
    data: *void;
}

FT_ListNodeRec :: FT_ListNodeRec_;

FT_ListRec_ :: struct {
    head: FT_ListNode;
    tail: FT_ListNode;
}

FT_ListRec :: FT_ListRec_;

FT_Glyph_Metrics_ :: struct {
    width: FT_Pos;
    height: FT_Pos;
    horiBearingX: FT_Pos;
    horiBearingY: FT_Pos;
    horiAdvance: FT_Pos;
    vertBearingX: FT_Pos;
    vertBearingY: FT_Pos;
    vertAdvance: FT_Pos;
}

FT_Glyph_Metrics :: FT_Glyph_Metrics_;

FT_Bitmap_Size_ :: struct {
    height: FT_Short;
    width: FT_Short;
    size: FT_Pos;
    x_ppem: FT_Pos;
    y_ppem: FT_Pos;
}

FT_Bitmap_Size :: FT_Bitmap_Size_;

FT_Library :: *FT_LibraryRec_;
FT_Module :: *FT_ModuleRec_;
FT_Driver :: *FT_DriverRec_;
FT_Renderer :: *FT_RendererRec_;
FT_Face :: *FT_FaceRec_;
FT_Size :: *FT_SizeRec_;
FT_GlyphSlot :: *FT_GlyphSlotRec_;
FT_CharMap :: *FT_CharMapRec_;
FT_CharMapRec_ :: struct {
    face: FT_Face;
    encoding: FT_Encoding;
    platform_id: FT_UShort;
    encoding_id: FT_UShort;
}

FT_CharMapRec :: FT_CharMapRec_;

FT_Face_Internal :: *FT_Face_InternalRec_;
FT_FaceRec_ :: struct {
    num_faces: FT_Long;
    face_index: FT_Long;
    face_flags: FT_Long;
    style_flags: FT_Long;
    num_glyphs: FT_Long;
    family_name: *FT_String;
    style_name: *FT_String;
    num_fixed_sizes: FT_Int;
    available_sizes: *FT_Bitmap_Size;
    num_charmaps: FT_Int;
    charmaps: *FT_CharMap;
    generic: FT_Generic;
    bbox: FT_BBox;
    units_per_EM: FT_UShort;
    ascender: FT_Short;
    descender: FT_Short;
    height: FT_Short;
    max_advance_width: FT_Short;
    max_advance_height: FT_Short;
    underline_position: FT_Short;
    underline_thickness: FT_Short;
    glyph: FT_GlyphSlot;
    size: FT_Size;
    charmap: FT_CharMap;
    driver: FT_Driver;
    memory: FT_Memory;
    stream: FT_Stream;
    sizes_list: FT_ListRec;
    autohint: FT_Generic;
    extensions: *void;
    internal: FT_Face_Internal;
}

FT_FaceRec :: FT_FaceRec_;

FT_Size_Internal :: *FT_Size_InternalRec_;
FT_Size_Metrics_ :: struct {
    x_ppem: FT_UShort;
    y_ppem: FT_UShort;
    x_scale: FT_Fixed;
    y_scale: FT_Fixed;
    ascender: FT_Pos;
    descender: FT_Pos;
    height: FT_Pos;
    max_advance: FT_Pos;
}

FT_Size_Metrics :: FT_Size_Metrics_;

FT_SizeRec_ :: struct {
    face: FT_Face;
    generic: FT_Generic;
    metrics: FT_Size_Metrics;
    internal: FT_Size_Internal;
}

FT_SizeRec :: FT_SizeRec_;

FT_SubGlyph :: *FT_SubGlyphRec_;
FT_Slot_Internal :: *FT_Slot_InternalRec_;
FT_GlyphSlotRec_ :: struct {
    library: FT_Library;
    face: FT_Face;
    next: FT_GlyphSlot;
    glyph_index: FT_UInt;
    generic: FT_Generic;
    metrics: FT_Glyph_Metrics;
    linearHoriAdvance: FT_Fixed;
    linearVertAdvance: FT_Fixed;
    advance: FT_Vector;
    format: FT_Glyph_Format;
    bitmap: FT_Bitmap;
    bitmap_left: FT_Int;
    bitmap_top: FT_Int;
    outline: FT_Outline;
    num_subglyphs: FT_UInt;
    subglyphs: FT_SubGlyph;
    control_data: *void;
    control_len: s64;
    lsb_delta: FT_Pos;
    rsb_delta: FT_Pos;
    other: *void;
    internal: FT_Slot_Internal;
}

FT_GlyphSlotRec :: FT_GlyphSlotRec_;

FT_Parameter_ :: struct {
    tag: FT_ULong;
    data: FT_Pointer;
}

FT_Parameter :: FT_Parameter_;

FT_Open_Args_ :: struct {
    flags: FT_UInt;
    memory_base: *FT_Byte;
    memory_size: FT_Long;
    pathname: *FT_String;
    stream: FT_Stream;
    driver: FT_Module;
    num_params: FT_Int;
    params: *FT_Parameter;
}

FT_Open_Args :: FT_Open_Args_;

FT_Size_RequestRec_ :: struct {
    type: FT_Size_Request_Type;
    width: FT_Long;
    height: FT_Long;
    horiResolution: FT_UInt;
    vertResolution: FT_UInt;
}

FT_Size_RequestRec :: FT_Size_RequestRec_;

FT_Size_Request :: *FT_Size_RequestRec_;
FT_Glyph_Class :: FT_Glyph_Class_;
FT_Glyph :: *FT_GlyphRec_;
FT_GlyphRec_ :: struct {
    library: FT_Library;
    clazz: *FT_Glyph_Class;
    format: FT_Glyph_Format;
    advance: FT_Vector;
}

FT_GlyphRec :: FT_GlyphRec_;

FT_BitmapGlyph :: *FT_BitmapGlyphRec_;
FT_BitmapGlyphRec_ :: struct {
    root: FT_GlyphRec;
    left: FT_Int;
    top: FT_Int;
    bitmap: FT_Bitmap;
}

FT_BitmapGlyphRec :: FT_BitmapGlyphRec_;

FT_OutlineGlyph :: *FT_OutlineGlyphRec_;
FT_OutlineGlyphRec_ :: struct {
    root: FT_GlyphRec;
    outline: FT_Outline;
}

FT_OutlineGlyphRec :: FT_OutlineGlyphRec_;

FT_SvgGlyph :: *FT_SvgGlyphRec_;
FT_SvgGlyphRec_ :: struct {
    root: FT_GlyphRec;
    svg_document: *FT_Byte;
    svg_document_length: FT_ULong;
    glyph_index: FT_UInt;
    metrics: FT_Size_Metrics;
    units_per_EM: FT_UShort;
    start_glyph_id: FT_UShort;
    end_glyph_id: FT_UShort;
    transform: FT_Matrix;
    delta: FT_Vector;
}

FT_SvgGlyphRec :: FT_SvgGlyphRec_;

FT_Color_ :: struct {
    blue: FT_Byte;
    green: FT_Byte;
    red: FT_Byte;
    alpha: FT_Byte;
}

FT_Color :: FT_Color_;

FT_Palette_Data_ :: struct {
    num_palettes: FT_UShort;
    palette_name_ids: *FT_UShort;
    palette_flags: *FT_UShort;
    num_palette_entries: FT_UShort;
    palette_entry_name_ids: *FT_UShort;
}

FT_Palette_Data :: FT_Palette_Data_;

FT_LayerIterator_ :: struct {
    num_layers: FT_UInt;
    layer: FT_UInt;
    p: *FT_Byte;
}

FT_LayerIterator :: FT_LayerIterator_;

FT_ColorStopIterator_ :: struct {
    num_color_stops: FT_UInt;
    current_color_stop: FT_UInt;
    p: *FT_Byte;
    read_variable: FT_Bool;
}

FT_ColorStopIterator :: FT_ColorStopIterator_;

FT_ColorIndex_ :: struct {
    palette_index: FT_UInt16;
    alpha: FT_F2Dot14;
}

FT_ColorIndex :: FT_ColorIndex_;

FT_ColorStop_ :: struct {
    stop_offset: FT_Fixed;
    color: FT_ColorIndex;
}

FT_ColorStop :: FT_ColorStop_;

FT_ColorLine_ :: struct {
    extend: FT_PaintExtend;
    color_stop_iterator: FT_ColorStopIterator;
}

FT_ColorLine :: FT_ColorLine_;

FT_Affine_23_ :: struct {
    _xx: FT_Fixed;
    xy: FT_Fixed;
    dx: FT_Fixed;
    yx: FT_Fixed;
    yy: FT_Fixed;
    dy: FT_Fixed;
}

FT_Affine23 :: FT_Affine_23_;

FT_Opaque_Paint_ :: struct {
    p: *FT_Byte;
    insert_root_transform: FT_Bool;
}

FT_OpaquePaint :: FT_Opaque_Paint_;

FT_PaintColrLayers_ :: struct {
    layer_iterator: FT_LayerIterator;
}

FT_PaintColrLayers :: FT_PaintColrLayers_;

FT_PaintSolid_ :: struct {
    color: FT_ColorIndex;
}

FT_PaintSolid :: FT_PaintSolid_;

FT_PaintLinearGradient_ :: struct {
    colorline: FT_ColorLine;
    p0: FT_Vector;
    p1: FT_Vector;
    p2: FT_Vector;
}

FT_PaintLinearGradient :: FT_PaintLinearGradient_;

FT_PaintRadialGradient_ :: struct {
    colorline: FT_ColorLine;
    c0: FT_Vector;
    r0: FT_Pos;
    c1: FT_Vector;
    r1: FT_Pos;
}

FT_PaintRadialGradient :: FT_PaintRadialGradient_;

FT_PaintSweepGradient_ :: struct {
    colorline: FT_ColorLine;
    center: FT_Vector;
    start_angle: FT_Fixed;
    end_angle: FT_Fixed;
}

FT_PaintSweepGradient :: FT_PaintSweepGradient_;

FT_PaintGlyph_ :: struct {
    paint: FT_OpaquePaint;
    glyphID: FT_UInt;
}

FT_PaintGlyph :: FT_PaintGlyph_;

FT_PaintColrGlyph_ :: struct {
    glyphID: FT_UInt;
}

FT_PaintColrGlyph :: FT_PaintColrGlyph_;

FT_PaintTransform_ :: struct {
    paint: FT_OpaquePaint;
    affine: FT_Affine23;
}

FT_PaintTransform :: FT_PaintTransform_;

FT_PaintTranslate_ :: struct {
    paint: FT_OpaquePaint;
    dx: FT_Fixed;
    dy: FT_Fixed;
}

FT_PaintTranslate :: FT_PaintTranslate_;

FT_PaintScale_ :: struct {
    paint: FT_OpaquePaint;
    scale_x: FT_Fixed;
    scale_y: FT_Fixed;
    center_x: FT_Fixed;
    center_y: FT_Fixed;
}

FT_PaintScale :: FT_PaintScale_;

FT_PaintRotate_ :: struct {
    paint: FT_OpaquePaint;
    angle: FT_Fixed;
    center_x: FT_Fixed;
    center_y: FT_Fixed;
}

FT_PaintRotate :: FT_PaintRotate_;

FT_PaintSkew_ :: struct {
    paint: FT_OpaquePaint;
    x_skew_angle: FT_Fixed;
    y_skew_angle: FT_Fixed;
    center_x: FT_Fixed;
    center_y: FT_Fixed;
}

FT_PaintSkew :: FT_PaintSkew_;

FT_PaintComposite_ :: struct {
    source_paint: FT_OpaquePaint;
    composite_mode: FT_Composite_Mode;
    backdrop_paint: FT_OpaquePaint;
}

FT_PaintComposite :: FT_PaintComposite_;

FT_COLR_Paint_ :: struct {
    format: FT_PaintFormat;
    u: union {
        colr_layers: FT_PaintColrLayers;
        glyph: FT_PaintGlyph;
        solid: FT_PaintSolid;
        linear_gradient: FT_PaintLinearGradient;
        radial_gradient: FT_PaintRadialGradient;
        sweep_gradient: FT_PaintSweepGradient;
        transform: FT_PaintTransform;
        translate: FT_PaintTranslate;
        scale: FT_PaintScale;
        rotate: FT_PaintRotate;
        skew: FT_PaintSkew;
        composite: FT_PaintComposite;
        colr_glyph: FT_PaintColrGlyph;
    }
}

FT_COLR_Paint :: FT_COLR_Paint_;

FT_ClipBox_ :: struct {
    bottom_left: FT_Vector;
    top_left: FT_Vector;
    top_right: FT_Vector;
    bottom_right: FT_Vector;
}

FT_ClipBox :: FT_ClipBox_;

FT_Stroker :: *FT_StrokerRec_;
FT_Module_Interface :: FT_Pointer;
FT_Module_Constructor :: #type (FT_Module) -> FT_Error #c_call;
FT_Module_Destructor :: #type (FT_Module) -> void #c_call;
FT_Module_Requester :: #type (FT_Module, *u8) -> FT_Module_Interface #c_call;
FT_Module_Class_ :: struct {
    module_flags: FT_ULong;
    module_size: FT_Long;
    module_name: *FT_String;
    module_version: FT_Fixed;
    module_requires: FT_Fixed;
    module_interface: *void;
    module_init: FT_Module_Constructor;
    module_done: FT_Module_Destructor;
    get_interface: FT_Module_Requester;
}

FT_Module_Class :: FT_Module_Class_;

FT_DebugHook_Func :: #type (*void) -> FT_Error #c_call;
FT_SfntName_ :: struct {
    platform_id: FT_UShort;
    encoding_id: FT_UShort;
    language_id: FT_UShort;
    name_id: FT_UShort;
    _string: *FT_Byte;
    string_len: FT_UInt;
}

FT_SfntName :: FT_SfntName_;

FT_SfntLangTag_ :: struct {
    _string: *FT_Byte;
    string_len: FT_UInt;
}

FT_SfntLangTag :: FT_SfntLangTag_;

TT_Header :: struct {
    Table_Version: FT_Fixed;
    Font_Revision: FT_Fixed;
    CheckSum_Adjust: FT_Long;
    Magic_Number: FT_Long;
    Flags: FT_UShort;
    Units_Per_EM: FT_UShort;
    Created: [2] FT_ULong;
    Modified: [2] FT_ULong;
    xMin: FT_Short;
    yMin: FT_Short;
    xMax: FT_Short;
    yMax: FT_Short;
    Mac_Style: FT_UShort;
    Lowest_Rec_PPEM: FT_UShort;
    Font_Direction: FT_Short;
    Index_To_Loc_Format: FT_Short;
    Glyph_Data_Format: FT_Short;
}

TT_HoriHeader :: struct {
    Version: FT_Fixed;
    Ascender: FT_Short;
    Descender: FT_Short;
    Line_Gap: FT_Short;
    advance_Width_Max: FT_UShort;
    min_Left_Side_Bearing: FT_Short;
    min_Right_Side_Bearing: FT_Short;
    xMax_Extent: FT_Short;
    caret_Slope_Rise: FT_Short;
    caret_Slope_Run: FT_Short;
    caret_Offset: FT_Short;
    Reserved: [4] FT_Short;
    metric_Data_Format: FT_Short;
    number_Of_HMetrics: FT_UShort;
    long_metrics: *void;
    short_metrics: *void;
}

TT_VertHeader :: struct {
    Version: FT_Fixed;
    Ascender: FT_Short;
    Descender: FT_Short;
    Line_Gap: FT_Short;
    advance_Height_Max: FT_UShort;
    min_Top_Side_Bearing: FT_Short;
    min_Bottom_Side_Bearing: FT_Short;
    yMax_Extent: FT_Short;
    caret_Slope_Rise: FT_Short;
    caret_Slope_Run: FT_Short;
    caret_Offset: FT_Short;
    Reserved: [4] FT_Short;
    metric_Data_Format: FT_Short;
    number_Of_VMetrics: FT_UShort;
    long_metrics: *void;
    short_metrics: *void;
}

TT_OS2 :: struct {
    version: FT_UShort;
    xAvgCharWidth: FT_Short;
    usWeightClass: FT_UShort;
    usWidthClass: FT_UShort;
    fsType: FT_UShort;
    ySubscriptXSize: FT_Short;
    ySubscriptYSize: FT_Short;
    ySubscriptXOffset: FT_Short;
    ySubscriptYOffset: FT_Short;
    ySuperscriptXSize: FT_Short;
    ySuperscriptYSize: FT_Short;
    ySuperscriptXOffset: FT_Short;
    ySuperscriptYOffset: FT_Short;
    yStrikeoutSize: FT_Short;
    yStrikeoutPosition: FT_Short;
    sFamilyClass: FT_Short;
    panose: [10] FT_Byte;
    ulUnicodeRange1: FT_ULong;
    ulUnicodeRange2: FT_ULong;
    ulUnicodeRange3: FT_ULong;
    ulUnicodeRange4: FT_ULong;
    achVendID: [4] FT_Char;
    fsSelection: FT_UShort;
    usFirstCharIndex: FT_UShort;
    usLastCharIndex: FT_UShort;
    sTypoAscender: FT_Short;
    sTypoDescender: FT_Short;
    sTypoLineGap: FT_Short;
    usWinAscent: FT_UShort;
    usWinDescent: FT_UShort;
    ulCodePageRange1: FT_ULong;
    ulCodePageRange2: FT_ULong;
    sxHeight: FT_Short;
    sCapHeight: FT_Short;
    usDefaultChar: FT_UShort;
    usBreakChar: FT_UShort;
    usMaxContext: FT_UShort;
    usLowerOpticalPointSize: FT_UShort;
    usUpperOpticalPointSize: FT_UShort;
}

TT_Postscript :: struct {
    FormatType: FT_Fixed;
    italicAngle: FT_Fixed;
    underlinePosition: FT_Short;
    underlineThickness: FT_Short;
    isFixedPitch: FT_ULong;
    minMemType42: FT_ULong;
    maxMemType42: FT_ULong;
    minMemType1: FT_ULong;
    maxMemType1: FT_ULong;
}

TT_PCLT :: struct {
    Version: FT_Fixed;
    FontNumber: FT_ULong;
    Pitch: FT_UShort;
    xHeight: FT_UShort;
    Style: FT_UShort;
    TypeFamily: FT_UShort;
    CapHeight: FT_UShort;
    SymbolSet: FT_UShort;
    TypeFace: [16] FT_Char;
    CharacterComplement: [8] FT_Char;
    FileName: [6] FT_Char;
    StrokeWeight: FT_Char;
    WidthType: FT_Char;
    SerifStyle: FT_Byte;
    Reserved: FT_Byte;
}

TT_MaxProfile :: struct {
    version: FT_Fixed;
    numGlyphs: FT_UShort;
    maxPoints: FT_UShort;
    maxContours: FT_UShort;
    maxCompositePoints: FT_UShort;
    maxCompositeContours: FT_UShort;
    maxZones: FT_UShort;
    maxTwilightPoints: FT_UShort;
    maxStorage: FT_UShort;
    maxFunctionDefs: FT_UShort;
    maxInstructionDefs: FT_UShort;
    maxStackElements: FT_UShort;
    maxSizeOfInstructions: FT_UShort;
    maxComponentElements: FT_UShort;
    maxComponentDepth: FT_UShort;
}

FT_MM_Axis_ :: struct {
    name: *FT_String;
    minimum: FT_Long;
    maximum: FT_Long;
}

FT_MM_Axis :: FT_MM_Axis_;

FT_Multi_Master_ :: struct {
    num_axis: FT_UInt;
    num_designs: FT_UInt;
    axis: [4] FT_MM_Axis;
}

FT_Multi_Master :: FT_Multi_Master_;

FT_Var_Axis_ :: struct {
    name: *FT_String;
    minimum: FT_Fixed;
    def: FT_Fixed;
    maximum: FT_Fixed;
    tag: FT_ULong;
    strid: FT_UInt;
}

FT_Var_Axis :: FT_Var_Axis_;

FT_Var_Named_Style_ :: struct {
    coords: *FT_Fixed;
    strid: FT_UInt;
    psid: FT_UInt;
}

FT_Var_Named_Style :: FT_Var_Named_Style_;

FT_MM_Var_ :: struct {
    num_axis: FT_UInt;
    num_designs: FT_UInt;
    num_namedstyles: FT_UInt;
    axis: *FT_Var_Axis;
    namedstyle: *FT_Var_Named_Style;
}

FT_MM_Var :: FT_MM_Var_;

FT_Angle :: FT_Fixed;
FT_Prop_GlyphToScriptMap_ :: struct {
    face: FT_Face;
    map: *FT_UShort;
}

FT_Prop_GlyphToScriptMap :: FT_Prop_GlyphToScriptMap_;

FT_Prop_IncreaseXHeight_ :: struct {
    face: FT_Face;
    limit: FT_UInt;
}

FT_Prop_IncreaseXHeight :: FT_Prop_IncreaseXHeight_;

// Opaque: the library owns the contents.
FT_DriverRec_ :: struct {}

// Opaque: the library owns the contents.
FT_Face_InternalRec_ :: struct {}

// Opaque: the library owns the contents.
FT_Glyph_Class_ :: struct {}

// Opaque: the library owns the contents.
FT_LibraryRec_ :: struct {}

// Opaque: the library owns the contents.
FT_ModuleRec_ :: struct {}

// Opaque: the library owns the contents.
FT_RasterRec_ :: struct {}

// Opaque: the library owns the contents.
FT_RendererRec_ :: struct {}

// Opaque: the library owns the contents.
FT_Size_InternalRec_ :: struct {}

// Opaque: the library owns the contents.
FT_Slot_InternalRec_ :: struct {}

// Opaque: the library owns the contents.
FT_StrokerRec_ :: struct {}

// Opaque: the library owns the contents.
FT_SubGlyphRec_ :: struct {}


// ------------------------------------------------------------------ enums ---

FT_Pixel_Mode_ :: enum s32 {
    NONE :: 0;
    MONO;
    GRAY;
    GRAY2;
    GRAY4;
    LCD;
    LCD_V;
    BGRA;
    MAX;

    FT_PIXEL_MODE_NONE :: NONE;
    FT_PIXEL_MODE_MONO :: MONO;
    FT_PIXEL_MODE_GRAY :: GRAY;
    FT_PIXEL_MODE_GRAY2 :: GRAY2;
    FT_PIXEL_MODE_GRAY4 :: GRAY4;
    FT_PIXEL_MODE_LCD :: LCD;
    FT_PIXEL_MODE_LCD_V :: LCD_V;
    FT_PIXEL_MODE_BGRA :: BGRA;
    FT_PIXEL_MODE_MAX :: MAX;
}
FT_Pixel_Mode :: FT_Pixel_Mode_;

FT_Glyph_Format_ :: enum s32 {
    NONE :: (((0) << 24) | ((0) << 16) | ((0) << 8) | (0));
    COMPOSITE :: (((99) << 24) | ((111) << 16) | ((109) << 8) | (112));
    BITMAP :: (((98) << 24) | ((105) << 16) | ((116) << 8) | (115));
    OUTLINE :: (((111) << 24) | ((117) << 16) | ((116) << 8) | (108));
    PLOTTER :: (((112) << 24) | ((108) << 16) | ((111) << 8) | (116));
    SVG :: (((83) << 24) | ((86) << 16) | ((71) << 8) | (32));

    FT_GLYPH_FORMAT_NONE :: NONE;
    FT_GLYPH_FORMAT_COMPOSITE :: COMPOSITE;
    FT_GLYPH_FORMAT_BITMAP :: BITMAP;
    FT_GLYPH_FORMAT_OUTLINE :: OUTLINE;
    FT_GLYPH_FORMAT_PLOTTER :: PLOTTER;
    FT_GLYPH_FORMAT_SVG :: SVG;
}
FT_Glyph_Format :: FT_Glyph_Format_;

FT_Mod_Err :: enum s32 {
    Base :: 0;
    Autofit :: 0;
    BDF :: 0;
    Bzip2 :: 0;
    Cache :: 0;
    CFF :: 0;
    CID :: 0;
    Gzip :: 0;
    LZW :: 0;
    OTvalid :: 0;
    PCF :: 0;
    PFR :: 0;
    PSaux :: 0;
    PShinter :: 0;
    PSnames :: 0;
    Raster :: 0;
    SFNT :: 0;
    Smooth :: 0;
    TrueType :: 0;
    Type1 :: 0;
    Type42 :: 0;
    Winfonts :: 0;
    GXvalid :: 0;
    Sdf :: 0;
    Max;
}

FT_Err :: enum s32 {
    Ok :: 0;
    Cannot_Open_Resource :: 1 + 0;
    Unknown_File_Format :: 2 + 0;
    Invalid_File_Format :: 3 + 0;
    Invalid_Version :: 4 + 0;
    Lower_Module_Version :: 5 + 0;
    Invalid_Argument :: 6 + 0;
    Unimplemented_Feature :: 7 + 0;
    Invalid_Table :: 8 + 0;
    Invalid_Offset :: 9 + 0;
    Array_Too_Large :: 10 + 0;
    Missing_Module :: 11 + 0;
    Missing_Property :: 12 + 0;
    Invalid_Glyph_Index :: 16 + 0;
    Invalid_Character_Code :: 17 + 0;
    Invalid_Glyph_Format :: 18 + 0;
    Cannot_Render_Glyph :: 19 + 0;
    Invalid_Outline :: 20 + 0;
    Invalid_Composite :: 21 + 0;
    Too_Many_Hints :: 22 + 0;
    Invalid_Pixel_Size :: 23 + 0;
    Invalid_SVG_Document :: 24 + 0;
    Invalid_Handle :: 32 + 0;
    Invalid_Library_Handle :: 33 + 0;
    Invalid_Driver_Handle :: 34 + 0;
    Invalid_Face_Handle :: 35 + 0;
    Invalid_Size_Handle :: 36 + 0;
    Invalid_Slot_Handle :: 37 + 0;
    Invalid_CharMap_Handle :: 38 + 0;
    Invalid_Cache_Handle :: 39 + 0;
    Invalid_Stream_Handle :: 40 + 0;
    Too_Many_Drivers :: 48 + 0;
    Too_Many_Extensions :: 49 + 0;
    Out_Of_Memory :: 64 + 0;
    Unlisted_Object :: 65 + 0;
    Cannot_Open_Stream :: 81 + 0;
    Invalid_Stream_Seek :: 82 + 0;
    Invalid_Stream_Skip :: 83 + 0;
    Invalid_Stream_Read :: 84 + 0;
    Invalid_Stream_Operation :: 85 + 0;
    Invalid_Frame_Operation :: 86 + 0;
    Nested_Frame_Access :: 87 + 0;
    Invalid_Frame_Read :: 88 + 0;
    Raster_Uninitialized :: 96 + 0;
    Raster_Corrupted :: 97 + 0;
    Raster_Overflow :: 98 + 0;
    Raster_Negative_Height :: 99 + 0;
    Too_Many_Caches :: 112 + 0;
    Invalid_Opcode :: 128 + 0;
    Too_Few_Arguments :: 129 + 0;
    Stack_Overflow :: 130 + 0;
    Code_Overflow :: 131 + 0;
    Bad_Argument :: 132 + 0;
    Divide_By_Zero :: 133 + 0;
    Invalid_Reference :: 134 + 0;
    Debug_OpCode :: 135 + 0;
    ENDF_In_Exec_Stream :: 136 + 0;
    Nested_DEFS :: 137 + 0;
    Invalid_CodeRange :: 138 + 0;
    Execution_Too_Long :: 139 + 0;
    Too_Many_Function_Defs :: 140 + 0;
    Too_Many_Instruction_Defs :: 141 + 0;
    Table_Missing :: 142 + 0;
    Horiz_Header_Missing :: 143 + 0;
    Locations_Missing :: 144 + 0;
    Name_Table_Missing :: 145 + 0;
    CMap_Table_Missing :: 146 + 0;
    Hmtx_Table_Missing :: 147 + 0;
    Post_Table_Missing :: 148 + 0;
    Invalid_Horiz_Metrics :: 149 + 0;
    Invalid_CharMap_Format :: 150 + 0;
    Invalid_PPem :: 151 + 0;
    Invalid_Vert_Metrics :: 152 + 0;
    Could_Not_Find_Context :: 153 + 0;
    Invalid_Post_Table_Format :: 154 + 0;
    Invalid_Post_Table :: 155 + 0;
    DEF_In_Glyf_Bytecode :: 156 + 0;
    Missing_Bitmap :: 157 + 0;
    Missing_SVG_Hooks :: 158 + 0;
    Syntax_Error :: 160 + 0;
    Stack_Underflow :: 161 + 0;
    Ignore :: 162 + 0;
    No_Unicode_Glyph_Name :: 163 + 0;
    Glyph_Too_Big :: 164 + 0;
    Missing_Startfont_Field :: 176 + 0;
    Missing_Font_Field :: 177 + 0;
    Missing_Size_Field :: 178 + 0;
    Missing_Fontboundingbox_Field :: 179 + 0;
    Missing_Chars_Field :: 180 + 0;
    Missing_Startchar_Field :: 181 + 0;
    Missing_Encoding_Field :: 182 + 0;
    Missing_Bbx_Field :: 183 + 0;
    Bbx_Too_Big :: 184 + 0;
    Corrupted_Font_Header :: 185 + 0;
    Corrupted_Font_Glyphs :: 186 + 0;
    Max;
}

FT_Encoding_ :: enum s32 {
    NONE :: (((0) << 24) | ((0) << 16) | ((0) << 8) | (0));
    MS_SYMBOL :: (((115) << 24) | ((121) << 16) | ((109) << 8) | (98));
    UNICODE :: (((117) << 24) | ((110) << 16) | ((105) << 8) | (99));
    SJIS :: (((115) << 24) | ((106) << 16) | ((105) << 8) | (115));
    PRC :: (((103) << 24) | ((98) << 16) | ((32) << 8) | (32));
    BIG5 :: (((98) << 24) | ((105) << 16) | ((103) << 8) | (53));
    WANSUNG :: (((119) << 24) | ((97) << 16) | ((110) << 8) | (115));
    JOHAB :: (((106) << 24) | ((111) << 16) | ((104) << 8) | (97));
    GB2312 :: PRC;
    MS_SJIS :: SJIS;
    MS_GB2312 :: PRC;
    MS_BIG5 :: BIG5;
    MS_WANSUNG :: WANSUNG;
    MS_JOHAB :: JOHAB;
    ADOBE_STANDARD :: (((65) << 24) | ((68) << 16) | ((79) << 8) | (66));
    ADOBE_EXPERT :: (((65) << 24) | ((68) << 16) | ((66) << 8) | (69));
    ADOBE_CUSTOM :: (((65) << 24) | ((68) << 16) | ((66) << 8) | (67));
    ADOBE_LATIN_1 :: (((108) << 24) | ((97) << 16) | ((116) << 8) | (49));
    OLD_LATIN_2 :: (((108) << 24) | ((97) << 16) | ((116) << 8) | (50));
    APPLE_ROMAN :: (((97) << 24) | ((114) << 16) | ((109) << 8) | (110));

    FT_ENCODING_NONE :: NONE;
    FT_ENCODING_MS_SYMBOL :: MS_SYMBOL;
    FT_ENCODING_UNICODE :: UNICODE;
    FT_ENCODING_SJIS :: SJIS;
    FT_ENCODING_PRC :: PRC;
    FT_ENCODING_BIG5 :: BIG5;
    FT_ENCODING_WANSUNG :: WANSUNG;
    FT_ENCODING_JOHAB :: JOHAB;
    FT_ENCODING_GB2312 :: GB2312;
    FT_ENCODING_MS_SJIS :: MS_SJIS;
    FT_ENCODING_MS_GB2312 :: MS_GB2312;
    FT_ENCODING_MS_BIG5 :: MS_BIG5;
    FT_ENCODING_MS_WANSUNG :: MS_WANSUNG;
    FT_ENCODING_MS_JOHAB :: MS_JOHAB;
    FT_ENCODING_ADOBE_STANDARD :: ADOBE_STANDARD;
    FT_ENCODING_ADOBE_EXPERT :: ADOBE_EXPERT;
    FT_ENCODING_ADOBE_CUSTOM :: ADOBE_CUSTOM;
    FT_ENCODING_ADOBE_LATIN_1 :: ADOBE_LATIN_1;
    FT_ENCODING_OLD_LATIN_2 :: OLD_LATIN_2;
    FT_ENCODING_APPLE_ROMAN :: APPLE_ROMAN;
}
FT_Encoding :: FT_Encoding_;

FT_Size_Request_Type_ :: enum s32 {
    NOMINAL;
    REAL_DIM;
    BBOX;
    CELL;
    SCALES;
    MAX;

    FT_SIZE_REQUEST_TYPE_NOMINAL :: NOMINAL;
    FT_SIZE_REQUEST_TYPE_REAL_DIM :: REAL_DIM;
    FT_SIZE_REQUEST_TYPE_BBOX :: BBOX;
    FT_SIZE_REQUEST_TYPE_CELL :: CELL;
    FT_SIZE_REQUEST_TYPE_SCALES :: SCALES;
    FT_SIZE_REQUEST_TYPE_MAX :: MAX;
}
FT_Size_Request_Type :: FT_Size_Request_Type_;

FT_Render_Mode_ :: enum s32 {
    NORMAL :: 0;
    LIGHT;
    MONO;
    LCD;
    LCD_V;
    SDF;
    MAX;

    FT_RENDER_MODE_NORMAL :: NORMAL;
    FT_RENDER_MODE_LIGHT :: LIGHT;
    FT_RENDER_MODE_MONO :: MONO;
    FT_RENDER_MODE_LCD :: LCD;
    FT_RENDER_MODE_LCD_V :: LCD_V;
    FT_RENDER_MODE_SDF :: SDF;
    FT_RENDER_MODE_MAX :: MAX;
}
FT_Render_Mode :: FT_Render_Mode_;

FT_Kerning_Mode_ :: enum s32 {
    DEFAULT :: 0;
    UNFITTED;
    UNSCALED;

    FT_KERNING_DEFAULT :: DEFAULT;
    FT_KERNING_UNFITTED :: UNFITTED;
    FT_KERNING_UNSCALED :: UNSCALED;
}
FT_Kerning_Mode :: FT_Kerning_Mode_;

FT_Glyph_BBox_Mode_ :: enum s32 {
    UNSCALED :: 0;
    SUBPIXELS :: 0;
    GRIDFIT :: 1;
    TRUNCATE :: 2;
    PIXELS :: 3;

    FT_GLYPH_BBOX_UNSCALED :: UNSCALED;
    FT_GLYPH_BBOX_SUBPIXELS :: SUBPIXELS;
    FT_GLYPH_BBOX_GRIDFIT :: GRIDFIT;
    FT_GLYPH_BBOX_TRUNCATE :: TRUNCATE;
    FT_GLYPH_BBOX_PIXELS :: PIXELS;
}
FT_Glyph_BBox_Mode :: FT_Glyph_BBox_Mode_;

FT_Orientation_ :: enum s32 {
    TRUETYPE :: 0;
    POSTSCRIPT :: 1;
    FILL_RIGHT :: TRUETYPE;
    FILL_LEFT :: POSTSCRIPT;
    NONE;

    FT_ORIENTATION_TRUETYPE :: TRUETYPE;
    FT_ORIENTATION_POSTSCRIPT :: POSTSCRIPT;
    FT_ORIENTATION_FILL_RIGHT :: FILL_RIGHT;
    FT_ORIENTATION_FILL_LEFT :: FILL_LEFT;
    FT_ORIENTATION_NONE :: NONE;
}
FT_Orientation :: FT_Orientation_;

FT_PaintFormat_ :: enum s32 {
    PAINTFORMAT_COLR_LAYERS :: 1;
    PAINTFORMAT_SOLID :: 2;
    PAINTFORMAT_LINEAR_GRADIENT :: 4;
    PAINTFORMAT_RADIAL_GRADIENT :: 6;
    PAINTFORMAT_SWEEP_GRADIENT :: 8;
    PAINTFORMAT_GLYPH :: 10;
    PAINTFORMAT_COLR_GLYPH :: 11;
    PAINTFORMAT_TRANSFORM :: 12;
    PAINTFORMAT_TRANSLATE :: 14;
    PAINTFORMAT_SCALE :: 16;
    PAINTFORMAT_ROTATE :: 24;
    PAINTFORMAT_SKEW :: 28;
    PAINTFORMAT_COMPOSITE :: 32;
    PAINT_FORMAT_MAX :: 33;
    PAINTFORMAT_UNSUPPORTED :: 255;

    FT_COLR_PAINTFORMAT_COLR_LAYERS :: PAINTFORMAT_COLR_LAYERS;
    FT_COLR_PAINTFORMAT_SOLID :: PAINTFORMAT_SOLID;
    FT_COLR_PAINTFORMAT_LINEAR_GRADIENT :: PAINTFORMAT_LINEAR_GRADIENT;
    FT_COLR_PAINTFORMAT_RADIAL_GRADIENT :: PAINTFORMAT_RADIAL_GRADIENT;
    FT_COLR_PAINTFORMAT_SWEEP_GRADIENT :: PAINTFORMAT_SWEEP_GRADIENT;
    FT_COLR_PAINTFORMAT_GLYPH :: PAINTFORMAT_GLYPH;
    FT_COLR_PAINTFORMAT_COLR_GLYPH :: PAINTFORMAT_COLR_GLYPH;
    FT_COLR_PAINTFORMAT_TRANSFORM :: PAINTFORMAT_TRANSFORM;
    FT_COLR_PAINTFORMAT_TRANSLATE :: PAINTFORMAT_TRANSLATE;
    FT_COLR_PAINTFORMAT_SCALE :: PAINTFORMAT_SCALE;
    FT_COLR_PAINTFORMAT_ROTATE :: PAINTFORMAT_ROTATE;
    FT_COLR_PAINTFORMAT_SKEW :: PAINTFORMAT_SKEW;
    FT_COLR_PAINTFORMAT_COMPOSITE :: PAINTFORMAT_COMPOSITE;
    FT_COLR_PAINT_FORMAT_MAX :: PAINT_FORMAT_MAX;
    FT_COLR_PAINTFORMAT_UNSUPPORTED :: PAINTFORMAT_UNSUPPORTED;
}
FT_PaintFormat :: FT_PaintFormat_;

FT_PaintExtend_ :: enum s32 {
    PAD :: 0;
    REPEAT :: 1;
    REFLECT :: 2;

    FT_COLR_PAINT_EXTEND_PAD :: PAD;
    FT_COLR_PAINT_EXTEND_REPEAT :: REPEAT;
    FT_COLR_PAINT_EXTEND_REFLECT :: REFLECT;
}
FT_PaintExtend :: FT_PaintExtend_;

FT_Composite_Mode_ :: enum s32 {
    CLEAR :: 0;
    SRC :: 1;
    DEST :: 2;
    SRC_OVER :: 3;
    DEST_OVER :: 4;
    SRC_IN :: 5;
    DEST_IN :: 6;
    SRC_OUT :: 7;
    DEST_OUT :: 8;
    SRC_ATOP :: 9;
    DEST_ATOP :: 10;
    XOR :: 11;
    PLUS :: 12;
    SCREEN :: 13;
    OVERLAY :: 14;
    DARKEN :: 15;
    LIGHTEN :: 16;
    COLOR_DODGE :: 17;
    COLOR_BURN :: 18;
    HARD_LIGHT :: 19;
    SOFT_LIGHT :: 20;
    DIFFERENCE :: 21;
    EXCLUSION :: 22;
    MULTIPLY :: 23;
    HSL_HUE :: 24;
    HSL_SATURATION :: 25;
    HSL_COLOR :: 26;
    HSL_LUMINOSITY :: 27;
    MAX :: 28;

    FT_COLR_COMPOSITE_CLEAR :: CLEAR;
    FT_COLR_COMPOSITE_SRC :: SRC;
    FT_COLR_COMPOSITE_DEST :: DEST;
    FT_COLR_COMPOSITE_SRC_OVER :: SRC_OVER;
    FT_COLR_COMPOSITE_DEST_OVER :: DEST_OVER;
    FT_COLR_COMPOSITE_SRC_IN :: SRC_IN;
    FT_COLR_COMPOSITE_DEST_IN :: DEST_IN;
    FT_COLR_COMPOSITE_SRC_OUT :: SRC_OUT;
    FT_COLR_COMPOSITE_DEST_OUT :: DEST_OUT;
    FT_COLR_COMPOSITE_SRC_ATOP :: SRC_ATOP;
    FT_COLR_COMPOSITE_DEST_ATOP :: DEST_ATOP;
    FT_COLR_COMPOSITE_XOR :: XOR;
    FT_COLR_COMPOSITE_PLUS :: PLUS;
    FT_COLR_COMPOSITE_SCREEN :: SCREEN;
    FT_COLR_COMPOSITE_OVERLAY :: OVERLAY;
    FT_COLR_COMPOSITE_DARKEN :: DARKEN;
    FT_COLR_COMPOSITE_LIGHTEN :: LIGHTEN;
    FT_COLR_COMPOSITE_COLOR_DODGE :: COLOR_DODGE;
    FT_COLR_COMPOSITE_COLOR_BURN :: COLOR_BURN;
    FT_COLR_COMPOSITE_HARD_LIGHT :: HARD_LIGHT;
    FT_COLR_COMPOSITE_SOFT_LIGHT :: SOFT_LIGHT;
    FT_COLR_COMPOSITE_DIFFERENCE :: DIFFERENCE;
    FT_COLR_COMPOSITE_EXCLUSION :: EXCLUSION;
    FT_COLR_COMPOSITE_MULTIPLY :: MULTIPLY;
    FT_COLR_COMPOSITE_HSL_HUE :: HSL_HUE;
    FT_COLR_COMPOSITE_HSL_SATURATION :: HSL_SATURATION;
    FT_COLR_COMPOSITE_HSL_COLOR :: HSL_COLOR;
    FT_COLR_COMPOSITE_HSL_LUMINOSITY :: HSL_LUMINOSITY;
    FT_COLR_COMPOSITE_MAX :: MAX;
}
FT_Composite_Mode :: FT_Composite_Mode_;

FT_Color_Root_Transform_ :: enum s32 {
    INCLUDE_ROOT_TRANSFORM;
    NO_ROOT_TRANSFORM;
    ROOT_TRANSFORM_MAX;

    FT_COLOR_INCLUDE_ROOT_TRANSFORM :: INCLUDE_ROOT_TRANSFORM;
    FT_COLOR_NO_ROOT_TRANSFORM :: NO_ROOT_TRANSFORM;
    FT_COLOR_ROOT_TRANSFORM_MAX :: ROOT_TRANSFORM_MAX;
}
FT_Color_Root_Transform :: FT_Color_Root_Transform_;

FT_Stroker_LineJoin_ :: enum s32 {
    ROUND :: 0;
    BEVEL :: 1;
    MITER_VARIABLE :: 2;
    MITER :: MITER_VARIABLE;
    MITER_FIXED :: 3;

    FT_STROKER_LINEJOIN_ROUND :: ROUND;
    FT_STROKER_LINEJOIN_BEVEL :: BEVEL;
    FT_STROKER_LINEJOIN_MITER_VARIABLE :: MITER_VARIABLE;
    FT_STROKER_LINEJOIN_MITER :: MITER;
    FT_STROKER_LINEJOIN_MITER_FIXED :: MITER_FIXED;
}
FT_Stroker_LineJoin :: FT_Stroker_LineJoin_;

FT_Stroker_LineCap_ :: enum s32 {
    BUTT :: 0;
    ROUND;
    SQUARE;

    FT_STROKER_LINECAP_BUTT :: BUTT;
    FT_STROKER_LINECAP_ROUND :: ROUND;
    FT_STROKER_LINECAP_SQUARE :: SQUARE;
}
FT_Stroker_LineCap :: FT_Stroker_LineCap_;

FT_StrokerBorder_ :: enum s32 {
    LEFT :: 0;
    RIGHT;

    FT_STROKER_BORDER_LEFT :: LEFT;
    FT_STROKER_BORDER_RIGHT :: RIGHT;
}
FT_StrokerBorder :: FT_StrokerBorder_;

FT_LcdFilter_ :: enum s32 {
    NONE :: 0;
    DEFAULT :: 1;
    LIGHT :: 2;
    LEGACY1 :: 3;
    LEGACY :: 16;
    MAX;

    FT_LCD_FILTER_NONE :: NONE;
    FT_LCD_FILTER_DEFAULT :: DEFAULT;
    FT_LCD_FILTER_LIGHT :: LIGHT;
    FT_LCD_FILTER_LEGACY1 :: LEGACY1;
    FT_LCD_FILTER_LEGACY :: LEGACY;
    FT_LCD_FILTER_MAX :: MAX;
}
FT_LcdFilter :: FT_LcdFilter_;

FT_TrueTypeEngineType_ :: enum s32 {
    NONE :: 0;
    UNPATENTED;
    PATENTED;

    FT_TRUETYPE_ENGINE_TYPE_NONE :: NONE;
    FT_TRUETYPE_ENGINE_TYPE_UNPATENTED :: UNPATENTED;
    FT_TRUETYPE_ENGINE_TYPE_PATENTED :: PATENTED;
}
FT_TrueTypeEngineType :: FT_TrueTypeEngineType_;

FT_Sfnt_Tag_ :: enum s32 {
    HEAD;
    MAXP;
    OS2;
    HHEA;
    VHEA;
    POST;
    PCLT;
    MAX;

    FT_SFNT_HEAD :: HEAD;
    FT_SFNT_MAXP :: MAXP;
    FT_SFNT_OS2 :: OS2;
    FT_SFNT_HHEA :: HHEA;
    FT_SFNT_VHEA :: VHEA;
    FT_SFNT_POST :: POST;
    FT_SFNT_PCLT :: PCLT;
    FT_SFNT_MAX :: MAX;
}
FT_Sfnt_Tag :: FT_Sfnt_Tag_;


// ------------------------------------------------------------- procedures ---

FT_Activate_Size :: (size: FT_Size) -> FT_Error #foreign libfreetype;
FT_Add_Default_Modules :: (library: FT_Library) -> void #foreign libfreetype;
FT_Add_Module :: (library: FT_Library, clazz: *FT_Module_Class) -> FT_Error #foreign libfreetype;
FT_Angle_Diff :: (angle1: FT_Angle, angle2: FT_Angle) -> FT_Angle #foreign libfreetype;
FT_Atan2 :: (x: FT_Fixed, y: FT_Fixed) -> FT_Angle #foreign libfreetype;
FT_Attach_File :: (face: FT_Face, filepathname: *u8) -> FT_Error #foreign libfreetype;
FT_Attach_Stream :: (face: FT_Face, parameters: *FT_Open_Args) -> FT_Error #foreign libfreetype;
FT_Bitmap_Blend :: (library: FT_Library, source: *FT_Bitmap, source_offset: FT_Vector, target: *FT_Bitmap, atarget_offset: *FT_Vector, color: FT_Color) -> FT_Error #foreign libfreetype;
FT_Bitmap_Convert :: (library: FT_Library, source: *FT_Bitmap, target: *FT_Bitmap, alignment: FT_Int) -> FT_Error #foreign libfreetype;
FT_Bitmap_Copy :: (library: FT_Library, source: *FT_Bitmap, target: *FT_Bitmap) -> FT_Error #foreign libfreetype;
FT_Bitmap_Done :: (library: FT_Library, bitmap: *FT_Bitmap) -> FT_Error #foreign libfreetype;
FT_Bitmap_Embolden :: (library: FT_Library, bitmap: *FT_Bitmap, xStrength: FT_Pos, yStrength: FT_Pos) -> FT_Error #foreign libfreetype;
FT_Bitmap_Init :: (abitmap: *FT_Bitmap) -> void #foreign libfreetype;
FT_Bitmap_New :: (abitmap: *FT_Bitmap) -> void #foreign libfreetype;
FT_CeilFix :: (a: FT_Fixed) -> FT_Fixed #foreign libfreetype;
FT_Cos :: (angle: FT_Angle) -> FT_Fixed #foreign libfreetype;
FT_DivFix :: (a: FT_Long, b: FT_Long) -> FT_Long #foreign libfreetype;
FT_Done_Face :: (face: FT_Face) -> FT_Error #foreign libfreetype;
FT_Done_FreeType :: (library: FT_Library) -> FT_Error #foreign libfreetype;
FT_Done_Glyph :: (glyph: FT_Glyph) -> void #foreign libfreetype;
FT_Done_Library :: (library: FT_Library) -> FT_Error #foreign libfreetype;
FT_Done_MM_Var :: (library: FT_Library, amaster: *FT_MM_Var) -> FT_Error #foreign libfreetype;
FT_Done_Size :: (size: FT_Size) -> FT_Error #foreign libfreetype;
FT_Error_String :: (error_code: FT_Error) -> *u8 #foreign libfreetype;
FT_Face_CheckTrueTypePatents :: (face: FT_Face) -> FT_Bool #foreign libfreetype;
FT_Face_GetCharVariantIndex :: (face: FT_Face, charcode: FT_ULong, variantSelector: FT_ULong) -> FT_UInt #foreign libfreetype;
FT_Face_GetCharVariantIsDefault :: (face: FT_Face, charcode: FT_ULong, variantSelector: FT_ULong) -> FT_Int #foreign libfreetype;
FT_Face_GetCharsOfVariant :: (face: FT_Face, variantSelector: FT_ULong) -> *FT_UInt32 #foreign libfreetype;
FT_Face_GetVariantSelectors :: (face: FT_Face) -> *FT_UInt32 #foreign libfreetype;
FT_Face_GetVariantsOfChar :: (face: FT_Face, charcode: FT_ULong) -> *FT_UInt32 #foreign libfreetype;
FT_Face_Properties :: (face: FT_Face, num_properties: FT_UInt, properties: *FT_Parameter) -> FT_Error #foreign libfreetype;
FT_Face_SetUnpatentedHinting :: (face: FT_Face, value: FT_Bool) -> FT_Bool #foreign libfreetype;
FT_FloorFix :: (a: FT_Fixed) -> FT_Fixed #foreign libfreetype;
FT_Get_Advance :: (face: FT_Face, gindex: FT_UInt, load_flags: FT_Int32, padvance: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Get_Advances :: (face: FT_Face, start: FT_UInt, count: FT_UInt, load_flags: FT_Int32, padvances: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Get_CMap_Format :: (charmap: FT_CharMap) -> FT_Long #foreign libfreetype;
FT_Get_CMap_Language_ID :: (charmap: FT_CharMap) -> FT_ULong #foreign libfreetype;
FT_Get_Char_Index :: (face: FT_Face, charcode: FT_ULong) -> FT_UInt #foreign libfreetype;
FT_Get_Charmap_Index :: (charmap: FT_CharMap) -> FT_Int #foreign libfreetype;
FT_Get_Color_Glyph_ClipBox :: (face: FT_Face, base_glyph: FT_UInt, clip_box: *FT_ClipBox) -> FT_Bool #foreign libfreetype;
FT_Get_Color_Glyph_Layer :: (face: FT_Face, base_glyph: FT_UInt, aglyph_index: *FT_UInt, acolor_index: *FT_UInt, iterator: *FT_LayerIterator) -> FT_Bool #foreign libfreetype;
FT_Get_Color_Glyph_Paint :: (face: FT_Face, base_glyph: FT_UInt, root_transform: FT_Color_Root_Transform, paint: *FT_OpaquePaint) -> FT_Bool #foreign libfreetype;
FT_Get_Colorline_Stops :: (face: FT_Face, color_stop: *FT_ColorStop, iterator: *FT_ColorStopIterator) -> FT_Bool #foreign libfreetype;
FT_Get_Default_Named_Instance :: (face: FT_Face, instance_index: *FT_UInt) -> FT_Error #foreign libfreetype;
FT_Get_FSType_Flags :: (face: FT_Face) -> FT_UShort #foreign libfreetype;
FT_Get_First_Char :: (face: FT_Face, agindex: *FT_UInt) -> FT_ULong #foreign libfreetype;
FT_Get_Font_Format :: (face: FT_Face) -> *u8 #foreign libfreetype;
FT_Get_Glyph :: (slot: FT_GlyphSlot, aglyph: *FT_Glyph) -> FT_Error #foreign libfreetype;
FT_Get_Glyph_Name :: (face: FT_Face, glyph_index: FT_UInt, buffer: FT_Pointer, buffer_max: FT_UInt) -> FT_Error #foreign libfreetype;
FT_Get_Kerning :: (face: FT_Face, left_glyph: FT_UInt, right_glyph: FT_UInt, kern_mode: FT_UInt, akerning: *FT_Vector) -> FT_Error #foreign libfreetype;
FT_Get_MM_Blend_Coordinates :: (face: FT_Face, num_coords: FT_UInt, coords: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Get_MM_Var :: (face: FT_Face, amaster: **FT_MM_Var) -> FT_Error #foreign libfreetype;
FT_Get_MM_WeightVector :: (face: FT_Face, len: *FT_UInt, weightvector: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Get_Module :: (library: FT_Library, module_name: *u8) -> FT_Module #foreign libfreetype;
FT_Get_Multi_Master :: (face: FT_Face, amaster: *FT_Multi_Master) -> FT_Error #foreign libfreetype;
FT_Get_Name_Index :: (face: FT_Face, glyph_name: *FT_String) -> FT_UInt #foreign libfreetype;
FT_Get_Next_Char :: (face: FT_Face, char_code: FT_ULong, agindex: *FT_UInt) -> FT_ULong #foreign libfreetype;
FT_Get_Paint :: (face: FT_Face, opaque_paint: FT_OpaquePaint, paint: *FT_COLR_Paint) -> FT_Bool #foreign libfreetype;
FT_Get_Paint_Layers :: (face: FT_Face, iterator: *FT_LayerIterator, paint: *FT_OpaquePaint) -> FT_Bool #foreign libfreetype;
FT_Get_Postscript_Name :: (face: FT_Face) -> *u8 #foreign libfreetype;
FT_Get_Sfnt_LangTag :: (face: FT_Face, langID: FT_UInt, alangTag: *FT_SfntLangTag) -> FT_Error #foreign libfreetype;
FT_Get_Sfnt_Name :: (face: FT_Face, idx: FT_UInt, aname: *FT_SfntName) -> FT_Error #foreign libfreetype;
FT_Get_Sfnt_Name_Count :: (face: FT_Face) -> FT_UInt #foreign libfreetype;
FT_Get_Sfnt_Table :: (face: FT_Face, tag: FT_Sfnt_Tag) -> *void #foreign libfreetype;
FT_Get_SubGlyph_Info :: (glyph: FT_GlyphSlot, sub_index: FT_UInt, p_index: *FT_Int, p_flags: *FT_UInt, p_arg1: *FT_Int, p_arg2: *FT_Int, p_transform: *FT_Matrix) -> FT_Error #foreign libfreetype;
FT_Get_Track_Kerning :: (face: FT_Face, point_size: FT_Fixed, degree: FT_Int, akerning: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Get_Transform :: (face: FT_Face, matrix: *FT_Matrix, delta: *FT_Vector) -> void #foreign libfreetype;
FT_Get_TrueType_Engine_Type :: (library: FT_Library) -> FT_TrueTypeEngineType #foreign libfreetype;
FT_Get_Var_Axis_Flags :: (master: *FT_MM_Var, axis_index: FT_UInt, flags: *FT_UInt) -> FT_Error #foreign libfreetype;
FT_Get_Var_Blend_Coordinates :: (face: FT_Face, num_coords: FT_UInt, coords: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Get_Var_Design_Coordinates :: (face: FT_Face, num_coords: FT_UInt, coords: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Get_X11_Font_Format :: (face: FT_Face) -> *u8 #foreign libfreetype;
FT_GlyphSlot_AdjustWeight :: (slot: FT_GlyphSlot, xdelta: FT_Fixed, ydelta: FT_Fixed) -> void #foreign libfreetype;
FT_GlyphSlot_Embolden :: (slot: FT_GlyphSlot) -> void #foreign libfreetype;
FT_GlyphSlot_Oblique :: (slot: FT_GlyphSlot) -> void #foreign libfreetype;
FT_GlyphSlot_Own_Bitmap :: (slot: FT_GlyphSlot) -> FT_Error #foreign libfreetype;
FT_GlyphSlot_Slant :: (slot: FT_GlyphSlot, xslant: FT_Fixed, yslant: FT_Fixed) -> void #foreign libfreetype;
FT_Glyph_Copy :: (source: FT_Glyph, target: *FT_Glyph) -> FT_Error #foreign libfreetype;
FT_Glyph_Get_CBox :: (glyph: FT_Glyph, bbox_mode: FT_UInt, acbox: *FT_BBox) -> void #foreign libfreetype;
FT_Glyph_Stroke :: (pglyph: *FT_Glyph, stroker: FT_Stroker, destroy: FT_Bool) -> FT_Error #foreign libfreetype;
FT_Glyph_StrokeBorder :: (pglyph: *FT_Glyph, stroker: FT_Stroker, inside: FT_Bool, destroy: FT_Bool) -> FT_Error #foreign libfreetype;
FT_Glyph_To_Bitmap :: (the_glyph: *FT_Glyph, render_mode: FT_Render_Mode, origin: *FT_Vector, destroy: FT_Bool) -> FT_Error #foreign libfreetype;
FT_Glyph_Transform :: (glyph: FT_Glyph, matrix: *FT_Matrix, delta: *FT_Vector) -> FT_Error #foreign libfreetype;
FT_Init_FreeType :: (alibrary: *FT_Library) -> FT_Error #foreign libfreetype;
FT_Library_SetLcdFilter :: (library: FT_Library, filter: FT_LcdFilter) -> FT_Error #foreign libfreetype;
FT_Library_SetLcdFilterWeights :: (library: FT_Library, weights: *u8) -> FT_Error #foreign libfreetype;
FT_Library_SetLcdGeometry :: (library: FT_Library, sub: *FT_Vector) -> FT_Error #foreign libfreetype;
FT_Library_Version :: (library: FT_Library, amajor: *FT_Int, aminor: *FT_Int, apatch: *FT_Int) -> void #foreign libfreetype;
FT_Load_Char :: (face: FT_Face, char_code: FT_ULong, load_flags: FT_Int32) -> FT_Error #foreign libfreetype;
FT_Load_Glyph :: (face: FT_Face, glyph_index: FT_UInt, load_flags: FT_Int32) -> FT_Error #foreign libfreetype;
FT_Load_Sfnt_Table :: (face: FT_Face, tag: FT_ULong, offset: FT_Long, buffer: *FT_Byte, length: *FT_ULong) -> FT_Error #foreign libfreetype;
FT_Matrix_Invert :: (matrix: *FT_Matrix) -> FT_Error #foreign libfreetype;
FT_Matrix_Multiply :: (a: *FT_Matrix, b: *FT_Matrix) -> void #foreign libfreetype;
FT_MulDiv :: (a: FT_Long, b: FT_Long, c: FT_Long) -> FT_Long #foreign libfreetype;
FT_MulFix :: (a: FT_Long, b: FT_Long) -> FT_Long #foreign libfreetype;
FT_New_Face :: (library: FT_Library, filepathname: *u8, face_index: FT_Long, aface: *FT_Face) -> FT_Error #foreign libfreetype;
FT_New_Glyph :: (library: FT_Library, format: FT_Glyph_Format, aglyph: *FT_Glyph) -> FT_Error #foreign libfreetype;
FT_New_Library :: (memory: FT_Memory, alibrary: *FT_Library) -> FT_Error #foreign libfreetype;
FT_New_Memory_Face :: (library: FT_Library, file_base: *FT_Byte, file_size: FT_Long, face_index: FT_Long, aface: *FT_Face) -> FT_Error #foreign libfreetype;
FT_New_Size :: (face: FT_Face, size: *FT_Size) -> FT_Error #foreign libfreetype;
FT_Open_Face :: (library: FT_Library, args: *FT_Open_Args, face_index: FT_Long, aface: *FT_Face) -> FT_Error #foreign libfreetype;
FT_Outline_Check :: (outline: *FT_Outline) -> FT_Error #foreign libfreetype;
FT_Outline_Copy :: (source: *FT_Outline, target: *FT_Outline) -> FT_Error #foreign libfreetype;
FT_Outline_Decompose :: (outline: *FT_Outline, func_interface: *FT_Outline_Funcs, user: *void) -> FT_Error #foreign libfreetype;
FT_Outline_Done :: (library: FT_Library, outline: *FT_Outline) -> FT_Error #foreign libfreetype;
FT_Outline_Embolden :: (outline: *FT_Outline, strength: FT_Pos) -> FT_Error #foreign libfreetype;
FT_Outline_EmboldenXY :: (outline: *FT_Outline, xstrength: FT_Pos, ystrength: FT_Pos) -> FT_Error #foreign libfreetype;
FT_Outline_GetInsideBorder :: (outline: *FT_Outline) -> FT_StrokerBorder #foreign libfreetype;
FT_Outline_GetOutsideBorder :: (outline: *FT_Outline) -> FT_StrokerBorder #foreign libfreetype;
FT_Outline_Get_BBox :: (outline: *FT_Outline, abbox: *FT_BBox) -> FT_Error #foreign libfreetype;
FT_Outline_Get_Bitmap :: (library: FT_Library, outline: *FT_Outline, abitmap: *FT_Bitmap) -> FT_Error #foreign libfreetype;
FT_Outline_Get_CBox :: (outline: *FT_Outline, acbox: *FT_BBox) -> void #foreign libfreetype;
FT_Outline_Get_Orientation :: (outline: *FT_Outline) -> FT_Orientation #foreign libfreetype;
FT_Outline_New :: (library: FT_Library, numPoints: FT_UInt, numContours: FT_Int, anoutline: *FT_Outline) -> FT_Error #foreign libfreetype;
FT_Outline_Render :: (library: FT_Library, outline: *FT_Outline, params: *FT_Raster_Params) -> FT_Error #foreign libfreetype;
FT_Outline_Reverse :: (outline: *FT_Outline) -> void #foreign libfreetype;
FT_Outline_Transform :: (outline: *FT_Outline, matrix: *FT_Matrix) -> void #foreign libfreetype;
FT_Outline_Translate :: (outline: *FT_Outline, xOffset: FT_Pos, yOffset: FT_Pos) -> void #foreign libfreetype;
FT_Palette_Data_Get :: (face: FT_Face, apalette: *FT_Palette_Data) -> FT_Error #foreign libfreetype;
FT_Palette_Select :: (face: FT_Face, palette_index: FT_UShort, apalette: **FT_Color) -> FT_Error #foreign libfreetype;
FT_Palette_Set_Foreground_Color :: (face: FT_Face, foreground_color: FT_Color) -> FT_Error #foreign libfreetype;
FT_Property_Get :: (library: FT_Library, module_name: *FT_String, property_name: *FT_String, value: *void) -> FT_Error #foreign libfreetype;
FT_Property_Set :: (library: FT_Library, module_name: *FT_String, property_name: *FT_String, value: *void) -> FT_Error #foreign libfreetype;
FT_Reference_Face :: (face: FT_Face) -> FT_Error #foreign libfreetype;
FT_Reference_Library :: (library: FT_Library) -> FT_Error #foreign libfreetype;
FT_Remove_Module :: (library: FT_Library, module: FT_Module) -> FT_Error #foreign libfreetype;
FT_Render_Glyph :: (slot: FT_GlyphSlot, render_mode: FT_Render_Mode) -> FT_Error #foreign libfreetype;
FT_Request_Size :: (face: FT_Face, req: FT_Size_Request) -> FT_Error #foreign libfreetype;
FT_RoundFix :: (a: FT_Fixed) -> FT_Fixed #foreign libfreetype;
FT_Select_Charmap :: (face: FT_Face, encoding: FT_Encoding) -> FT_Error #foreign libfreetype;
FT_Select_Size :: (face: FT_Face, strike_index: FT_Int) -> FT_Error #foreign libfreetype;
FT_Set_Char_Size :: (face: FT_Face, char_width: FT_F26Dot6, char_height: FT_F26Dot6, horz_resolution: FT_UInt, vert_resolution: FT_UInt) -> FT_Error #foreign libfreetype;
FT_Set_Charmap :: (face: FT_Face, charmap: FT_CharMap) -> FT_Error #foreign libfreetype;
FT_Set_Debug_Hook :: (library: FT_Library, hook_index: FT_UInt, debug_hook: FT_DebugHook_Func) -> void #foreign libfreetype;
FT_Set_Default_Properties :: (library: FT_Library) -> void #foreign libfreetype;
FT_Set_MM_Blend_Coordinates :: (face: FT_Face, num_coords: FT_UInt, coords: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Set_MM_Design_Coordinates :: (face: FT_Face, num_coords: FT_UInt, coords: *FT_Long) -> FT_Error #foreign libfreetype;
FT_Set_MM_WeightVector :: (face: FT_Face, len: FT_UInt, weightvector: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Set_Named_Instance :: (face: FT_Face, instance_index: FT_UInt) -> FT_Error #foreign libfreetype;
FT_Set_Pixel_Sizes :: (face: FT_Face, pixel_width: FT_UInt, pixel_height: FT_UInt) -> FT_Error #foreign libfreetype;
FT_Set_Transform :: (face: FT_Face, matrix: *FT_Matrix, delta: *FT_Vector) -> void #foreign libfreetype;
FT_Set_Var_Blend_Coordinates :: (face: FT_Face, num_coords: FT_UInt, coords: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Set_Var_Design_Coordinates :: (face: FT_Face, num_coords: FT_UInt, coords: *FT_Fixed) -> FT_Error #foreign libfreetype;
FT_Sfnt_Table_Info :: (face: FT_Face, table_index: FT_UInt, tag: *FT_ULong, length: *FT_ULong) -> FT_Error #foreign libfreetype;
FT_Sin :: (angle: FT_Angle) -> FT_Fixed #foreign libfreetype;
FT_Stroker_BeginSubPath :: (stroker: FT_Stroker, to: *FT_Vector, open: FT_Bool) -> FT_Error #foreign libfreetype;
FT_Stroker_ConicTo :: (stroker: FT_Stroker, control: *FT_Vector, to: *FT_Vector) -> FT_Error #foreign libfreetype;
FT_Stroker_CubicTo :: (stroker: FT_Stroker, control1: *FT_Vector, control2: *FT_Vector, to: *FT_Vector) -> FT_Error #foreign libfreetype;
FT_Stroker_Done :: (stroker: FT_Stroker) -> void #foreign libfreetype;
FT_Stroker_EndSubPath :: (stroker: FT_Stroker) -> FT_Error #foreign libfreetype;
FT_Stroker_Export :: (stroker: FT_Stroker, outline: *FT_Outline) -> void #foreign libfreetype;
FT_Stroker_ExportBorder :: (stroker: FT_Stroker, border: FT_StrokerBorder, outline: *FT_Outline) -> void #foreign libfreetype;
FT_Stroker_GetBorderCounts :: (stroker: FT_Stroker, border: FT_StrokerBorder, anum_points: *FT_UInt, anum_contours: *FT_UInt) -> FT_Error #foreign libfreetype;
FT_Stroker_GetCounts :: (stroker: FT_Stroker, anum_points: *FT_UInt, anum_contours: *FT_UInt) -> FT_Error #foreign libfreetype;
FT_Stroker_LineTo :: (stroker: FT_Stroker, to: *FT_Vector) -> FT_Error #foreign libfreetype;
FT_Stroker_New :: (library: FT_Library, astroker: *FT_Stroker) -> FT_Error #foreign libfreetype;
FT_Stroker_ParseOutline :: (stroker: FT_Stroker, outline: *FT_Outline, opened: FT_Bool) -> FT_Error #foreign libfreetype;
FT_Stroker_Rewind :: (stroker: FT_Stroker) -> void #foreign libfreetype;
FT_Stroker_Set :: (stroker: FT_Stroker, radius: FT_Fixed, line_cap: FT_Stroker_LineCap, line_join: FT_Stroker_LineJoin, miter_limit: FT_Fixed) -> void #foreign libfreetype;
FT_Tan :: (angle: FT_Angle) -> FT_Fixed #foreign libfreetype;
FT_Vector_From_Polar :: (vec: *FT_Vector, length: FT_Fixed, angle: FT_Angle) -> void #foreign libfreetype;
FT_Vector_Length :: (vec: *FT_Vector) -> FT_Fixed #foreign libfreetype;
FT_Vector_Polarize :: (vec: *FT_Vector, length: *FT_Fixed, angle: *FT_Angle) -> void #foreign libfreetype;
FT_Vector_Rotate :: (vec: *FT_Vector, angle: FT_Angle) -> void #foreign libfreetype;
FT_Vector_Transform :: (vector: *FT_Vector, matrix: *FT_Matrix) -> void #foreign libfreetype;
FT_Vector_Unit :: (vec: *FT_Vector, angle: FT_Angle) -> void #foreign libfreetype;

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

