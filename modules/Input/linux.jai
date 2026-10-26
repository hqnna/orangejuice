//
// Input's X11 side: reads every event pending on `x_global_display` and turns
// the ones a program cares about into `Event`s.
//
// Key codes come from the key's unshifted keysym, so Shift+A is `A` held with
// Shift; text comes from the keysym `XLookupString` picks with the modifiers
// applied. Auto-repeat is made detectable, so a held key sends further
// presses without the releases X would otherwise interleave, and those are
// marked `repeat`. A window asking to close is the `WM_DELETE_WINDOW`
// protocol `Window_Creation` signs every window up for.
//

#scope_module

X :: #import "X11";

#scope_export

update_window_events :: () {
    input_per_frame_event_and_flag_update();

    display := X.x_global_display;
    if !display  return;

    if !x_input.initialized {
        x_input.initialized = true;
        x_input.wm_protocols = X.XInternAtom(display, "WM_PROTOCOLS", X.False);
        x_input.wm_delete_window = X.XInternAtom(display, "WM_DELETE_WINDOW", X.False);
        supported: s32;
        X.XkbSetDetectableAutoRepeat(display, X.True, *supported);
    }

    while X.XPending(display) {
        event: X.XEvent;
        X.XNextEvent(display, *event);
        handle_x_event(display, *event);
    }
}

#scope_file

X_Input_State :: struct {
    initialized: bool;
    wm_protocols: X.Atom;
    wm_delete_window: X.Atom;

    have_pointer: bool;
    last_x: s32;
    last_y: s32;

    sizes: [..] Window_Resize_Record;
}

x_input: X_Input_State;

modifiers_from_state :: (state: u32) -> Event_Modifier_Flags {
    result: Event_Modifier_Flags;
    result.shift_pressed    = (state & X.ShiftMask) != 0;
    result.ctrl_pressed     = (state & X.ControlMask) != 0;
    result.alt_pressed      = (state & X.Mod1Mask) != 0;
    result.cmd_meta_pressed = (state & X.Mod4Mask) != 0;
    return result;
}

handle_x_event :: (display: *X.Display, event: *X.XEvent) {
    if event.type == {
      case X.KeyPress; #through;
      case X.KeyRelease;
        key := *event.xkey;
        pressed := event.type == X.KeyPress;
        modifiers := modifiers_from_state(key.state);
        code := key_code_from_keysym(X.XLookupKeysym(key, 0));
        if code == .UNKNOWN  code = key_code_from_keysym(X.XLookupKeysym(key, 1));
        input_key_event(code, pressed, modifiers, key.window);
        if pressed {
            buffer: [32] u8;
            keysym: X.KeySym;
            X.XLookupString(key, buffer.data, buffer.count, *keysym, null);
            character := codepoint_from_keysym(keysym);
            if character  input_text_event(character, modifiers, key.window);
        }

      case X.ButtonPress; #through;
      case X.ButtonRelease;
        button := *event.xbutton;
        pressed := event.type == X.ButtonPress;
        modifiers := modifiers_from_state(button.state);
        if button.button == {
          case 1; input_key_event(.MOUSE_BUTTON_LEFT, pressed, modifiers, button.window);
          case 2; input_key_event(.MOUSE_BUTTON_MIDDLE, pressed, modifiers, button.window);
          case 3; input_key_event(.MOUSE_BUTTON_RIGHT, pressed, modifiers, button.window);
          case 4; if pressed  input_wheel_event(TYPICAL_WHEEL_DELTA, modifiers, button.window);
          case 5; if pressed  input_wheel_event(-TYPICAL_WHEEL_DELTA, modifiers, button.window);
          case 8; input_key_event(.MOUSE_BUTTON_X1, pressed, modifiers, button.window);
          case 9; input_key_event(.MOUSE_BUTTON_X2, pressed, modifiers, button.window);
        }

      case X.MotionNotify;
        motion := *event.xmotion;
        if x_input.have_pointer {
            mouse_delta_x += motion.x - x_input.last_x;
            mouse_delta_y += motion.y - x_input.last_y;
        }
        x_input.have_pointer = true;
        x_input.last_x = motion.x;
        x_input.last_y = motion.y;

      case X.ConfigureNotify;
        configure := *event.xconfigure;
        known := false;
        for * x_input.sizes  if it.window == configure.window {
            known = true;
            if it.width != configure.width || it.height != configure.height {
                it.width = configure.width;
                it.height = configure.height;
                input_resize_event(configure.window, configure.width, configure.height);
            }
        }
        if !known {
            array_add(*x_input.sizes, .{configure.window, configure.width, configure.height});
            input_resize_event(configure.window, configure.width, configure.height);
        }

      case X.FocusIn;
        input_application_has_focus = true;

      case X.FocusOut;
        input_application_has_focus = false;
        input_release_all_keys();

      case X.ClientMessage;
        message := *event.xclient;
        if message.message_type == x_input.wm_protocols && cast(X.Atom) message.data.l[0] == x_input.wm_delete_window {
            input_quit_event(message.window);
        }
    }
}

// What a keysym is as text: the Latin-1 keysyms are their own codepoints,
// and a Unicode keysym is 0x0100_0000 plus its codepoint. Control characters
// are not text.
codepoint_from_keysym :: (keysym: X.KeySym) -> u32 {
    if keysym >= 0x20 && keysym <= 0x7e  return cast(u32) keysym;
    if keysym >= 0xa0 && keysym <= 0xff  return cast(u32) keysym;
    if (keysym & 0xff00_0000) == 0x0100_0000 {
        codepoint := cast(u32) (keysym & 0x00ff_ffff);
        if codepoint >= 0x20 && codepoint != 0x7f && codepoint <= 0x10_ffff  return codepoint;
    }
    if keysym == X.XK_KP_Space  return #char " ";
    if keysym >= X.XK_KP_0 && keysym <= X.XK_KP_9  return cast(u32) (#char "0" + (keysym - X.XK_KP_0));
    if keysym == X.XK_KP_Multiply  return #char "*";
    if keysym == X.XK_KP_Add       return #char "+";
    if keysym == X.XK_KP_Subtract  return #char "-";
    if keysym == X.XK_KP_Decimal   return #char ".";
    if keysym == X.XK_KP_Divide    return #char "/";
    return 0;
}

key_code_from_keysym :: (keysym: X.KeySym) -> Key_Code {
    if keysym >= X.XK_a && keysym <= X.XK_z  return cast(Key_Code) (keysym - X.XK_a + #char "A");
    if keysym >= 0x20 && keysym <= 0x7e      return cast(Key_Code) keysym;
    if keysym >= X.XK_F1 && keysym <= X.XK_F24  return cast(Key_Code) (cast(u64) Key_Code.F1 + (keysym - X.XK_F1));
    if keysym >= X.XK_KP_0 && keysym <= X.XK_KP_9  return cast(Key_Code) (cast(u64) Key_Code.NUMPAD_0 + (keysym - X.XK_KP_0));

    if keysym == {
      case X.XK_BackSpace;    return .BACKSPACE;
      case X.XK_Tab;          return .TAB;
      case X.XK_ISO_Left_Tab; return .TAB;
      case X.XK_Linefeed;     return .LINEFEED;
      case X.XK_Return;       return .ENTER;
      case X.XK_Escape;       return .ESCAPE;
      case X.XK_Delete;       return .DELETE;

      case X.XK_Up;           return .ARROW_UP;
      case X.XK_Down;         return .ARROW_DOWN;
      case X.XK_Left;         return .ARROW_LEFT;
      case X.XK_Right;        return .ARROW_RIGHT;
      case X.XK_Page_Up;      return .PAGE_UP;
      case X.XK_Page_Down;    return .PAGE_DOWN;
      case X.XK_Home;         return .HOME;
      case X.XK_End;          return .END;
      case X.XK_Insert;       return .INSERT;
      case X.XK_Pause;        return .PAUSE;
      case X.XK_Scroll_Lock;  return .SCROLL_LOCK;
      case X.XK_Print;        return .PRINT_SCREEN;
      case X.XK_Caps_Lock;    return .CAPS_LOCK;
      case X.XK_Num_Lock;     return .NUM_LOCK;

      case X.XK_Shift_L;      return .SHIFT;
      case X.XK_Shift_R;      return .SHIFT;
      case X.XK_Control_L;    return .CTRL;
      case X.XK_Control_R;    return .CTRL;
      case X.XK_Alt_L;        return .ALT;
      case X.XK_Alt_R;        return .ALT;
      case X.XK_ISO_Level3_Shift; return .ALT;
      case X.XK_Meta_L;       return .CMD;
      case X.XK_Meta_R;       return .CMD;
      case X.XK_Super_L;      return .CMD;
      case X.XK_Super_R;      return .CMD;

      case X.XK_KP_Insert;    return .NUMPAD_0;
      case X.XK_KP_End;       return .NUMPAD_1;
      case X.XK_KP_Down;      return .NUMPAD_2;
      case X.XK_KP_Page_Down; return .NUMPAD_3;
      case X.XK_KP_Left;      return .NUMPAD_4;
      case X.XK_KP_Begin;     return .NUMPAD_5;
      case X.XK_KP_Right;     return .NUMPAD_6;
      case X.XK_KP_Home;      return .NUMPAD_7;
      case X.XK_KP_Up;        return .NUMPAD_8;
      case X.XK_KP_Page_Up;   return .NUMPAD_9;
      case X.XK_KP_Multiply;  return .NUMPAD_MULTIPLY;
      case X.XK_KP_Add;       return .NUMPAD_ADD;
      case X.XK_KP_Subtract;  return .NUMPAD_SUBTRACT;
      case X.XK_KP_Decimal;   return .NUMPAD_DECIMAL;
      case X.XK_KP_Delete;    return .NUMPAD_DECIMAL;
      case X.XK_KP_Divide;    return .NUMPAD_DIVIDE;
      case X.XK_KP_Enter;     return .NUMPAD_ENTER;
    }
    return .UNKNOWN;
}
