//
// Dynamic_Font: text through FreeType, rasterized a glyph at a time into
// atlas pages as it is needed.
//
//     font := get_font_at_size("data/fonts", "OpenSans.ttf", 24);
//     width := prepare_text(font, "Score: 10");
//     draw_prepared_text(font, x, y, .{1, 1, 1, 1});
//
// `x, y` is where the text's baseline starts. `prepare_text` lays a string
// out — glyphs, kerning, the width it takes — and `draw_prepared_text`
// draws what was prepared last; `draw_text` does both. Prepared text is per
// font, so preparing with one font does not disturb what another prepared.
//
// A font is opened once per file and size: asking again for the same pair
// returns the same `Dynamic_Font`. Its vertical metrics are FreeType's for
// that size, in whole pixels: `default_line_spacing` from one baseline to
// the next, `max_ascender` and `max_descender` the tallest and deepest any
// glyph goes, and `typical_ascender` and `typical_descender` how far the
// Latin capitals and descenders do — the descender negative, below the
// baseline, as FreeType counts it. `character_height` is the pixel size asked
// for.
//
// A glyph's pixels are kept in its page's bitmap, and a page reaches the GPU
// the first time text on it is drawn after a glyph was added, so fonts can be
// opened and text measured before there is a render target.
//

#import "freetype";
#import "Hash_Table";
Unicode :: #import "Unicode";

Font_Effects :: enum_flags u8 {
    NONE :: 0;
}

Glyph_Data :: struct {
    utf32: u32;
    glyph_index: u32;
    page: *Font_Page;

    // Where the glyph is in its page, in pixels from the page's first row.
    x0: s32;
    y0: s32;
    width: s32;
    height: s32;

    // From the pen position on the baseline to the glyph's left edge and up
    // to its top edge.
    offset_x: s32;
    ascent: s32;

    advance: s32;
}

Font_Page :: struct {
    texture: Texture;
    bitmap: Bitmap;
    dirty: bool;
    cursor_x: s32;
    cursor_y: s32;
    row_height: s32;
}

Font_Quad :: struct {
    glyph: *Glyph_Data;
    x: s32;
}

Dynamic_Font :: struct {
    name: string;
    character_height: s32;
    default_line_spacing: s32;
    max_ascender: s32;
    max_descender: s32;
    typical_ascender: s32;
    typical_descender: s32;
    em_width: s32;
    x_advance: s32;
    y_offset_for_my_latin_characters: s32;

    face: FT_Face;
    file_data: [] u8;

    glyphs: Table(u32, *Glyph_Data);
    pages: [..] *Font_Page;
    current_quads: [..] Font_Quad;
    text_width: s64;
}

// Opens `name` in `font_directory` at `pixel_height`, or returns the font
// already opened for that pair. Null, with the reason logged, when the file
// does not open as a font.
get_font_at_size :: (font_directory: string, name: string, pixel_height: int) -> *Dynamic_Font {
    path := ifx font_directory then join(font_directory, "/", name,, temp) else name;
    key := tprint("%:%", path, pixel_height);
    found := find_font(key);
    if found  return found;

    data, ok := read_entire_file(path, log_errors = false);
    if !ok {
        log_error("Simp: could not read the font '%'.\n", path);
        return null;
    }
    return open_font(key, name, cast([] u8) data, pixel_height);
}

// The same, for a font file already in memory, which has to stay where it is
// for as long as the font is used. `name` tells apart two fonts of one size.
get_font_at_size :: (name: string, memory: [] u8, pixel_height: int) -> *Dynamic_Font {
    key := tprint("%:%:%", name, cast(*void) memory.data, pixel_height);
    found := find_font(key);
    if found  return found;
    return open_font(key, name, memory, pixel_height, owns_data = false);
}

get_font_at_size :: (memory: [] u8, pixel_height: int) -> *Dynamic_Font {
    return get_font_at_size("", memory, pixel_height);
}

// Lays `text` out for `draw_prepared_text` and returns how wide it is.
prepare_text :: (font: *Dynamic_Font, text: string, effects: Font_Effects = .NONE) -> width: s64 {
    if !font  return 0;
    array_reset_keeping_memory(*font.current_quads);
    pen: s32;
    previous: u32;
    use_kerning := FT_HAS_KERNING(font.face);
    rest := text;
    while rest.count {
        utf32 := Unicode.utf8_next_character(*rest);
        glyph := find_or_make_glyph(font, utf32);
        if !glyph  continue;
        if use_kerning && previous && glyph.glyph_index {
            kerning: FT_Vector;
            if FT_Get_Kerning(font.face, previous, glyph.glyph_index, xx FT_Kerning_Mode_.DEFAULT, *kerning) == 0 {
                pen += cast(s32) (kerning.x >> 6);
            }
        }
        array_add(*font.current_quads, .{glyph, pen});
        pen += glyph.advance;
        previous = glyph.glyph_index;
    }
    font.text_width = pen;
    return pen;
}

// Draws what `prepare_text` prepared last, from `x, y` on the baseline.
draw_prepared_text :: (font: *Dynamic_Font, x: s64, y: s64, color := Vector4.{1, 1, 1, 1}, effects: Font_Effects = .NONE) {
    if !font || !gl_ready  return;
    state := simp_state();
    left_handed := state.coordinate_system == .LEFT_HANDED;
    for page: font.pages {
        any := false;
        for font.current_quads  if it.glyph.page == page && it.glyph.width  any = true;
        if !any  continue;
        if page.dirty {
            upload(*page.texture, *page.bitmap, swizzle_for_text = true);
            page.dirty = false;
        }
        set_shader_for_text(*page.texture);
        page_width := cast(float) page.bitmap.width;
        page_height := cast(float) page.bitmap.height;
        for font.current_quads {
            glyph := it.glyph;
            if glyph.page != page || !glyph.width  continue;
            x0 := cast(float) (x + it.x + glyph.offset_x);
            x1 := x0 + glyph.width;
            top := cast(float) (ifx left_handed then y - glyph.ascent - font.y_offset_for_my_latin_characters else y + glyph.ascent + font.y_offset_for_my_latin_characters);
            bottom := ifx left_handed then top + glyph.height else top - glyph.height;
            u0 := glyph.x0 / page_width;
            u1 := (glyph.x0 + glyph.width) / page_width;
            v_top := glyph.y0 / page_height;
            v_bottom := (glyph.y0 + glyph.height) / page_height;
            immediate_quad(Vector2.{x0, bottom}, Vector2.{x1, bottom}, Vector2.{x1, top}, Vector2.{x0, top}, color, .{u0, v_bottom}, .{u1, v_bottom}, .{u1, v_top}, .{u0, v_top});
        }
    }
}

draw_text :: (font: *Dynamic_Font, x: s64, y: s64, text: string, color := Vector4.{1, 1, 1, 1}, effects: Font_Effects = .NONE) -> width: s64 {
    width := prepare_text(font, text, effects);
    draw_prepared_text(font, x, y, color, effects);
    return width;
}

get_text_width :: (font: *Dynamic_Font, text: string, effects: Font_Effects = .NONE) -> s64 {
    if !font  return 0;
    saved := font.current_quads;
    font.current_quads = .{};
    width := prepare_text(font, text, effects);
    array_free(font.current_quads);
    font.current_quads = saved;
    return width;
}

#scope_module

ft_library: FT_Library;
fonts: [..] *Dynamic_Font;
font_keys: [..] string;

find_font :: (key: string) -> *Dynamic_Font {
    for font_keys  if it == key  return fonts[it_index];
    return null;
}

open_font :: (key: string, name: string, data: [] u8, pixel_height: int, owns_data := true) -> *Dynamic_Font {
    if !ft_library && FT_Init_FreeType(*ft_library) != 0 {
        log_error("Simp: FreeType did not start.\n");
        return null;
    }
    face: FT_Face;
    if FT_New_Memory_Face(ft_library, data.data, xx data.count, 0, *face) != 0 {
        log_error("Simp: '%' is not a font FreeType can open.\n", name);
        if owns_data  array_free(data);
        return null;
    }
    height := cast(s32) max(pixel_height, 1);
    FT_Set_Pixel_Sizes(face, 0, xx height);

    font := New(Dynamic_Font);
    font.name = copy_string(name);
    font.face = face;
    if owns_data  font.file_data = data;
    font.character_height = height;
    metrics := *face.size.metrics;
    font.default_line_spacing = cast(s32) ((metrics.height + 63) >> 6);
    font.max_ascender = cast(s32) ((metrics.ascender + 63) >> 6);
    font.max_descender = cast(s32) (metrics.descender >> 6);
    font.typical_ascender = font.max_ascender;
    font.typical_descender = font.max_descender;
    if FT_Load_Char(face, #char "M", FT_LOAD_DEFAULT) == 0 {
        font.typical_ascender = cast(s32) (face.glyph.metrics.horiBearingY >> 6);
        font.em_width = cast(s32) (face.glyph.metrics.width >> 6);
        font.x_advance = cast(s32) (face.glyph.advance.x >> 6);
    }
    if FT_Load_Char(face, #char "g", FT_LOAD_DEFAULT) == 0 {
        font.typical_descender = cast(s32) ((face.glyph.metrics.horiBearingY - face.glyph.metrics.height) >> 6);
    }
    array_add(*fonts, font);
    array_add(*font_keys, copy_string(key));
    return font;
}

find_or_make_glyph :: (font: *Dynamic_Font, utf32: u32) -> *Glyph_Data {
    found, glyph := table_find(*font.glyphs, utf32);
    if found  return glyph;

    face := font.face;
    index := FT_Get_Char_Index(face, utf32);
    if FT_Load_Glyph(face, index, FT_LOAD_RENDER) != 0  return null;
    slot := face.glyph;
    source := *slot.bitmap;

    glyph = New(Glyph_Data);
    glyph.utf32 = utf32;
    glyph.glyph_index = index;
    glyph.width = xx source.width;
    glyph.height = xx source.rows;
    glyph.offset_x = slot.bitmap_left;
    glyph.ascent = slot.bitmap_top;
    glyph.advance = cast(s32) (slot.advance.x >> 6);

    if glyph.width && glyph.height {
        page, x, y := find_room(font, glyph.width, glyph.height);
        glyph.page = page;
        glyph.x0 = x;
        glyph.y0 = y;
        for row: 0..glyph.height - 1 {
            from := source.buffer + row * source.pitch;
            to := page.bitmap.data.data + (y + row) * page.bitmap.stride + x;
            if source.pixel_mode == xx FT_Pixel_Mode_.MONO {
                for column: 0..glyph.width - 1 {
                    bit := (from[column / 8] >> (7 - column % 8)) & 1;
                    to[column] = xx ifx bit then 255 else 0;
                }
            } else {
                memcpy(to, from, glyph.width);
            }
        }
        page.dirty = true;
    }
    table_add(*font.glyphs, utf32, glyph);
    return glyph;
}

// A place for a `width` by `height` glyph: along the current row of the last
// page, on a new row under it, or on a new page.
find_room :: (font: *Dynamic_Font, width: s32, height: s32) -> *Font_Page, s32, s32 {
    PADDING :: 1;
    page: *Font_Page;
    if font.pages.count  page = font.pages[font.pages.count - 1];
    if page && page.cursor_x + width + PADDING > page.bitmap.width {
        page.cursor_x = 0;
        page.cursor_y += page.row_height + PADDING;
        page.row_height = 0;
    }
    if !page || page.cursor_y + height + PADDING > page.bitmap.height {
        size: s32 = 256;
        while size < font.character_height * 12 && size < 4096  size *= 2;
        while size < width + PADDING || size < height + PADDING  size *= 2;
        page = New(Font_Page);
        bitmap_alloc(*page.bitmap, size, size, .R8);
        array_add(*font.pages, page);
    }
    x := page.cursor_x;
    y := page.cursor_y;
    page.cursor_x += width + PADDING;
    page.row_height = max(page.row_height, height);
    return page, x, y;
}
