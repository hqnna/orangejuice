//! `Window_Creation`, `Input` and `Simp`: the windowing and 2D drawing stack
//! a graphical program imports, over `X11`, `GL`, `stb_image` and `freetype`.
//!
//! Each program is built against orangejuice's own modules and run on an
//! `Xvfb` of its own. Input is tested by sending the program's own window the
//! events a user would cause — keys, buttons, a resize, the window manager's
//! close request — and checking what `Input` makes of them. Drawing is tested
//! by reading back the framebuffer, which needs GLX on the display: when the
//! virtual display has none the render half falls back to `$DISPLAY`, and with
//! neither it says so and skips.
//!
//! When the dev shell's C library is older than the one the system's OpenGL
//! driver was built against, the driver does not load into the program and
//! GLX looks absent. `OJ_TEST_GL_LOADER` names a dynamic loader to run the
//! drawing program through instead — the system's `ld-linux` — with
//! `OJ_TEST_GL_LIBRARY_PATH` (default `/run/opengl-driver/lib`) as its library
//! path.
//!
//! The Way to Jai's chapter 33, 51 and 52 programs that use these modules are
//! built too, when `OJ_TTWJ_DIR` names a checkout of the book.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

fn linker_is_available() -> bool {
  let driver = oj_link::driver(oj_types::Target::HOST);
  Command::new(&driver)
    .arg("--version")
    .output()
    .is_ok_and(|output| output.status.success())
}

fn system_has(packages: &[&str]) -> bool {
  let known = Command::new("pkg-config")
    .arg("--exists")
    .args(packages)
    .status()
    .is_ok_and(|status| status.success());
  if known {
    return true;
  }
  if oj_testsupport::linker_required() {
    panic!("pkg-config does not know {packages:?}, and this run requires everything to link");
  }
  eprintln!("skipping: pkg-config does not know {packages:?}");
  false
}

struct Built {
  directory: tempfile::TempDir,
  executable: PathBuf,
}

fn build_file(path: &Path) -> Result<PathBuf, String> {
  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let options = oj_driver::BuildOptions::new();
  let report = oj_driver::run(path, &options, oj_driver::Stage::Executable, None);
  if report.failed {
    return Err(report.diagnostics.join(""));
  }
  report
    .executable
    .ok_or_else(|| "a successful build has an executable".to_string())
}

fn build(body: &str) -> Option<Built> {
  if !linker_is_available() {
    oj_testsupport::skip_without_linker("no C driver on PATH to link with");
    return None;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, body).expect("the input should be writable");
  let executable = build_file(&path).unwrap_or_else(|diagnostics| {
    panic!("the program should build against our own modules, but:\n{diagnostics}")
  });
  Some(Built {
    directory,
    executable,
  })
}

fn run(
  built: &Built,
  loader: Option<&GlLoader>,
  arguments: &[&str],
  display: Option<&str>,
) -> Output {
  let mut command = match loader {
    Some(loader) => {
      let mut command = Command::new(&loader.loader);
      command
        .arg("--library-path")
        .arg(&loader.library_path)
        .arg(&built.executable);
      command
    }
    None => Command::new(&built.executable),
  };
  command.args(arguments).current_dir(built.directory.path());
  match display {
    Some(display) => command.env("DISPLAY", display),
    None => command.env_remove("DISPLAY"),
  };
  command.output().expect("the produced program should run")
}

fn stdout_of(output: &Output) -> String {
  assert!(
    output.status.success(),
    "the program should run, but exited {:?}:\n{}{}",
    output.status.code(),
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A virtual X server for one test, stopped when it is dropped.
struct VirtualDisplay {
  child: Child,
  display: String,
}

impl VirtualDisplay {
  fn start() -> Option<VirtualDisplay> {
    let mut child = Command::new("Xvfb")
      .args([
        "-displayfd",
        "1",
        "-screen",
        "0",
        "640x480x24",
        "-nolisten",
        "tcp",
      ])
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .ok()?;
    let mut line = String::new();
    let stdout = child.stdout.take()?;
    BufReader::new(stdout).read_line(&mut line).ok()?;
    let number = line.trim();
    if number.is_empty() {
      let _ = child.kill();
      return None;
    }
    Some(VirtualDisplay {
      display: format!(":{number}"),
      child,
    })
  }
}

impl Drop for VirtualDisplay {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

struct GlLoader {
  loader: PathBuf,
  library_path: String,
}

fn gl_loader() -> Option<GlLoader> {
  let loader = std::env::var_os("OJ_TEST_GL_LOADER")?;
  let library_path = std::env::var("OJ_TEST_GL_LIBRARY_PATH")
    .unwrap_or_else(|_| "/run/opengl-driver/lib".to_string());
  Some(GlLoader {
    loader: PathBuf::from(loader),
    library_path,
  })
}

fn system_font() -> Option<String> {
  let output = Command::new("fc-match")
    .args(["-f", "%{file}", "sans"])
    .output()
    .ok()?;
  let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
  (output.status.success() && Path::new(&path).is_file()).then_some(path)
}

const WINDOW_JAI: &str = r##"#import "Basic";
#import "Input";
#import "Window_Creation";
X :: #import "X11";

send_key :: (window: Window_Type, keysym: X.KeySym, press: bool, state: u32 = 0) {
    display := X.x_global_display;
    event: X.XEvent;
    event.xkey.type = xx ifx press then X.KeyPress else X.KeyRelease;
    event.xkey.display = display;
    event.xkey.window = window;
    event.xkey.root = X.XDefaultRootWindow(display);
    event.xkey.state = state;
    event.xkey.keycode = X.XKeysymToKeycode(display, keysym);
    event.xkey.same_screen = X.True;
    X.XSendEvent(display, window, X.True, ifx press then X.KeyPressMask else X.KeyReleaseMask, *event);
}

send_button :: (window: Window_Type, button: u32, press: bool) {
    display := X.x_global_display;
    event: X.XEvent;
    event.xbutton.type = xx ifx press then X.ButtonPress else X.ButtonRelease;
    event.xbutton.display = display;
    event.xbutton.window = window;
    event.xbutton.button = button;
    event.xbutton.same_screen = X.True;
    X.XSendEvent(display, window, X.True, ifx press then X.ButtonPressMask else X.ButtonReleaseMask, *event);
}

send_close :: (window: Window_Type) {
    display := X.x_global_display;
    event: X.XEvent;
    event.xclient.type = X.ClientMessage;
    event.xclient.window = window;
    event.xclient.message_type = X.XInternAtom(display, "WM_PROTOCOLS", X.False);
    event.xclient.format = 32;
    event.xclient.data.l[0] = xx X.XInternAtom(display, "WM_DELETE_WINDOW", X.False);
    X.XSendEvent(display, window, X.False, 0, *event);
}

frame :: () {
    X.XSync(X.x_global_display, X.False);
    update_window_events();
}

describe :: (event: Event) -> string {
    if event.type == {
      case .KEYBOARD;
        name := ifx event.key_code >= 32 && event.key_code < 127 then tprint("'%'", string.{1, cast(*u8) *event.key_code}) else tprint("%", event.key_code);
        return tprint("key % % shift % ctrl % repeat %", name, ifx event.key_pressed then "down" else "up", event.shift_pressed, event.ctrl_pressed, event.repeat);
      case .TEXT_INPUT;
        return tprint("text % (#%)", event.utf32, event.text_input_count);
      case .MOUSE_WHEEL;
        return tprint("wheel % of %", event.wheel_delta, event.typical_wheel_delta);
      case .WINDOW;  return "window";
      case .QUIT;    return "quit";
    }
    return tprint("%", event.type);
}

main :: () {
    window := create_window(320, 200, "Input test", window_class = "oj_test");
    if window == INVALID_WINDOW {
        print("no window\n");
        return;
    }
    x, y, w, h, ok := get_dimensions(window, false);
    print("dimensions %x% %\n", w, h, ok);

    send_key(window, X.XK_a, true, X.ShiftMask);
    send_key(window, X.XK_a, false, X.ShiftMask);
    send_key(window, X.XK_Left, true, X.ControlMask);
    send_key(window, X.XK_Left, true, X.ControlMask);
    send_key(window, X.XK_Escape, true);
    send_key(window, X.XK_F5, true);
    send_key(window, X.XK_KP_7, true);
    send_key(window, X.XK_1, true, X.ShiftMask);
    send_button(window, 1, true);
    send_button(window, 4, true);
    send_button(window, 4, false);
    frame();
    for events_this_frame  print("%\n", describe(it));
    print("left % a % mouse % wheel %\n", input_button_states[Key_Code.ARROW_LEFT], input_button_states[#char "A"], input_button_states[Key_Code.MOUSE_BUTTON_LEFT], mouse_delta_z);

    send_key(window, X.XK_Left, false);
    send_button(window, 1, false);
    frame();
    for events_this_frame  print("%\n", describe(it));
    print("left % a % mouse %\n", input_button_states[Key_Code.ARROW_LEFT], input_button_states[#char "A"], input_button_states[Key_Code.MOUSE_BUTTON_LEFT]);
    frame();
    print("quiet frame: % events, left %\n", events_this_frame.count, input_button_states[Key_Code.ARROW_LEFT]);

    X.XResizeWindow(X.x_global_display, window, 400, 300);
    X.XResizeWindow(X.x_global_display, window, 410, 310);
    frame();
    for get_window_resizes()  print("resize % %x%\n", it.window == window, it.width, it.height);
    print("resizes left %\n", get_window_resizes().count);
    x, y, w, h, ok = get_dimensions(window, true);
    print("dimensions %x% %\n", w, h, ok);

    X.XWarpPointer(X.x_global_display, X.None, window, 0, 0, 0, 0, 30, 40);
    frame();
    mx, my, mok := get_mouse_pointer_position(window, false);
    print("pointer % % %\n", mx, my, mok);
    mx, my, mok = get_mouse_pointer_position(window, true);
    print("pointer right-handed % % %\n", mx, my, mok);
    X.XWarpPointer(X.x_global_display, X.None, window, 0, 0, 0, 0, 35, 37);
    frame();
    print("mouse delta % %\n", mouse_delta_x, mouse_delta_y);

    saved: Saved_Window_Info;
    full, fw, fh := toggle_fullscreen(window, true, *saved);
    print("fullscreen % saved %x%\n", full, saved.width, saved.height);
    full, fw, fh = toggle_fullscreen(window, false, *saved);
    print("windowed % %x%\n", full, fw, fh);

    send_close(window);
    frame();
    for events_this_frame  print("%\n", describe(it));
}
"##;

/// A window is made, measured and resized, and what is sent to it — shifted
/// and auto-repeated keys, function and keypad keys, a mouse button and the
/// wheel, pointer motion, the window manager's close request — comes out of
/// `Input` as the events and button states a frame reads. Only on an `Xvfb`
/// of its own: the program maps a window and moves the pointer.
#[test]
fn input_reports_what_happens_to_a_window_creation_window() {
  if !system_has(&["x11"]) {
    return;
  }
  let Some(built) = build(WINDOW_JAI) else {
    return;
  };
  let Some(server) = VirtualDisplay::start() else {
    eprintln!("skipping: no Xvfb to open a window on");
    return;
  };
  let output = stdout_of(&run(&built, None, &[], Some(&server.display)));
  assert_eq!(
    output,
    "dimensions 320x200 true\n\
     key 'A' down shift true ctrl false repeat false\n\
     text 65 (#0)\n\
     key 'A' up shift true ctrl false repeat false\n\
     key ARROW_LEFT down shift false ctrl true repeat false\n\
     key ARROW_LEFT down shift false ctrl true repeat true\n\
     key ESCAPE down shift false ctrl false repeat false\n\
     key F5 down shift false ctrl false repeat false\n\
     key NUMPAD_7 down shift false ctrl false repeat false\n\
     key '1' down shift true ctrl false repeat false\n\
     text 33 (#1)\n\
     key MOUSE_BUTTON_LEFT down shift false ctrl false repeat false\n\
     wheel 120 of 120\n\
     left DOWN | START a DOWN | START | END mouse DOWN | START wheel 120\n\
     key ARROW_LEFT up shift false ctrl false repeat false\n\
     key MOUSE_BUTTON_LEFT up shift false ctrl false repeat false\n\
     left DOWN | END a NONE mouse DOWN | END\n\
     quiet frame: 0 events, left NONE\n\
     resize true 410x310\n\
     resizes left 0\n\
     dimensions 410x310 true\n\
     pointer 30 40 true\n\
     pointer right-handed 30 270 true\n\
     mouse delta 5 -3\n\
     fullscreen true saved 410x310\n\
     windowed true 410x310\n\
     quit\n"
  );
}

#[test]
fn without_a_display_there_is_no_window_and_no_events() {
  if !system_has(&["x11"]) {
    return;
  }
  let Some(built) = build(WINDOW_JAI) else {
    return;
  };
  let output = run(&built, None, &[], None);
  assert_eq!(stdout_of(&output), "no window\n");
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("could not open the X display"),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
}

const SIMP_JAI: &str = r##"#import "Basic";
#import "Math";
#import "Input";
#import "Window_Creation";
#import "Simp";
#import "GL";
#import "String";
#import "stb_image_write";

pixel_at :: (x: s32, y: s32) -> [4] u8 {
    immediate_flush();
    pixel: [4] u8;
    glReadPixels(x, y, 1, 1, GL_RGBA, GL_UNSIGNED_BYTE, pixel.data);
    return pixel;
}

main :: () {
    args := get_command_line_arguments();
    directory := args[1];
    font_path := ifx args.count > 2 then args[2] else "";

    // Everything that works without a window: an image from a file, a font
    // and the width of a string.
    image_path := tprint("%/stripes.png", directory);
    stripes: [2 * 2 * 4] u8 = .[255, 0, 0, 255,  255, 0, 0, 255,  0, 0, 255, 255,  0, 0, 255, 255];
    stbi_write_png(temp_c_string(image_path), 2, 2, 4, stripes.data, 8);
    early: Texture;
    print("early texture % %x%\n", texture_load_from_file(*early, image_path), early.width, early.height);
    missing: Texture;
    print("missing texture %\n", texture_load_from_file(*missing, tprint("%/missing.png", directory)));

    font: *Dynamic_Font;
    if font_path {
        font = get_font_at_size(path_strip_filename(font_path), path_filename(font_path), 32);
        print("font % same %\n", font != null, get_font_at_size(path_strip_filename(font_path), path_filename(font_path), 32) == font);
        print("text width % % %\n", get_text_width(font, "Hi") > 0, prepare_text(font, "Hello") > get_text_width(font, "Hi"), font.default_line_spacing >= font.character_height);
    }

    window := create_window(64, 48, "Simp test");
    set_render_target(window);
    if !context.simp.current_window {
        print("no context\n");
        return;
    }
    W, H := get_render_dimensions(window);
    print("render target %\n", W > 0 && H > 0);

    clear_render_target(0, 0, 1, 1);
    set_shader_for_color();
    immediate_quad(0, 0, 32, 24, .{1, 0, 0, 1});
    print("right-handed: quad % background %\n", pixel_at(5, 5), pixel_at(W - 6, H - 6));

    set_shader_for_color(true);
    immediate_quad(0, 0, xx W, xx H, .{0, 1, 0, 0.5});
    blended := pixel_at(5, 5);
    print("blended % %\n", blended[0] >= 125 && blended[0] <= 130 && blended[1] >= 125 && blended[1] <= 130, blended[2]);

    clear_render_target(0, 0, 0, 1);
    set_shader_for_color();
    immediate_triangle(.{0, 0, 0}, .{xx W, 0, 0}, .{0, xx H, 0}, .{1, 1, 1, 1}, .{1, 1, 1, 1}, .{1, 1, 1, 1});
    print("triangle inside % outside %\n", pixel_at(3, 3), pixel_at(W - 4, H - 4));

    clear_render_target(0, 0, 0, 1);
    set_scissor(0, 0, 16, 16);
    immediate_quad(0, 0, xx W, xx H, .{1, 1, 0, 1});
    clear_scissor();
    print("scissor inside % outside %\n", pixel_at(5, 5), pixel_at(30, 30));

    clear_render_target(0, 0, 0, 1);
    set_shader_for_images(*early);
    immediate_quad(0, 0, xx W, xx H);
    print("image top % bottom %\n", pixel_at(W / 2, H - 6), pixel_at(W / 2, 6));

    bitmap: Bitmap;
    bitmap_alloc(*bitmap, 1, 1, .RGBA8);
    bitmap.data[0] = 10; bitmap.data[1] = 200; bitmap.data[2] = 30; bitmap.data[3] = 255;
    made: Texture;
    texture_load_from_bitmap(*made, *bitmap);
    deinit(*bitmap);
    set_shader_for_images(*made);
    immediate_quad(Vector2.{0, 0}, Vector2.{xx W, 0}, Vector2.{xx W, xx H}, Vector2.{0, xx H}, .{1, 1, 1, 1});
    print("bitmap %\n", pixel_at(W / 2, H / 2));

    if font {
        clear_render_target(0, 0, 0, 1);
        width := draw_text(font, 4, 10, "HH", .{0, 1, 0, 1});
        immediate_flush();
        pixels := NewArray(W * H * 4, u8);
        glReadPixels(0, 0, xx W, xx H, GL_RGBA, GL_UNSIGNED_BYTE, pixels.data);
        lit, foreign, below := 0;
        for y: 0..H - 1  for x: 0..W - 1 {
            p := (y * W + x) * 4;
            if pixels[p + 1] > 200 {
                lit += 1;
                if y < 10  below += 1;
            }
            if pixels[p] || pixels[p + 2]  foreign += 1;
        }
        print("text width % lit % other colours % below the baseline %\n", width > 0, lit > 40, foreign, below);
    }

    set_render_target(window, .LEFT_HANDED);
    clear_render_target(0, 0, 1, 1);
    set_shader_for_color();
    immediate_quad(0, 0, 32, 24, .{1, 0, 0, 1});
    print("left-handed: quad % background %\n", pixel_at(5, H - 5), pixel_at(5, 5));
    print("coordinate system %\n", context.simp.coordinate_system);

    X :: #import "X11";
    X.XResizeWindow(X.x_global_display, window, xx (W + 16), xx (H + 12));
    seen := false;
    for 1..200 {
        X.XSync(X.x_global_display, X.False);
        update_window_events();
        for get_window_resizes()  if it.window == window {
            update_window(it.window);
            width, height := get_render_dimensions(window);
            seen = width == it.width && height == it.height && context.simp.render_target_width == it.width && context.simp.render_target_height == it.height;
        }
        if seen  break;
        sleep_milliseconds(5);
    }
    print("resize followed %\n", seen);
    swap_buffers(window);
    print("GL error %\n", glGetError());
}
"##;

const SIMP_HEADLESS: &str = "early texture true 2x2\nmissing texture false\n";
const SIMP_FONT: &str = "font true same true\ntext width true true true\n";
const SIMP_DRAWN: &str = "render target true\n\
  right-handed: quad [255, 0, 0, 255] background [0, 0, 255, 255]\n\
  blended true 0\n\
  triangle inside [255, 255, 255, 255] outside [0, 0, 0, 255]\n\
  scissor inside [255, 255, 0, 255] outside [0, 0, 0, 255]\n\
  image top [255, 0, 0, 255] bottom [0, 0, 255, 255]\n\
  bitmap [10, 200, 30, 255]\n";
const SIMP_TEXT: &str = "text width true lit true other colours 0 below the baseline 0\n";
const SIMP_AFTER: &str = "left-handed: quad [255, 0, 0, 255] background [0, 0, 255, 255]\n\
  coordinate system LEFT_HANDED\n\
  resize followed true\n\
  GL error 0\n";

/// Before any window, an image file loads into a `Texture` and a system font
/// opens and measures text. Then, on a display with GLX, every way of drawing
/// is read back from the framebuffer: a quad where the right-handed
/// coordinate system puts it, alpha blending, a triangle, the scissor, the
/// image the right way up, a texture made from a `Bitmap`, text in its colour
/// above its baseline, the left-handed coordinate system, and the render
/// target following a resize.
#[test]
fn simp_draws_into_a_window_and_reads_back_what_it_drew() {
  if !system_has(&["gl", "x11", "freetype2"]) {
    return;
  }
  let Some(built) = build(SIMP_JAI) else {
    return;
  };
  let directory = built.directory.path().to_string_lossy().into_owned();
  let font = system_font();
  if font.is_none() {
    eprintln!("no font: fc-match found none, so text is not drawn");
  }
  let mut arguments = vec![directory.as_str()];
  if let Some(font) = &font {
    arguments.push(font);
  }
  let headless = if font.is_some() {
    format!("{SIMP_HEADLESS}{SIMP_FONT}")
  } else {
    SIMP_HEADLESS.to_string()
  };

  let output = stdout_of(&run(&built, None, &arguments, None));
  assert_eq!(output, format!("{headless}no context\n"));

  let server = VirtualDisplay::start();
  let mut displays: Vec<String> = server.iter().map(|server| server.display.clone()).collect();
  if let Ok(display) = std::env::var("DISPLAY")
    && !display.is_empty()
  {
    displays.push(display);
  }
  let loader = gl_loader();
  for display in &displays {
    let mut output = stdout_of(&run(&built, None, &arguments, Some(display)));
    if output.ends_with("no context\n")
      && let Some(loader) = &loader
    {
      output = stdout_of(&run(&built, Some(loader), &arguments, Some(display)));
    }
    if output.ends_with("no context\n") {
      eprintln!("{display} has no GLX this program can use");
      continue;
    }
    let text = if font.is_some() { SIMP_TEXT } else { "" };
    assert_eq!(output, format!("{headless}{SIMP_DRAWN}{text}{SIMP_AFTER}"));
    return;
  }
  eprintln!("skipping the drawing half: no display with GLX (see OJ_TEST_GL_LOADER)");
}

const GETRECT_JAI: &str = r##"#import "Basic";
#import "Math";
#import "Input";
#import "Window_Creation";
#import "Simp";
#import "GetRect";
#import "GL";
X :: #import "X11";

W :: 320;
H :: 240;

pixel_at :: (x: float, y: float) -> [4] u8 {
    immediate_flush();
    pixel: [4] u8;
    glReadPixels(xx x, xx y, 1, 1, GL_RGBA, GL_UNSIGNED_BYTE, pixel.data);
    return pixel;
}

as_bytes :: (c: Vector4) -> [4] u8 {
    return .[cast(u8) (c.x * 255 + .5), cast(u8) (c.y * 255 + .5), cast(u8) (c.z * 255 + .5), cast(u8) (c.w * 255 + .5)];
}

near :: (a: [4] u8, b: [4] u8) -> bool {
    for 0..3  if abs(cast(int) a[it] - cast(int) b[it]) > 2  return false;
    return true;
}

mouse_event :: (pressed: bool) -> Event {
    event: Event;
    event.type = .KEYBOARD;
    event.key_code = .MOUSE_BUTTON_LEFT;
    event.key_pressed = xx pressed;
    return event;
}

value := 0.5;
choice: s32 = 0;
clicks := 0;
under := 0;

window: Window_Type;

// One frame: the button goes down and/or up, the pointer is where it is
// asked to be, and the widgets are drawn.
frame :: (x: float, y: float, press := false, release := false, warp := false) -> (button_clicked: bool, slid: bool, state: *Dropdown_State) {
    update_window_events();
    if warp {
        X.XWarpPointer(X.x_global_display, 0, window, 0, 0, 0, 0, xx x, xx (H - y));
        X.XSync(X.x_global_display, X.False);
    }
    if press    getrect_handle_event(mouse_event(true));
    if release  getrect_handle_event(mouse_event(false));
    clear_render_target(0, 0, 0, 1);
    ui_per_frame_update(window, W, H, get_time());
    if !warp  ui_set_mouse(x, y);

    clicked := button(get_rect(10, 200, 140, 30), "Button");
    if clicked  clicks += 1;
    slid := slider(get_rect(10, 160, 140, 30), *value, 0.0, 1.0, 0.25, prefix = "v ");
    state := dropdown(get_rect(10, 120, 140, 30), .["one", "two", "three"], *choice);
    if button(get_rect(10, 60, 140, 30), "Under")  under += 1;
    draw_popups();
    return clicked, slid, state;
}

main :: () {
    args := get_command_line_arguments();
    window = create_window(W, H, "GetRect test");
    set_render_target(window);
    if !context.simp.current_window {
        print("no context\n");
        return;
    }
    ui_init();
    theme := default_theme();
    button_theme := theme.button_theme;

    frame(160, 10);
    print("idle %\n", near(pixel_at(14, 215), as_bytes(button_theme.surface_color)));
    // The pointer itself is only moved on a display of the test's own.
    frame(75, 215, warp = args.count > 1 && args[1] == "warp");
    print("hover %\n", near(pixel_at(14, 215), as_bytes(button_theme.surface_color_over)));
    frame(75, 215, press = true);
    print("held %\n", near(pixel_at(14, 215), as_bytes(button_theme.surface_color_down)));
    clicked := frame(75, 215, release = true);
    print("clicked % %\n", clicked, clicks);
    frame(75, 215, press = true);
    frame(200, 30, release = true);
    print("released elsewhere %\n", clicks);
    clicked = frame(75, 215, press = true, release = true);
    print("one-frame click % %\n", clicked, clicks);

    _, slid := frame(10 + 140 * .8, 175, press = true);
    print("slider % %\n", slid, value);
    _, slid = frame(10 + 140 * .1, 175);
    print("dragged % %\n", slid, value);
    frame(300, 175, release = true);
    print("let go %\n", value);
    frame(10 + 140 * .9, 175);
    print("not held %\n", value);

    _, _, state := frame(75, 135, press = true, release = true);
    print("open %\n", state.open);
    item_theme := theme.dropdown_theme.item_theme;
    print("list drawn % %\n", near(pixel_at(14, 45), as_bytes(item_theme.surface_color)), near(pixel_at(14, 75), as_bytes(item_theme.surface_color)));
    frame(75, 75, press = true, release = true);
    print("covered button % choice %\n", under, choice);
    _, _, state = frame(75, 135, press = true, release = true);
    _, _, state = frame(75, 45, press = true, release = true);
    print("picked % open %\n", choice, state.open);
    frame(75, 75, press = true, release = true);
    print("uncovered button %\n", under);

    swap_buffers(window);
    print("GL error %\n", glGetError());
}
"##;

const GETRECT_DRAWN: &str = "idle true\n\
  hover true\n\
  held true\n\
  clicked true 1\n\
  released elsewhere 1\n\
  one-frame click true 2\n\
  slider true 0.75\n\
  dragged true 0\n\
  let go 1\n\
  not held 1\n\
  open true\n\
  list drawn true true\n\
  covered button 0 choice 1\n\
  picked 2 open false\n\
  uncovered button 1\n\
  GL error 0\n";

/// GetRect's widgets on a display with GLX, frame by frame: a button shows
/// whether the pointer is over it (on the test's own display, read from where
/// `XWarpPointer` put it) and whether it is held, clicks only when the button goes up over the
/// widget it went down on, a slider follows a drag in steps and stops when
/// let go, and a dropdown opens a list that lies over the button under it and
/// sets the choice it was clicked on.
#[test]
fn getrect_widgets_answer_the_mouse_and_draw_their_state() {
  if !system_has(&["gl", "x11", "freetype2"]) {
    return;
  }
  let Some(built) = build(GETRECT_JAI) else {
    return;
  };
  let output = stdout_of(&run(&built, None, &[], None));
  assert_eq!(output, "no context\n");

  let server = VirtualDisplay::start();
  let mut displays: Vec<String> = server.iter().map(|server| server.display.clone()).collect();
  if let Ok(display) = std::env::var("DISPLAY")
    && !display.is_empty()
  {
    displays.push(display);
  }
  let loader = gl_loader();
  for display in &displays {
    let own = server
      .as_ref()
      .is_some_and(|server| &server.display == display);
    let arguments: &[&str] = if own { &["warp"] } else { &[] };
    let mut output = stdout_of(&run(&built, None, arguments, Some(display)));
    if output.ends_with("no context\n")
      && let Some(loader) = &loader
    {
      output = stdout_of(&run(&built, Some(loader), arguments, Some(display)));
    }
    if output.ends_with("no context\n") {
      eprintln!("{display} has no GLX this program can use");
      continue;
    }
    assert_eq!(output, GETRECT_DRAWN);
    return;
  }
  eprintln!("skipping: no display with GLX (see OJ_TEST_GL_LOADER)");
}

const SHAPES_JAI: &str = r##"#import "Basic";
#import "Math";
#import "Input";
#import "Window_Creation";
Simp :: #import "Simp";
WT :: #import "Window_Type";

LEFT_HANDED :: true;

Draw_Procs :: struct {
    get_font_at_size: type_of(#procedure_of_call Simp.get_font_at_size(memory, 1));
    immediate_quad: type_of(#procedure_of_call Simp.immediate_quad(p, p, p, p, c));
    set_scissor: type_of(Simp.set_scissor);
    texture_load_from_memory: type_of(Simp.texture_load_from_memory);
}
memory: [] u8;
p: Vector2;
c: Vector4;

Str_Code :: struct { str: string; code: Key_Code; }
using Key_Code;
NAMES :: Str_Code.[
    .{ "ArrowUp", ARROW_UP },
    .{ "Escape",  ESCAPE },
    .{ "Space",   #char " " },
    .{ "F12",     F12 },
    .{ "Numpad0", NUMPAD_0 },
    .{ "MouseX1", MOUSE_BUTTON_X1 },
];

Z :: 90;

is_key_down :: inline (button: Key_Code) -> bool {
    return cast(bool) (input_button_states[button] & .DOWN);
}

renderer_state: struct {
    window: WT.Window_Type;
    text_shader: Simp.Shader;
};

TEXT_SHADER :: #string END
OUT_IN vec2 TextureCoords;
#ifdef VERTEX_SHADER
in vec4 vert_position;
in vec2 vert_uv0;
uniform mat4 projection;
void main() { gl_Position = projection * vec4(vert_position.xy, 0.0, 1.0); TextureCoords = vert_uv0; }
#endif
#ifdef FRAGMENT_SHADER
out vec4 color;
uniform sampler2D text_sampler;
void main() { color = texture(text_sampler, TextureCoords); }
#endif
END

shapes :: () {
    procs := Draw_Procs.{
        get_font_at_size = #procedure_of_call Simp.get_font_at_size(memory, 1),
        immediate_quad = #procedure_of_call Simp.immediate_quad(p, p, p, p, c),
        set_scissor = Simp.set_scissor,
        texture_load_from_memory = Simp.texture_load_from_memory,
    };

    window := create_window(800, 600, "Shapes", window_class = "shapes");
    other := create_window(window_name = "Example", width = 200, height = 200);
    Simp.set_render_target(window, ifx LEFT_HANDED then Simp.Coordinate_System.LEFT_HANDED else .RIGHT_HANDED);
    renderer_state.window = window;
    w, h := Simp.get_render_dimensions(window);
    _, _, ww, wh := get_dimensions(window, right_handed = false);

    renderer_state.text_shader.gl_handle = Simp.get_shader_program(TEXT_SHADER);
    renderer_state.text_shader.alpha_blend = true;
    if context.simp.current_shader != *renderer_state.text_shader {
        Simp.immediate_set_shader(*renderer_state.text_shader);
        Simp.set_projection();
    }
    if context.simp.coordinate_system == .LEFT_HANDED  Simp.set_shader_for_color(enable_blend = true);

    bitmap: Simp.Bitmap;
    Simp.bitmap_alloc(*bitmap, 16, 16, .R8,, temp);
    bitmap.flags &= ~.FREE_DATA_ON_DEINIT;
    assert(bitmap.data.count == 256);
    texture: Simp.Texture;
    Simp.texture_load_from_bitmap(*texture, *bitmap);
    gl_texture := texture.gl_handle;
    Simp.texture_load_from_file(*texture, "x.png", false, true);
    pixel := u8.[0x42, 0x4d];
    Simp.texture_load_from_memory(*texture, pixel);
    Simp.set_shader_for_images(*texture);

    font := Simp.get_font_at_size("fonts", "font.ttf", 24);
    font = Simp.get_font_at_size("name", memory, 24);
    effects: Simp.Font_Effects = 0;
    text_w := Simp.draw_text(font, 10, 10 + font.default_line_spacing + font.typical_descender, "text", .{1, 1, 1, 1}, effects);
    text_w = Simp.prepare_text(font, "text", effects);
    Simp.draw_prepared_text(font, 10, 10, color = .{1, 1, 1, 1});
    font.y_offset_for_my_latin_characters = -9;
    if font.character_height > 24  text_w = Simp.get_text_width(font, "+");

    x, y := 1.0, 2.0;
    s := Vector4.{1, 1, 1, 1};
    t := Vector4.{1, 1, 1, 0};
    Simp.immediate_quad(x, y, x + 10, y + 10, c00 = s, c01 = t, c10 = s, c11 = t);
    Simp.immediate_quad(x, y, x + 10, y + 10, s);
    Simp.immediate_quad(Vector3.{0, 0, 1}, .{1, 0, 1}, .{1, 1, 1}, .{0, 1, 1}, s, .{0, 0}, .{1, 0}, .{1, 1}, .{0, 1});
    Simp.immediate_quad(Vector2.{0, 0}, .{1, 0}, .{1, 1}, .{0, 1}, uv0 = .{0, 0}, uv1 = .{1, 0}, uv2 = .{1, 1}, uv3 = .{0, 1});
    center := Vector3.{5, 5, 0};
    Simp.immediate_triangle(center, center + Vector3.{1, 0, 0}, center + Vector3.{0, 1, 0}, s, s, s);
    Simp.immediate_triangle(Vector2.{0, 0}, .{1, 0}, .{0, 1}, s, s, s);
    Simp.immediate_begin();
    Simp.immediate_flush();
    Simp.set_scissor(0, 0, 10, 10);
    Simp.clear_scissor();
    Simp.clear_render_target(.1, .2, .3, 1);
    Simp.swap_buffers(window, true);

    update_window_events();
    for Input.get_window_resizes()  Simp.update_window(it.window);
    for *event: events_this_frame {
        if event.type == .KEYBOARD && event.key_pressed && event.key_code == .F4 && event.alt_pressed {}
        if event.type == .TEXT_INPUT && event.ctrl_pressed {
            remove event;
            continue;
        }
        if event.modifier_flags.packed == 0 && event.modifier_flags.shift_pressed {}
        if event.key_code == {
          case .ESCAPE; break;
          case Z;       break;
        }
    }
    if input_button_states[Key_Code.SHIFT] != .NONE && is_key_down(.SPACEBAR) && input_application_has_focus {}
    factor := mouse_delta_z / 120.0 + mouse_delta_x + mouse_delta_y;
    mx, my, ok := get_mouse_pointer_position(window, !LEFT_HANDED);
    mx, my, ok = get_mouse_pointer_position();
    info: Saved_Window_Info;
    full := toggle_fullscreen(window, true, *info);
    records: [] Window_Resize_Record = get_window_resizes();
}

Input :: #import "Input";

main :: () {
    if get_command_line_arguments().count > 99  shapes();
    print("% names, first %\n", NAMES.count, NAMES[0].code);
}
"##;

/// The calls real programs make — every overload of `immediate_quad` in the
/// forms Focus, hitboxer, jai-simpler and the editor write them, a shader of
/// the program's own, `context.simp`, a `Bitmap` in temporary storage, key
/// codes named through `using Key_Code` and from `#char` — typecheck, lower
/// and link. The program does not open a window when run.
#[test]
fn the_call_shapes_real_programs_use_build() {
  if !system_has(&["gl", "x11", "freetype2"]) {
    return;
  }
  let Some(built) = build(SHAPES_JAI) else {
    return;
  };
  let output = stdout_of(&run(&built, None, &[], None));
  assert_eq!(output, "6 names, first ARROW_UP\n");
}

/// The Way to Jai's programs over these modules, each with why it is not
/// built when it is not.
const TTWJ_PROGRAMS: &[(&str, Option<&str>)] = &[
  ("33/33.2A_simp_window.jai", None),
  ("33/33.2B_bouncing_square.jai", None),
  (
    "33/33.2C_simp_window.jai",
    Some("imports Windows_Resources, which is Windows-only"),
  ),
  ("33/33.3_getrect_buttons.jai", None),
  (
    "33/33.6_window_creation_events.jai",
    Some("imports the Windows module"),
  ),
  ("33/33.8_load_font.jai", None),
  ("33/33.9_load_texture.jai", None),
  (
    "33/33.10_play_sound.jai",
    Some("needs Sound_Player, which the distribution does not ship"),
  ),
  ("33/33.11_colored_triangle.jai", None),
  (
    "33/33.12_drawing_circle.jai.jai",
    Some("`2*PI/30 * it` is a float64 under L§2.6, and make_vector2 takes float32"),
  ),
  ("33/33.13_drawing_board.jai", None),
  ("33/33.14_moving_shape.jai", None),
  ("33/33.15_drawing_texture.jai", None),
  ("51/51.2_game_of_life_gui.jai", None),
  ("52/52.1_simp_pong.jai", None),
];

/// What the book's programs that load a font and a texture without opening
/// a window do when run from their own directory, beside their assets.
const TTWJ_HEADLESS: &[&str] = &["33/33.8_load_font.jai", "33/33.9_load_texture.jai"];

/// Every program The Way to Jai writes against Window_Creation, Input and
/// Simp that does not need a module the distribution lacks builds, and the
/// two that need no window run. Only when `OJ_TTWJ_DIR` names the book.
#[test]
fn the_way_to_jai_graphical_programs_build() {
  let Some(book) = std::env::var_os("OJ_TTWJ_DIR") else {
    eprintln!("skipping: OJ_TTWJ_DIR does not name a checkout of The Way to Jai");
    return;
  };
  if !system_has(&["gl", "x11", "freetype2"]) || !linker_is_available() {
    return;
  }
  let examples = Path::new(&book).join("examples");
  let mut failures = Vec::new();
  let mut built = 0;
  for (program, skip) in TTWJ_PROGRAMS {
    if let Some(reason) = skip {
      eprintln!("skip {program}: {reason}");
      continue;
    }
    let source = examples.join(program);
    let directory = tempfile::tempdir().expect("a temporary directory");
    let copy = directory
      .path()
      .join(source.file_name().expect("a file name"));
    std::fs::copy(&source, &copy).unwrap_or_else(|error| panic!("{}: {error}", source.display()));
    let assets = source.parent().expect("a chapter").join("assets");
    if assets.is_dir() {
      std::os::unix::fs::symlink(&assets, directory.path().join("assets"))
        .expect("a link to the assets");
    }
    match build_file(&copy) {
      Err(diagnostics) => failures.push(format!("{program} does not build:\n{diagnostics}")),
      Ok(executable) => {
        built += 1;
        if TTWJ_HEADLESS.contains(program) {
          let output = Command::new(&executable)
            .current_dir(directory.path())
            .env_remove("DISPLAY")
            .output()
            .expect("the program should run");
          if !output.status.success() {
            failures.push(format!(
              "{program} exited {:?}:\n{}",
              output.status.code(),
              String::from_utf8_lossy(&output.stderr)
            ));
          }
        }
      }
    }
  }
  assert!(failures.is_empty(), "{}", failures.join("\n"));
  eprintln!("{built} programs built");
}
