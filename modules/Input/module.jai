//
// Input: what the keyboard, the mouse and the window system did since the
// last frame, the same way on every platform.
//
// A frame calls `update_window_events` once, then reads what it gathered:
//
//     update_window_events();
//     for get_window_resizes()  if it.window == window  resize(it.width, it.height);
//     for events_this_frame {
//         if it.type == .QUIT  quit = true;
//         if it.type == .KEYBOARD && it.key_pressed && it.key_code == .ESCAPE  quit = true;
//         if it.type == .TEXT_INPUT  type_character(it.utf32);
//     }
//     if input_button_states[Key_Code.ARROW_LEFT] & .DOWN  move_left();
//
// `events_this_frame` holds one `Event` per thing that happened, in order:
// a key or mouse button going down or up (`.KEYBOARD`, with `key_pressed`
// and `key_code`), a character typed (`.TEXT_INPUT`, with `utf32`), the
// mouse wheel turning (`.MOUSE_WHEEL`, with `wheel_delta` in units of which
// one notch is `typical_wheel_delta`), a window asking to be closed
// (`.QUIT`), and a window changing size (`.WINDOW`, which
// `get_window_resizes` also reports). Every event says which modifier keys
// were held when it happened.
//
// `input_button_states` is the other way of asking: one `Key_Current_State`
// per key code, `.DOWN` while the key is held, with `.START` on the frame it
// went down and `.END` on the frame it came up — so a press and release
// within one frame still shows as both. A letter's key code is its
// upper-case ASCII value (`cast(Key_Code) #char "A"`), a digit's or a
// punctuation key's is the character itself, and every other key is named.
// `mouse_delta_x`, `mouse_delta_y` and `mouse_delta_z` (the wheel) are how
// far the mouse moved during the frame.
//
// On Linux the events come from X11. The connection is `X11`'s shared
// `x_global_display`, which `Window_Creation` opens when it makes the first
// window; until there is one, `update_window_events` has nothing to read.
// Typed text is what the key's keysym means once Shift and Caps Lock are
// applied — Latin-1 or a Unicode keysym — so a dead key or an input method's
// composition is not seen as text.
//

#import "Basic";
WT :: #import "Window_Type";

#scope_module
Window_Type :: WT.Window_Type;
#scope_export

Event_Type :: enum u32 {
    UNINITIALIZED       :: 0;
    KEYBOARD            :: 1;
    TEXT_INPUT          :: 2;
    WINDOW              :: 3;
    MOUSE_WHEEL         :: 4;
    QUIT                :: 5;
    DRAG_AND_DROP_FILES :: 6;
}

// The control keys keep their ASCII values; everything that has none comes
// after the ASCII range, and the whole set fits `NUM_BUTTON_STATES`.
Key_Code :: enum u32 {
    UNKNOWN     :: 0;
    BACKSPACE   :: 8;
    TAB         :: 9;
    LINEFEED    :: 10;
    ENTER       :: 13;
    ESCAPE      :: 27;
    SPACEBAR    :: 32;
    DELETE      :: 127;

    ARROW_UP    :: 128;
    ARROW_DOWN;
    ARROW_LEFT;
    ARROW_RIGHT;
    PAGE_UP;
    PAGE_DOWN;
    HOME;
    END;
    INSERT;
    PAUSE;
    SCROLL_LOCK;
    PRINT_SCREEN;
    CAPS_LOCK;
    NUM_LOCK;

    ALT;
    CTRL;
    SHIFT;
    CMD;
    META :: CMD;

    F1; F2; F3; F4; F5; F6; F7; F8; F9; F10; F11; F12;
    F13; F14; F15; F16; F17; F18; F19; F20; F21; F22; F23; F24;

    MOUSE_BUTTON_LEFT;
    MOUSE_BUTTON_MIDDLE;
    MOUSE_BUTTON_RIGHT;
    MOUSE_BUTTON_X1;
    MOUSE_BUTTON_X2;
    MOUSE_WHEEL_UP;
    MOUSE_WHEEL_DOWN;

    NUMPAD_0; NUMPAD_1; NUMPAD_2; NUMPAD_3; NUMPAD_4;
    NUMPAD_5; NUMPAD_6; NUMPAD_7; NUMPAD_8; NUMPAD_9;
    NUMPAD_MULTIPLY;
    NUMPAD_ADD;
    NUMPAD_SUBTRACT;
    NUMPAD_DECIMAL;
    NUMPAD_DIVIDE;
    NUMPAD_ENTER;
}

NUM_BUTTON_STATES :: 256;

Key_Current_State :: enum_flags u32 {
    NONE  :: 0x0;
    DOWN  :: 0x1;
    START :: 0x4;
    END   :: 0x8;
}

// Which modifier keys were held. `packed` is all four at once, for comparing
// two sets.
Event_Modifier_Flags :: union {
    struct {
        shift_pressed:    bool;
        ctrl_pressed:     bool;
        alt_pressed:      bool;
        cmd_meta_pressed: bool;
    }
    packed: u32;
}

Event :: struct {
    type: Event_Type;

    // `.KEYBOARD`: 1 when the key went down, 0 when it came up. A key held
    // long enough to repeat sends more presses with `repeat` set.
    key_pressed: u32;
    key_code := Key_Code.UNKNOWN;
    repeat: bool;

    using modifier_flags: Event_Modifier_Flags;

    // `.TEXT_INPUT`: the character typed, and how many `.TEXT_INPUT` events
    // this frame has had before it.
    utf32: u32;
    text_input_count: u16;

    // `.MOUSE_WHEEL`: positive away from the user.
    typical_wheel_delta: s32;
    wheel_delta: s32;

    // `.DRAG_AND_DROP_FILES`.
    files: [] string;

    // The window it happened to.
    window: Window_Type;
}

Window_Resize_Record :: struct {
    window: Window_Type;
    width:  s32;
    height: s32;
}

TYPICAL_WHEEL_DELTA :: 120;

events_this_frame: [..] Event;
input_button_states: [NUM_BUTTON_STATES] Key_Current_State;

mouse_delta_x: int;
mouse_delta_y: int;
mouse_delta_z: int;

input_application_has_focus := false;

// Resizes that `get_window_resizes` has not handed out yet.
pending_resizes: [..] Window_Resize_Record;

// The resizes since the last call, one per window with its latest size, in
// temporary storage.
get_window_resizes :: () -> [] Window_Resize_Record {
    if !pending_resizes.count  return .[];
    result := array_copy(pending_resizes, temp);
    array_reset_keeping_memory(*pending_resizes);
    return result;
}

// Starts a new frame: forgets last frame's events and deltas, and turns
// `.START` and `.END` into plain held and plain released. `update_window_events`
// calls it first.
input_per_frame_event_and_flag_update :: () {
    array_reset_keeping_memory(*events_this_frame);
    mouse_delta_x = 0;
    mouse_delta_y = 0;
    mouse_delta_z = 0;
    text_inputs_this_frame = 0;
    for * input_button_states {
        if it.* & .END  it.* = .NONE;
        else            it.* &= ~Key_Current_State.START;
    }
}

// Records a key or button going down or up, both as an event and in
// `input_button_states`. The platform layer calls this; so may a program
// that synthesizes input.
input_key_event :: (key_code: Key_Code, pressed: bool, modifiers: Event_Modifier_Flags, window: Window_Type = 0) {
    index := cast(s64) key_code;
    event: Event;
    event.type = .KEYBOARD;
    event.key_code = key_code;
    event.key_pressed = xx ifx pressed then 1 else 0;
    event.modifier_flags = modifiers;
    event.window = window;
    if index >= 0 && index < NUM_BUTTON_STATES {
        state := *input_button_states[index];
        if pressed {
            event.repeat = (state.* & .DOWN) == .DOWN && (state.* & .END) != .END;
            if !event.repeat  state.* = .DOWN | .START;
        } else {
            state.* |= .END;
        }
    }
    array_add(*events_this_frame, event);
}

input_text_event :: (utf32: u32, modifiers: Event_Modifier_Flags, window: Window_Type = 0) {
    event: Event;
    event.type = .TEXT_INPUT;
    event.utf32 = utf32;
    event.text_input_count = text_inputs_this_frame;
    event.modifier_flags = modifiers;
    event.window = window;
    text_inputs_this_frame += 1;
    array_add(*events_this_frame, event);
}

input_wheel_event :: (delta: s32, modifiers: Event_Modifier_Flags, window: Window_Type = 0) {
    event: Event;
    event.type = .MOUSE_WHEEL;
    event.typical_wheel_delta = TYPICAL_WHEEL_DELTA;
    event.wheel_delta = delta;
    event.modifier_flags = modifiers;
    event.window = window;
    mouse_delta_z += delta;
    array_add(*events_this_frame, event);
}

input_quit_event :: (window: Window_Type = 0) {
    event: Event;
    event.type = .QUIT;
    event.window = window;
    array_add(*events_this_frame, event);
}

// Records that `window` is now `width` by `height`. A second resize of the
// same window before `get_window_resizes` replaces the first.
input_resize_event :: (window: Window_Type, width: s32, height: s32) {
    found := false;
    for * pending_resizes  if it.window == window {
        it.width = width;
        it.height = height;
        found = true;
    }
    if !found  array_add(*pending_resizes, .{window, width, height});

    event: Event;
    event.type = .WINDOW;
    event.window = window;
    array_add(*events_this_frame, event);
}

// Every held key comes up: what a program sees when it loses the keyboard,
// since the release will go to another window.
input_release_all_keys :: () {
    for * input_button_states  if it.* & .DOWN  it.* |= .END;
}

#scope_module

text_inputs_this_frame: u16;

#scope_export

#if OS == .LINUX {
    #load "linux.jai";
} else {
    update_window_events :: () {
        input_per_frame_event_and_flag_update();
    }
}
