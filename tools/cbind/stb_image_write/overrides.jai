//
// `stbi_write_func` is a C *function* type, which the header passes as a
// pointer; in Jai a procedure type is already the pointer, so the callback
// variants take one directly. `context` is a Jai keyword, so the header's
// parameter of that name is `_context` here.
//

stbi_write_func :: #type (_context: *void, data: *void, size: s32) #c_call;

stbi_write_bmp_to_func :: (func: stbi_write_func, _context: *void, w: s32, h: s32, comp: s32, data: *void) -> s32 #foreign libstb_image_write;
stbi_write_hdr_to_func :: (func: stbi_write_func, _context: *void, w: s32, h: s32, comp: s32, data: *float32) -> s32 #foreign libstb_image_write;
stbi_write_jpg_to_func :: (func: stbi_write_func, _context: *void, x: s32, y: s32, comp: s32, data: *void, quality: s32) -> s32 #foreign libstb_image_write;
stbi_write_png_to_func :: (func: stbi_write_func, _context: *void, w: s32, h: s32, comp: s32, data: *void, stride_in_bytes: s32) -> s32 #foreign libstb_image_write;
stbi_write_tga_to_func :: (func: stbi_write_func, _context: *void, w: s32, h: s32, comp: s32, data: *void) -> s32 #foreign libstb_image_write;
