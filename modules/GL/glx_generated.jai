//
// The GLX surface: what `<GL/glx.h>` and `<GL/glxext.h>` declare, which is
// how an OpenGL context is made on an X display and where `gl_load` gets its
// entry points from on Linux.
//
// Generated from the system's own headers by `tools/cbind` — see
// `tools/cbind/README.md`. The X types it names come from `X11`, which
// `GL/module.jai` imports; `libgl` is declared there too.
//


// -------------------------------------------------------------- constants ---

GLX_3DFX_FULLSCREEN_MODE_MESA :: 2;
GLX_3DFX_WINDOW_MODE_MESA :: 1;
GLX_3DFX_multisample :: 1;
GLX_ACCUM_ALPHA_SIZE :: 17;
GLX_ACCUM_BLUE_SIZE :: 16;
GLX_ACCUM_BUFFER_BIT :: 128;
GLX_ACCUM_BUFFER_BIT_SGIX :: 128;
GLX_ACCUM_GREEN_SIZE :: 15;
GLX_ACCUM_RED_SIZE :: 14;
GLX_ALPHA_SIZE :: 11;
GLX_AMD_gpu_association :: 1;
GLX_ARB_context_flush_control :: 1;
GLX_ARB_create_context :: 1;
GLX_ARB_create_context_no_error :: 1;
GLX_ARB_create_context_profile :: 1;
GLX_ARB_create_context_robustness :: 1;
GLX_ARB_fbconfig_float :: 1;
GLX_ARB_framebuffer_sRGB :: 1;
GLX_ARB_get_proc_address :: 1;
GLX_ARB_multisample :: 1;
GLX_ARB_render_texture :: 1;
GLX_ARB_robustness_application_isolation :: 1;
GLX_ARB_robustness_share_group_isolation :: 1;
GLX_ARB_vertex_buffer_object :: 1;
GLX_AUX0_EXT :: 8418;
GLX_AUX1_EXT :: 8419;
GLX_AUX2_EXT :: 8420;
GLX_AUX3_EXT :: 8421;
GLX_AUX4_EXT :: 8422;
GLX_AUX5_EXT :: 8423;
GLX_AUX6_EXT :: 8424;
GLX_AUX7_EXT :: 8425;
GLX_AUX8_EXT :: 8426;
GLX_AUX9_EXT :: 8427;
GLX_AUX_BUFFERS :: 7;
GLX_AUX_BUFFERS_BIT :: 16;
GLX_AUX_BUFFERS_BIT_SGIX :: 16;
GLX_BACK_BUFFER_AGE_EXT :: 8436;
GLX_BACK_EXT :: 8416;
GLX_BACK_LEFT_BUFFER_BIT :: 4;
GLX_BACK_LEFT_BUFFER_BIT_SGIX :: 4;
GLX_BACK_LEFT_EXT :: 8416;
GLX_BACK_RIGHT_BUFFER_BIT :: 8;
GLX_BACK_RIGHT_BUFFER_BIT_SGIX :: 8;
GLX_BACK_RIGHT_EXT :: 8417;
GLX_BAD_ATTRIBUTE :: 2;
GLX_BAD_CONTEXT :: 5;
GLX_BAD_ENUM :: 7;
GLX_BAD_HYPERPIPE_CONFIG_SGIX :: 91;
GLX_BAD_HYPERPIPE_SGIX :: 92;
GLX_BAD_SCREEN :: 1;
GLX_BAD_VALUE :: 6;
GLX_BAD_VISUAL :: 4;
GLX_BIND_TO_MIPMAP_TEXTURE_EXT :: 8402;
GLX_BIND_TO_TEXTURE_RGBA_EXT :: 8401;
GLX_BIND_TO_TEXTURE_RGB_EXT :: 8400;
GLX_BIND_TO_TEXTURE_TARGETS_EXT :: 8403;
GLX_BLENDED_RGBA_SGIS :: 32805;
GLX_BLUE_SIZE :: 10;
GLX_BUFFER_CLOBBER_MASK_SGIX :: 134217728;
GLX_BUFFER_SIZE :: 2;
GLX_BUFFER_SWAP_COMPLETE_INTEL_MASK :: 67108864;
GLX_BufferSwapComplete :: 1;
GLX_COLOR_INDEX_BIT :: 2;
GLX_COLOR_INDEX_BIT_SGIX :: 2;
GLX_COLOR_INDEX_TYPE :: 32789;
GLX_COLOR_INDEX_TYPE_SGIX :: 32789;
GLX_COLOR_SAMPLES_NV :: 8371;
GLX_CONFIG_CAVEAT :: 32;
GLX_CONTEXT_ALLOW_BUFFER_BYTE_ORDER_MISMATCH_ARB :: 8341;
GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB :: 2;
GLX_CONTEXT_CORE_PROFILE_BIT_ARB :: 1;
GLX_CONTEXT_DEBUG_BIT_ARB :: 1;
GLX_CONTEXT_ES2_PROFILE_BIT_EXT :: 4;
GLX_CONTEXT_ES_PROFILE_BIT_EXT :: 4;
GLX_CONTEXT_FLAGS_ARB :: 8340;
GLX_CONTEXT_FORWARD_COMPATIBLE_BIT_ARB :: 2;
GLX_CONTEXT_MAJOR_VERSION_ARB :: 8337;
GLX_CONTEXT_MINOR_VERSION_ARB :: 8338;
GLX_CONTEXT_MULTIGPU_ATTRIB_AFR_NV :: 8364;
GLX_CONTEXT_MULTIGPU_ATTRIB_MULTICAST_NV :: 8365;
GLX_CONTEXT_MULTIGPU_ATTRIB_MULTI_DISPLAY_MULTICAST_NV :: 8366;
GLX_CONTEXT_MULTIGPU_ATTRIB_NV :: 8362;
GLX_CONTEXT_MULTIGPU_ATTRIB_SINGLE_NV :: 8363;
GLX_CONTEXT_OPENGL_NO_ERROR_ARB :: 12723;
GLX_CONTEXT_PRIORITY_HIGH_EXT :: 12545;
GLX_CONTEXT_PRIORITY_LEVEL_EXT :: 12544;
GLX_CONTEXT_PRIORITY_LOW_EXT :: 12547;
GLX_CONTEXT_PRIORITY_MEDIUM_EXT :: 12546;
GLX_CONTEXT_PROFILE_MASK_ARB :: 37158;
GLX_CONTEXT_RELEASE_BEHAVIOR_ARB :: 8343;
GLX_CONTEXT_RELEASE_BEHAVIOR_FLUSH_ARB :: 8344;
GLX_CONTEXT_RELEASE_BEHAVIOR_NONE_ARB :: 0;
GLX_CONTEXT_RESET_ISOLATION_BIT_ARB :: 8;
GLX_CONTEXT_RESET_NOTIFICATION_STRATEGY_ARB :: 33366;
GLX_CONTEXT_ROBUST_ACCESS_BIT_ARB :: 4;
GLX_COPY_COMPLETE_INTEL :: 33153;
GLX_COVERAGE_SAMPLES_NV :: 100001;
GLX_DAMAGED :: 32800;
GLX_DAMAGED_SGIX :: 32800;
GLX_DEPTH_BUFFER_BIT :: 32;
GLX_DEPTH_BUFFER_BIT_SGIX :: 32;
GLX_DEPTH_SIZE :: 12;
GLX_DEVICE_ID_NV :: 8397;
GLX_DIRECT_COLOR :: 32771;
GLX_DIRECT_COLOR_EXT :: 32771;
GLX_DONT_CARE :: 4294967295;
GLX_DOUBLEBUFFER :: 5;
GLX_DRAWABLE_TYPE :: 32784;
GLX_DRAWABLE_TYPE_SGIX :: 32784;
GLX_EVENT_MASK :: 32799;
GLX_EVENT_MASK_SGIX :: 32799;
GLX_EXCHANGE_COMPLETE_INTEL :: 33152;
GLX_EXTENSIONS :: 3;
GLX_EXT_buffer_age :: 1;
GLX_EXT_context_priority :: 1;
GLX_EXT_create_context_es2_profile :: 1;
GLX_EXT_create_context_es_profile :: 1;
GLX_EXT_fbconfig_packed_float :: 1;
GLX_EXT_framebuffer_sRGB :: 1;
GLX_EXT_get_drawable_type :: 1;
GLX_EXT_import_context :: 1;
GLX_EXT_libglvnd :: 1;
GLX_EXT_no_config_context :: 1;
GLX_EXT_stereo_tree :: 1;
GLX_EXT_swap_control :: 1;
GLX_EXT_swap_control_tear :: 1;
GLX_EXT_texture_from_pixmap :: 1;
GLX_EXT_visual_info :: 1;
GLX_EXT_visual_rating :: 1;
GLX_FBCONFIG_ID :: 32787;
GLX_FBCONFIG_ID_SGIX :: 32787;
GLX_FLIP_COMPLETE_INTEL :: 33154;
GLX_FLOAT_COMPONENTS_NV :: 8368;
GLX_FRAMEBUFFER_SRGB_CAPABLE_ARB :: 8370;
GLX_FRAMEBUFFER_SRGB_CAPABLE_EXT :: 8370;
GLX_FRONT_EXT :: 8414;
GLX_FRONT_LEFT_BUFFER_BIT :: 1;
GLX_FRONT_LEFT_BUFFER_BIT_SGIX :: 1;
GLX_FRONT_LEFT_EXT :: 8414;
GLX_FRONT_RIGHT_BUFFER_BIT :: 2;
GLX_FRONT_RIGHT_BUFFER_BIT_SGIX :: 2;
GLX_FRONT_RIGHT_EXT :: 8415;
GLX_GENERATE_RESET_ON_VIDEO_MEMORY_PURGE_NV :: 8439;
GLX_GLXEXT_VERSION :: 20220530;
GLX_GPU_CLOCK_AMD :: 8612;
GLX_GPU_FASTEST_TARGET_GPUS_AMD :: 8610;
GLX_GPU_NUM_PIPES_AMD :: 8613;
GLX_GPU_NUM_RB_AMD :: 8615;
GLX_GPU_NUM_SIMD_AMD :: 8614;
GLX_GPU_NUM_SPI_AMD :: 8616;
GLX_GPU_OPENGL_VERSION_STRING_AMD :: 7938;
GLX_GPU_RAM_AMD :: 8611;
GLX_GPU_RENDERER_STRING_AMD :: 7937;
GLX_GPU_VENDOR_AMD :: 7936;
GLX_GRAY_SCALE :: 32774;
GLX_GRAY_SCALE_EXT :: 32774;
GLX_GREEN_SIZE :: 9;
GLX_HEIGHT :: 32798;
GLX_HEIGHT_SGIX :: 32798;
GLX_HYPERPIPE_DISPLAY_PIPE_SGIX :: 1;
GLX_HYPERPIPE_ID_SGIX :: 32816;
GLX_HYPERPIPE_PIPE_NAME_LENGTH_SGIX :: 80;
GLX_HYPERPIPE_PIXEL_AVERAGE_SGIX :: 4;
GLX_HYPERPIPE_RENDER_PIPE_SGIX :: 2;
GLX_HYPERPIPE_STEREO_SGIX :: 3;
GLX_INTEL_swap_event :: 1;
GLX_LARGEST_PBUFFER :: 32796;
GLX_LARGEST_PBUFFER_SGIX :: 32796;
GLX_LATE_SWAPS_TEAR_EXT :: 8435;
GLX_LEVEL :: 3;
GLX_LOSE_CONTEXT_ON_RESET_ARB :: 33362;
GLX_MAX_PBUFFER_HEIGHT :: 32791;
GLX_MAX_PBUFFER_HEIGHT_SGIX :: 32791;
GLX_MAX_PBUFFER_PIXELS :: 32792;
GLX_MAX_PBUFFER_PIXELS_SGIX :: 32792;
GLX_MAX_PBUFFER_WIDTH :: 32790;
GLX_MAX_PBUFFER_WIDTH_SGIX :: 32790;
GLX_MAX_SWAP_INTERVAL_EXT :: 8434;
GLX_MESA_agp_offset :: 1;
GLX_MESA_copy_sub_buffer :: 1;
GLX_MESA_pixmap_colormap :: 1;
GLX_MESA_query_renderer :: 1;
GLX_MESA_release_buffers :: 1;
GLX_MESA_set_3dfx_mode :: 1;
GLX_MESA_swap_control :: 1;
GLX_MESA_swap_frame_usage :: 1;
GLX_MIPMAP_TEXTURE_EXT :: 8407;
GLX_MULTISAMPLE_SUB_RECT_HEIGHT_SGIS :: 32807;
GLX_MULTISAMPLE_SUB_RECT_WIDTH_SGIS :: 32806;
GLX_NONE :: 32768;
GLX_NONE_EXT :: 32768;
GLX_NON_CONFORMANT_CONFIG :: 32781;
GLX_NON_CONFORMANT_VISUAL_EXT :: 32781;
GLX_NO_EXTENSION :: 3;
GLX_NO_RESET_NOTIFICATION_ARB :: 33377;
GLX_NUM_VIDEO_CAPTURE_SLOTS_NV :: 8399;
GLX_NUM_VIDEO_SLOTS_NV :: 8432;
GLX_NV_copy_buffer :: 1;
GLX_NV_copy_image :: 1;
GLX_NV_delay_before_swap :: 1;
GLX_NV_float_buffer :: 1;
GLX_NV_multigpu_context :: 1;
GLX_NV_multisample_coverage :: 1;
GLX_NV_present_video :: 1;
GLX_NV_robustness_video_memory_purge :: 1;
GLX_NV_swap_group :: 1;
GLX_NV_video_capture :: 1;
GLX_NV_video_out :: 1;
GLX_OML_swap_method :: 1;
GLX_OML_sync_control :: 1;
GLX_OPTIMAL_PBUFFER_HEIGHT_SGIX :: 32794;
GLX_OPTIMAL_PBUFFER_WIDTH_SGIX :: 32793;
GLX_PBUFFER :: 32803;
GLX_PBUFFER_BIT :: 4;
GLX_PBUFFER_BIT_SGIX :: 4;
GLX_PBUFFER_CLOBBER_MASK :: 134217728;
GLX_PBUFFER_HEIGHT :: 32832;
GLX_PBUFFER_SGIX :: 32803;
GLX_PBUFFER_WIDTH :: 32833;
GLX_PIPE_RECT_LIMITS_SGIX :: 2;
GLX_PIPE_RECT_SGIX :: 1;
GLX_PIXMAP_BIT :: 2;
GLX_PIXMAP_BIT_SGIX :: 2;
GLX_PRESERVED_CONTENTS :: 32795;
GLX_PRESERVED_CONTENTS_SGIX :: 32795;
GLX_PSEUDO_COLOR :: 32772;
GLX_PSEUDO_COLOR_EXT :: 32772;
GLX_PbufferClobber :: 0;
GLX_RED_SIZE :: 8;
GLX_RENDERER_ACCELERATED_MESA :: 33158;
GLX_RENDERER_DEVICE_ID_MESA :: 33156;
GLX_RENDERER_OPENGL_COMPATIBILITY_PROFILE_VERSION_MESA :: 33163;
GLX_RENDERER_OPENGL_CORE_PROFILE_VERSION_MESA :: 33162;
GLX_RENDERER_OPENGL_ES2_PROFILE_VERSION_MESA :: 33165;
GLX_RENDERER_OPENGL_ES_PROFILE_VERSION_MESA :: 33164;
GLX_RENDERER_PREFERRED_PROFILE_MESA :: 33161;
GLX_RENDERER_UNIFIED_MEMORY_ARCHITECTURE_MESA :: 33160;
GLX_RENDERER_VENDOR_ID_MESA :: 33155;
GLX_RENDERER_VERSION_MESA :: 33157;
GLX_RENDERER_VIDEO_MEMORY_MESA :: 33159;
GLX_RENDER_TYPE :: 32785;
GLX_RENDER_TYPE_SGIX :: 32785;
GLX_RGBA :: 4;
GLX_RGBA_BIT :: 1;
GLX_RGBA_BIT_SGIX :: 1;
GLX_RGBA_FLOAT_BIT_ARB :: 4;
GLX_RGBA_FLOAT_TYPE_ARB :: 8377;
GLX_RGBA_TYPE :: 32788;
GLX_RGBA_TYPE_SGIX :: 32788;
GLX_RGBA_UNSIGNED_FLOAT_BIT_EXT :: 8;
GLX_RGBA_UNSIGNED_FLOAT_TYPE_EXT :: 8369;
GLX_SAMPLES :: 100001;
GLX_SAMPLES_3DFX :: 32849;
GLX_SAMPLES_ARB :: 100001;
GLX_SAMPLES_SGIS :: 100001;
GLX_SAMPLE_BUFFERS :: 100000;
GLX_SAMPLE_BUFFERS_3DFX :: 32848;
GLX_SAMPLE_BUFFERS_ARB :: 100000;
GLX_SAMPLE_BUFFERS_BIT_SGIX :: 256;
GLX_SAMPLE_BUFFERS_SGIS :: 100000;
GLX_SAVED :: 32801;
GLX_SAVED_SGIX :: 32801;
GLX_SCREEN :: 32780;
GLX_SCREEN_EXT :: 32780;
GLX_SGIS_blended_overlay :: 1;
GLX_SGIS_multisample :: 1;
GLX_SGIS_shared_multisample :: 1;
GLX_SGIX_dmbuffer :: 1;
GLX_SGIX_fbconfig :: 1;
GLX_SGIX_hyperpipe :: 1;
GLX_SGIX_pbuffer :: 1;
GLX_SGIX_swap_barrier :: 1;
GLX_SGIX_swap_group :: 1;
GLX_SGIX_video_resize :: 1;
GLX_SGIX_video_source :: 1;
GLX_SGIX_visual_select_group :: 1;
GLX_SGI_cushion :: 1;
GLX_SGI_make_current_read :: 1;
GLX_SGI_swap_control :: 1;
GLX_SGI_video_sync :: 1;
GLX_SHARE_CONTEXT_EXT :: 32778;
GLX_SLOW_CONFIG :: 32769;
GLX_SLOW_VISUAL_EXT :: 32769;
GLX_STATIC_COLOR :: 32773;
GLX_STATIC_COLOR_EXT :: 32773;
GLX_STATIC_GRAY :: 32775;
GLX_STATIC_GRAY_EXT :: 32775;
GLX_STENCIL_BUFFER_BIT :: 64;
GLX_STENCIL_BUFFER_BIT_SGIX :: 64;
GLX_STENCIL_SIZE :: 13;
GLX_STEREO :: 6;
GLX_STEREO_NOTIFY_EXT :: 0;
GLX_STEREO_NOTIFY_MASK_EXT :: 1;
GLX_STEREO_TREE_EXT :: 8437;
GLX_SUN_get_transparent_index :: 1;
GLX_SWAP_COPY_OML :: 32866;
GLX_SWAP_EXCHANGE_OML :: 32865;
GLX_SWAP_INTERVAL_EXT :: 8433;
GLX_SWAP_METHOD_OML :: 32864;
GLX_SWAP_UNDEFINED_OML :: 32867;
GLX_SYNC_FRAME_SGIX :: 0;
GLX_SYNC_SWAP_SGIX :: 1;
GLX_TEXTURE_1D_BIT_EXT :: 1;
GLX_TEXTURE_1D_EXT :: 8411;
GLX_TEXTURE_2D_BIT_EXT :: 2;
GLX_TEXTURE_2D_EXT :: 8412;
GLX_TEXTURE_FORMAT_EXT :: 8405;
GLX_TEXTURE_FORMAT_NONE_EXT :: 8408;
GLX_TEXTURE_FORMAT_RGBA_EXT :: 8410;
GLX_TEXTURE_FORMAT_RGB_EXT :: 8409;
GLX_TEXTURE_RECTANGLE_BIT_EXT :: 4;
GLX_TEXTURE_RECTANGLE_EXT :: 8413;
GLX_TEXTURE_TARGET_EXT :: 8406;
GLX_TRANSPARENT_ALPHA_VALUE :: 40;
GLX_TRANSPARENT_ALPHA_VALUE_EXT :: 40;
GLX_TRANSPARENT_BLUE_VALUE :: 39;
GLX_TRANSPARENT_BLUE_VALUE_EXT :: 39;
GLX_TRANSPARENT_GREEN_VALUE :: 38;
GLX_TRANSPARENT_GREEN_VALUE_EXT :: 38;
GLX_TRANSPARENT_INDEX :: 32777;
GLX_TRANSPARENT_INDEX_EXT :: 32777;
GLX_TRANSPARENT_INDEX_VALUE :: 36;
GLX_TRANSPARENT_INDEX_VALUE_EXT :: 36;
GLX_TRANSPARENT_RED_VALUE :: 37;
GLX_TRANSPARENT_RED_VALUE_EXT :: 37;
GLX_TRANSPARENT_RGB :: 32776;
GLX_TRANSPARENT_RGB_EXT :: 32776;
GLX_TRANSPARENT_TYPE :: 35;
GLX_TRANSPARENT_TYPE_EXT :: 35;
GLX_TRUE_COLOR :: 32770;
GLX_TRUE_COLOR_EXT :: 32770;
GLX_UNIQUE_ID_NV :: 8398;
GLX_USE_GL :: 1;
GLX_VENDOR :: 1;
GLX_VENDOR_NAMES_EXT :: 8438;
GLX_VERSION :: 2;
GLX_VERSION_1_1 :: 1;
GLX_VERSION_1_2 :: 1;
GLX_VERSION_1_3 :: 1;
GLX_VERSION_1_4 :: 1;
GLX_VIDEO_OUT_ALPHA_NV :: 8388;
GLX_VIDEO_OUT_COLOR_AND_ALPHA_NV :: 8390;
GLX_VIDEO_OUT_COLOR_AND_DEPTH_NV :: 8391;
GLX_VIDEO_OUT_COLOR_NV :: 8387;
GLX_VIDEO_OUT_DEPTH_NV :: 8389;
GLX_VIDEO_OUT_FIELD_1_NV :: 8393;
GLX_VIDEO_OUT_FIELD_2_NV :: 8394;
GLX_VIDEO_OUT_FRAME_NV :: 8392;
GLX_VIDEO_OUT_STACKED_FIELDS_1_2_NV :: 8395;
GLX_VIDEO_OUT_STACKED_FIELDS_2_1_NV :: 8396;
GLX_VISUAL_CAVEAT_EXT :: 32;
GLX_VISUAL_ID :: 32779;
GLX_VISUAL_ID_EXT :: 32779;
GLX_VISUAL_SELECT_GROUP_SGIX :: 32808;
GLX_WIDTH :: 32797;
GLX_WIDTH_SGIX :: 32797;
GLX_WINDOW :: 32802;
GLX_WINDOW_BIT :: 1;
GLX_WINDOW_BIT_SGIX :: 1;
GLX_WINDOW_SGIX :: 32802;
GLX_X_RENDERABLE :: 32786;
GLX_X_RENDERABLE_SGIX :: 32786;
GLX_X_VISUAL_TYPE :: 34;
GLX_X_VISUAL_TYPE_EXT :: 34;
GLX_Y_INVERTED_EXT :: 8404;

// -------------------------------------------------------------- the types ---

GLXContext :: *__GLXcontextRec;
GLXPixmap :: XID;
GLXDrawable :: XID;
GLXFBConfig :: *__GLXFBConfigRec;
GLXFBConfigID :: XID;
GLXContextID :: XID;
GLXWindow :: XID;
GLXPbuffer :: XID;
__GLXextFuncPtr :: #type () -> void #c_call;
GLXStereoNotifyEventEXT :: struct {
    type: s32;
    serial: u64;
    send_event: s32;
    display: *Display;
    extension: s32;
    evtype: s32;
    window: GLXDrawable;
    stereo_tree: s32;
}

GLXVideoCaptureDeviceNV :: XID;
GLXVideoDeviceNV :: u32;
GLXPbufferSGIX :: XID;
GLXFBConfigSGIX :: *__GLXFBConfigRec;
GLXHyperpipeNetworkSGIX :: struct {
    pipeName: [80] u8;
    networkId: s32;
}

GLXHyperpipeConfigSGIX :: struct {
    pipeName: [80] u8;
    channel: s32;
    participationType: u32;
    timeSlice: s32;
}

GLXPipeRect :: struct {
    pipeName: [80] u8;
    srcXOrigin: s32;
    srcYOrigin: s32;
    srcWidth: s32;
    srcHeight: s32;
    destXOrigin: s32;
    destYOrigin: s32;
    destWidth: s32;
    destHeight: s32;
}

GLXPipeRectLimits :: struct {
    pipeName: [80] u8;
    XOrigin: s32;
    YOrigin: s32;
    maxHeight: s32;
    maxWidth: s32;
}

GLXVideoSourceSGIX :: XID;
GLXPbufferClobberEvent :: struct {
    event_type: s32;
    draw_type: s32;
    serial: u64;
    send_event: s32;
    display: *Display;
    drawable: GLXDrawable;
    buffer_mask: u32;
    aux_buffer: u32;
    x: s32;
    y: s32;
    width: s32;
    height: s32;
    count: s32;
}

GLXBufferSwapComplete :: struct {
    type: s32;
    serial: u64;
    send_event: s32;
    display: *Display;
    drawable: GLXDrawable;
    event_type: s32;
    ust: s64;
    msc: s64;
    sbc: s64;
}

GLXEvent :: union {
    glxpbufferclobber: GLXPbufferClobberEvent;
    glxbufferswapcomplete: GLXBufferSwapComplete;
    pad: [24] s64;
}

// Opaque: the library owns the contents.
__GLXFBConfigRec :: struct {}

// Opaque: the library owns the contents.
__GLXcontextRec :: struct {}


// ------------------------------------------------------------------ enums ---


// ------------------------------------------------------------- procedures ---

glXAllocateMemoryNV :: (size: GLsizei, readfreq: GLfloat, writefreq: GLfloat, priority: GLfloat) -> *void #foreign libgl;
glXChooseFBConfig :: (dpy: *Display, screen: s32, attribList: *s32, nitems: *s32) -> *GLXFBConfig #foreign libgl;
glXChooseVisual :: (dpy: *Display, screen: s32, attribList: *s32) -> *XVisualInfo #foreign libgl;
glXCopyContext :: (dpy: *Display, src: GLXContext, dst: GLXContext, mask: u64) -> void #foreign libgl;
glXCreateContext :: (dpy: *Display, vis: *XVisualInfo, shareList: GLXContext, direct: s32) -> GLXContext #foreign libgl;
glXCreateGLXPixmap :: (dpy: *Display, visual: *XVisualInfo, pixmap: Pixmap) -> GLXPixmap #foreign libgl;
glXCreateNewContext :: (dpy: *Display, config: GLXFBConfig, renderType: s32, shareList: GLXContext, direct: s32) -> GLXContext #foreign libgl;
glXCreatePbuffer :: (dpy: *Display, config: GLXFBConfig, attribList: *s32) -> GLXPbuffer #foreign libgl;
glXCreatePixmap :: (dpy: *Display, config: GLXFBConfig, pixmap: Pixmap, attribList: *s32) -> GLXPixmap #foreign libgl;
glXCreateWindow :: (dpy: *Display, config: GLXFBConfig, win: Window, attribList: *s32) -> GLXWindow #foreign libgl;
glXDestroyContext :: (dpy: *Display, ctx: GLXContext) -> void #foreign libgl;
glXDestroyGLXPixmap :: (dpy: *Display, pixmap: GLXPixmap) -> void #foreign libgl;
glXDestroyPbuffer :: (dpy: *Display, pbuf: GLXPbuffer) -> void #foreign libgl;
glXDestroyPixmap :: (dpy: *Display, pixmap: GLXPixmap) -> void #foreign libgl;
glXDestroyWindow :: (dpy: *Display, window: GLXWindow) -> void #foreign libgl;
glXFreeMemoryNV :: (pointer: *GLvoid) -> void #foreign libgl;
glXGetClientString :: (dpy: *Display, name: s32) -> *u8 #foreign libgl;
glXGetConfig :: (dpy: *Display, visual: *XVisualInfo, attrib: s32, value: *s32) -> s32 #foreign libgl;
glXGetCurrentContext :: () -> GLXContext #foreign libgl;
glXGetCurrentDisplay :: () -> *Display #foreign libgl;
glXGetCurrentDrawable :: () -> GLXDrawable #foreign libgl;
glXGetCurrentReadDrawable :: () -> GLXDrawable #foreign libgl;
glXGetFBConfigAttrib :: (dpy: *Display, config: GLXFBConfig, attribute: s32, value: *s32) -> s32 #foreign libgl;
glXGetFBConfigs :: (dpy: *Display, screen: s32, nelements: *s32) -> *GLXFBConfig #foreign libgl;
glXGetProcAddressARB :: (__arg0: *GLubyte) -> __GLXextFuncPtr #foreign libgl;
glXGetSelectedEvent :: (dpy: *Display, drawable: GLXDrawable, mask: *u64) -> void #foreign libgl;
glXGetVisualFromFBConfig :: (dpy: *Display, config: GLXFBConfig) -> *XVisualInfo #foreign libgl;
glXIsDirect :: (dpy: *Display, ctx: GLXContext) -> s32 #foreign libgl;
glXMakeContextCurrent :: (dpy: *Display, draw: GLXDrawable, read: GLXDrawable, ctx: GLXContext) -> s32 #foreign libgl;
glXMakeCurrent :: (dpy: *Display, drawable: GLXDrawable, ctx: GLXContext) -> s32 #foreign libgl;
glXQueryContext :: (dpy: *Display, ctx: GLXContext, attribute: s32, value: *s32) -> s32 #foreign libgl;
glXQueryDrawable :: (dpy: *Display, draw: GLXDrawable, attribute: s32, value: *u32) -> void #foreign libgl;
glXQueryExtension :: (dpy: *Display, errorb: *s32, event: *s32) -> s32 #foreign libgl;
glXQueryExtensionsString :: (dpy: *Display, screen: s32) -> *u8 #foreign libgl;
glXQueryServerString :: (dpy: *Display, screen: s32, name: s32) -> *u8 #foreign libgl;
glXQueryVersion :: (dpy: *Display, maj: *s32, min: *s32) -> s32 #foreign libgl;
glXSelectEvent :: (dpy: *Display, drawable: GLXDrawable, mask: u64) -> void #foreign libgl;
glXSwapBuffers :: (dpy: *Display, drawable: GLXDrawable) -> void #foreign libgl;
glXUseXFont :: (font: Font, first: s32, count: s32, list: s32) -> void #foreign libgl;
glXWaitGL :: () -> void #foreign libgl;
glXWaitX :: () -> void #foreign libgl;
