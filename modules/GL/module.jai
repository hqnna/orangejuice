//
// GL: OpenGL, from 1.0 through 4.6 and the registered extensions.
//
// OpenGL's entry points are not linked: a driver hands them out at run time,
// once a context exists, through a `GetProcAddress` the window system
// provides. So every one of them is a member of `GL_Procedures`, and `gl` —
// which this module `using`s, so that `glClear(...)` names the member — is
// filled in by `gl_load`:
//
//     gl_load(*gl);                         // through glXGetProcAddressARB
//     gl_load(*gl, SDL_GL_GetProcAddress);  // or a loader of the caller's
//
// `gl_load` also sets the struct's `bool` members, one per version and one
// per extension (`GL_VERSION_3_3`, `GL_ARB_debug_output`), from what the
// context that is current when it is called reports. With none current they
// all stay false; the procedures are loaded regardless, since GLX answers for
// a name whether or not a context supports it.
//
// The constants and types are those of `<GL/gl.h>` and `<GL/glext.h>`, and
// `glx_generated.jai` is `<GL/glx.h>` — how a context is made on an X
// display. Both are projected from the system's own headers by `tools/cbind`;
// the GLX side is linked against `libGL` with `#library,system`.
//
// `DEFAULT_MSAA` is the sample count `glx_visual_attributes` asks for, which
// is what a window library importing this module (`#import "GL"(DEFAULT_MSAA
// = 4)`) builds its framebuffer with.
//

#module_parameters (DEFAULT_MSAA: s32 = 0);

#import "Basic";
#import "String";

#if OS == .LINUX {
    #import "X11";
}

#scope_module

#if OS == .LINUX {
    libgl :: #library,system "libGL";
}

#scope_export

GetProcAddressFunction :: #type (name: *u8) -> *void #c_call;

using gl: GL_Procedures;

#if OS == .LINUX {
    // The loader `gl_load` uses when it is given none.
    glx_get_proc_address :: (name: *u8) -> *void #c_call {
        return cast(*void) glXGetProcAddressARB(name);
    }

    DEFAULT_GET_PROC_ADDRESS :: glx_get_proc_address;

    // The attribute list for `glXChooseVisual`: double-buffered RGBA with a
    // depth and a stencil buffer, multisampled when `DEFAULT_MSAA` says so.
    glx_visual_attributes :: (msaa := DEFAULT_MSAA) -> [..] s32 {
        attributes: [..] s32;
        array_add(*attributes, GLX_RGBA, GLX_DOUBLEBUFFER, GLX_RED_SIZE, 8, GLX_GREEN_SIZE, 8, GLX_BLUE_SIZE, 8, GLX_DEPTH_SIZE, 24, GLX_STENCIL_SIZE, 8);
        if msaa > 0  array_add(*attributes, GLX_SAMPLE_BUFFERS, 1, GLX_SAMPLES, msaa);
        array_add(*attributes, 0);
        return attributes;
    }
} else {
    DEFAULT_GET_PROC_ADDRESS :: cast(GetProcAddressFunction) null;
}

// Fills in every procedure of `procs` by asking `get_proc_address` for it by
// name, then sets the version and extension flags from the current context.
gl_load :: (procs: *GL_Procedures, get_proc_address: GetProcAddressFunction = DEFAULT_GET_PROC_ADDRESS) {
    if !get_proc_address  return;

    info := type_info(GL_Procedures);
    base := cast(*u8) procs;
    name: [256] u8;
    for info.members {
        if it.type.type != .PROCEDURE  continue;
        if it.name.count >= name.count  continue;
        memcpy(name.data, it.name.data, it.name.count);
        name[it.name.count] = 0;
        slot := cast(**void) (base + it.offset_in_bytes);
        slot.* = get_proc_address(name.data);
    }

    major, minor := gl_version(procs);
    extensions := gl_extensions(procs, major);
    defer array_free(extensions);

    for info.members {
        if it.type.type != .BOOL  continue;
        flag := cast(*bool) (base + it.offset_in_bytes);
        if begins_with(it.name, "GL_VERSION_") {
            wanted_major := cast(s32) (it.name[11] - #char "0");
            wanted_minor := cast(s32) (it.name[13] - #char "0");
            flag.* = major > wanted_major || (major == wanted_major && minor >= wanted_minor);
        } else {
            flag.* = array_find(extensions, it.name);
        }
    }
}

// The version the current context reports, or 0.0 with none current.
gl_version :: (procs: *GL_Procedures = *gl) -> (major: s32, minor: s32) {
    if !procs.glGetString  return 0, 0;
    text := procs.glGetString(GL_VERSION);
    if !text  return 0, 0;
    version := to_string(text);
    // An OpenGL ES context says so before the number.
    while version && (version[0] < #char "0" || version[0] > #char "9")  advance(*version);
    major, minor: s32;
    while version && version[0] >= #char "0" && version[0] <= #char "9" {
        major = major * 10 + version[0] - #char "0";
        advance(*version);
    }
    if version && version[0] == #char "."  advance(*version);
    while version && version[0] >= #char "0" && version[0] <= #char "9" {
        minor = minor * 10 + version[0] - #char "0";
        advance(*version);
    }
    return major, minor;
}

#scope_file

gl_extensions :: (procs: *GL_Procedures, major: s32) -> [..] string {
    result: [..] string;
    if major >= 3 && procs.glGetStringi && procs.glGetIntegerv {
        count: GLint;
        procs.glGetIntegerv(GL_NUM_EXTENSIONS, *count);
        for 0..count - 1 {
            text := procs.glGetStringi(GL_EXTENSIONS, cast(GLuint) it);
            if text  array_add(*result, to_string(text));
        }
    } else if procs.glGetString {
        text := procs.glGetString(GL_EXTENSIONS);
        if text {
            all := to_string(text);
            while all {
                end := 0;
                while end < all.count && all[end] != #char " "  end += 1;
                if end  array_add(*result, slice(all, 0, end));
                advance(*all, min(end + 1, all.count));
            }
        }
    }
    return result;
}

#scope_export

#load "generated.jai";

#if OS == .LINUX {
    #load "glx_generated.jai";
}
