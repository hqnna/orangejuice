//
// Bitmaps — pixels in memory — and the textures made from them.
//
// A `Bitmap` is `height` rows of `stride` bytes, the first row first. An
// image file decoded while the coordinate system is right-handed is stored
// bottom row first, which is the row OpenGL puts at texture coordinate
// v = 0, so the image comes out upright either way.
//
// `texture_load_from_file` and `texture_load_from_memory` decode PNG, JPEG,
// BMP, TGA, GIF, PSD, HDR, PIC and PNM through `stb_image`. A one-channel
// image is grey, two channels are grey and alpha; both are sampled as RGBA.
//

#import "stb_image";
#import "File";

Texture_Format :: enum u16 {
    UNINITIALIZED :: 0;
    R8;
    RG8;
    RGB8;
    RGBA8;
}

Bitmap_Flags :: enum_flags u32 {
    FREE_DATA_ON_DEINIT;
}

Bitmap :: struct {
    width: s32;
    height: s32;
    stride: s32;
    format: Texture_Format;
    data: [] u8;
    flags: Bitmap_Flags;
}

Texture :: struct {
    gl_handle: GLuint;
    width: s32;
    height: s32;
    format: Texture_Format;
    srgb: bool;
    build_mipmaps: bool;

    // What a texture loaded before OpenGL started keeps until it is drawn.
    pending: Bitmap;
}

bytes_per_pixel :: (format: Texture_Format) -> s32 {
    if format == {
      case .R8;    return 1;
      case .RG8;   return 2;
      case .RGB8;  return 3;
      case .RGBA8; return 4;
    }
    return 0;
}

// Makes `bitmap` a `width` by `height` bitmap of zeros, in `context.allocator`.
bitmap_alloc :: (bitmap: *Bitmap, width: s32, height: s32, format: Texture_Format) {
    bitmap.width = width;
    bitmap.height = height;
    bitmap.format = format;
    bitmap.stride = width * bytes_per_pixel(format);
    bitmap.data = NewArray(bitmap.stride * height, u8);
    bitmap.flags = .FREE_DATA_ON_DEINIT;
}

deinit :: (bitmap: *Bitmap) {
    if bitmap.flags & .FREE_DATA_ON_DEINIT  array_free(bitmap.data);
    bitmap.* = .{};
}

bitmap_load :: (bitmap: *Bitmap, path: string) -> bool {
    data, ok := read_entire_file(path, log_errors = false);
    if !ok {
        log_error("Simp: could not read the image '%'.\n", path);
        return false;
    }
    defer free(data);
    if !bitmap_load_from_memory(bitmap, cast([] u8) data) {
        log_error("Simp: could not decode the image '%'.\n", path);
        return false;
    }
    return true;
}

bitmap_load_from_memory :: (bitmap: *Bitmap, memory: [] u8) -> bool {
    width, height, channels: s32;
    pixels := stbi_load_from_memory(memory.data, xx memory.count, *width, *height, *channels, 0);
    if !pixels  return false;
    defer stbi_image_free(pixels);

    format: Texture_Format;
    if channels == {
      case 1; format = .R8;
      case 2; format = .RG8;
      case 3; format = .RGB8;
      case 4; format = .RGBA8;
      case;   return false;
    }
    bitmap_alloc(bitmap, width, height, format);
    flip := simp_state().coordinate_system == .RIGHT_HANDED;
    for row: 0..height - 1 {
        from := ifx flip then height - 1 - row else row;
        memcpy(bitmap.data.data + row * bitmap.stride, pixels + from * bitmap.stride, bitmap.stride);
    }
    return true;
}

texture_load_from_file :: (texture: *Texture, path: string, srgb := false, build_mipmaps := false) -> bool {
    bitmap: Bitmap;
    if !bitmap_load(*bitmap, path)  return false;
    defer deinit(*bitmap);
    return texture_load_from_bitmap(texture, *bitmap, srgb, build_mipmaps);
}

texture_load_from_memory :: (texture: *Texture, memory: [] u8, srgb := false, build_mipmaps := false) -> bool {
    bitmap: Bitmap;
    if !bitmap_load_from_memory(*bitmap, memory) {
        log_error("Simp: could not decode an image of % bytes.\n", memory.count);
        return false;
    }
    defer deinit(*bitmap);
    return texture_load_from_bitmap(texture, *bitmap, srgb, build_mipmaps);
}

// Uploads `bitmap` as the texture's pixels, replacing what it had. Without
// OpenGL yet, the texture keeps a copy and uploads it when first drawn.
texture_load_from_bitmap :: (texture: *Texture, bitmap: *Bitmap, srgb := false, build_mipmaps := false) -> bool {
    if !bytes_per_pixel(bitmap.format) || bitmap.width <= 0 || bitmap.height <= 0  return false;
    texture.width = bitmap.width;
    texture.height = bitmap.height;
    texture.format = bitmap.format;
    texture.srgb = srgb;
    texture.build_mipmaps = build_mipmaps;
    if !gl_ready {
        deinit(*texture.pending);
        texture.pending = bitmap.*;
        texture.pending.data = array_copy(bitmap.data);
        texture.pending.flags = .FREE_DATA_ON_DEINIT;
        return true;
    }
    upload(texture, bitmap);
    return true;
}

// Replaces the pixels of a rectangle of the texture, given in texture rows
// from the first one uploaded.
texture_update :: (texture: *Texture, bitmap: *Bitmap, x: s32 = 0, y: s32 = 0) {
    if !gl_ready || !texture.gl_handle  return;
    texture_make_resident(texture);
    glBindTexture(GL_TEXTURE_2D, texture.gl_handle);
    glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
    glPixelStorei(GL_UNPACK_ROW_LENGTH, bitmap.stride / bytes_per_pixel(bitmap.format));
    _, format := gl_formats(texture.format, texture.srgb);
    glTexSubImage2D(GL_TEXTURE_2D, 0, x, y, xx bitmap.width, xx bitmap.height, format, GL_UNSIGNED_BYTE, bitmap.data.data);
    glPixelStorei(GL_UNPACK_ROW_LENGTH, 0);
}

deinit :: (texture: *Texture) {
    deinit(*texture.pending);
    if texture.gl_handle && gl_ready  glDeleteTextures(1, *texture.gl_handle);
    texture.* = .{};
}

#scope_module

gl_formats :: (format: Texture_Format, srgb: bool) -> internal: GLint, format: GLenum {
    if format == {
      case .R8;    return GL_R8, GL_RED;
      case .RG8;   return GL_RG8, GL_RG;
      case .RGB8;  return xx ifx srgb then GL_SRGB8 else GL_RGB8, GL_RGB;
    }
    return xx ifx srgb then GL_SRGB8_ALPHA8 else GL_RGBA8, GL_RGBA;
}

upload :: (texture: *Texture, bitmap: *Bitmap, swizzle_for_text := false) {
    if !texture.gl_handle  glGenTextures(1, *texture.gl_handle);
    glBindTexture(GL_TEXTURE_2D, texture.gl_handle);
    glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
    glPixelStorei(GL_UNPACK_ROW_LENGTH, bitmap.stride / bytes_per_pixel(bitmap.format));
    internal, format := gl_formats(bitmap.format, texture.srgb);
    glTexImage2D(GL_TEXTURE_2D, 0, internal, xx bitmap.width, xx bitmap.height, 0, format, GL_UNSIGNED_BYTE, bitmap.data.data);
    glPixelStorei(GL_UNPACK_ROW_LENGTH, 0);

    swizzle: [4] GLint;
    if swizzle_for_text {
        swizzle = .[GL_ONE, GL_ONE, GL_ONE, GL_RED];
    } else if bitmap.format == .R8 {
        swizzle = .[GL_RED, GL_RED, GL_RED, GL_ONE];
    } else if bitmap.format == .RG8 {
        swizzle = .[GL_RED, GL_RED, GL_RED, GL_GREEN];
    } else if bitmap.format == .RGB8 {
        swizzle = .[GL_RED, GL_GREEN, GL_BLUE, GL_ONE];
    } else {
        swizzle = .[GL_RED, GL_GREEN, GL_BLUE, GL_ALPHA];
    }
    glTexParameteriv(GL_TEXTURE_2D, GL_TEXTURE_SWIZZLE_RGBA, swizzle.data);

    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    if texture.build_mipmaps {
        glGenerateMipmap(GL_TEXTURE_2D);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
    } else {
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    }
}

// Uploads what a texture loaded before OpenGL started has been keeping.
texture_make_resident :: (texture: *Texture) {
    if texture.gl_handle || !texture.pending.data.count  return;
    upload(texture, *texture.pending);
    deinit(*texture.pending);
}
