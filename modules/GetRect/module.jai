//
// GetRect: immediate-mode widgets — buttons, sliders and dropdowns — drawn
// with Simp into the window Window_Creation made, and driven by the events
// Input reports.
//
//     ui_init();
//     while running {
//         update_window_events();
//         for events_this_frame  getrect_handle_event(it);
//         clear_render_target(.2, .2, .2, 1);
//         width, height := get_render_dimensions(window);
//         ui_per_frame_update(window, width, height, get_time());
//
//         r := get_rect(10, xx height - 40, 200, 30);
//         if button(r, "Press me")  print("pressed\n");
//         r.y -= 35;
//         slider(r, *volume, 0.0, 1.0, 0.05, prefix = "Volume ");
//         r.y -= 35;
//         dropdown(r, .["Red", "Green", "Blue"], *colour);
//
//         draw_popups();
//         swap_buffers(window);
//     }
//
// There is no retained tree of widgets: each call draws its widget and says
// what happened to it this frame. A widget is told apart from the others by
// where it was called from (`#caller_location`) and an `identifier` the
// caller may add, which is what a loop that makes several of them from one
// line passes.
//
// Rects are in the render target's pixels, in Simp's coordinate system: with
// the default right-handed one `y` is the bottom edge and goes up, and a
// dropdown's list opens below it, towards smaller `y`.
//
// What a widget looks like is its theme. Every widget takes a pointer to one
// and falls back to the theme `set_default_theme` made current; the themes
// `default_theme_procs` makes are a starting point to change. Text is drawn
// in a font ui_init finds on the system — `fc-match`'s sans-serif, or a
// common path — and a program that wants another sets
// `Overall_Theme.font_path` before `set_default_theme`. With no font at all
// the widgets work and draw, without their text.
//

#import "Basic";
#import "Math";
#import "String";
#import "Simp";
#import "Window_Creation";
Input :: #import "Input";
WT :: #import "Window_Type";
#import "Process";
#import "File_Utilities";

#scope_export

Rect :: struct {
    x, y, w, h: float;
}

get_rect :: (x: float, y: float, w: float, h: float) -> Rect {
    r: Rect = ---;
    r.x = x;
    r.y = y;
    r.w = w;
    r.h = h;
    return r;
}

Text_Alignment :: enum u8 {
    Center;
    Left;
    Right;
}

Label_Theme :: struct {
    text_color := Vector4.{.92, .92, .92, 1};
    alignment  := Text_Alignment.Center;

    // How tall the text is, as a fraction of the rect's height, and how far
    // from the edge it is kept when it is not centered, as a fraction of the
    // text's height.
    text_height := 0.55;
    padding     := 0.5;
}

Button_Theme :: struct {
    label_theme: Label_Theme;

    surface_color      := Vector4.{.20, .28, .38, 1};
    surface_color_over := Vector4.{.26, .36, .48, 1};
    surface_color_down := Vector4.{.14, .20, .28, 1};
    frame_color        := Vector4.{.55, .65, .75, 1};
    frame_color_over   := Vector4.{.85, .90, .95, 1};

    // As a fraction of the rect's height.
    frame_thickness := 0.06;
}

Slider_Theme :: struct {
    background: Button_Theme;
    foreground: Label_Theme;

    fill_color      := Vector4.{.30, .48, .66, 1};
    fill_color_over := Vector4.{.38, .58, .78, 1};
}

Dropdown_Theme :: struct {
    button_theme: Button_Theme;
    item_theme: Button_Theme;

    current_item_color := Vector4.{.30, .48, .66, 1};
    arrow_color        := Vector4.{.92, .92, .92, 1};
}

Overall_Theme :: struct {
    button_theme:   Button_Theme;
    slider_theme:   Slider_Theme;
    dropdown_theme: Dropdown_Theme;

    // A font file to draw text with; empty means the one ui_init found.
    font_path: string;
}

Default_Themes :: enum u8 {
    Default;
    Grayscale;
    High_Contrast;
}

default_theme_procs :: (() -> Overall_Theme).[default_theme, grayscale_theme, high_contrast_theme];

default_theme :: () -> Overall_Theme {
    theme: Overall_Theme;
    theme.dropdown_theme.item_theme.surface_color = .{.16, .22, .30, 1};
    return theme;
}

grayscale_theme :: () -> Overall_Theme {
    theme: Overall_Theme;
    gray :: (v: float) -> Vector4 { return .{v, v, v, 1}; }
    set_button_colors :: (button: *Button_Theme, surface: float) {
        button.surface_color      = gray(surface);
        button.surface_color_over = gray(surface + .08);
        button.surface_color_down = gray(surface - .06);
        button.frame_color        = gray(.62);
        button.frame_color_over   = gray(.90);
        button.label_theme.text_color = gray(.95);
    }
    set_button_colors(*theme.button_theme, .30);
    set_button_colors(*theme.slider_theme.background, .22);
    theme.slider_theme.foreground.text_color = gray(.95);
    theme.slider_theme.fill_color      = gray(.45);
    theme.slider_theme.fill_color_over = gray(.55);
    set_button_colors(*theme.dropdown_theme.button_theme, .30);
    set_button_colors(*theme.dropdown_theme.item_theme, .24);
    theme.dropdown_theme.current_item_color = gray(.45);
    theme.dropdown_theme.arrow_color = gray(.95);
    return theme;
}

high_contrast_theme :: () -> Overall_Theme {
    theme: Overall_Theme;
    set_button_colors :: (button: *Button_Theme) {
        button.surface_color      = .{0, 0, 0, 1};
        button.surface_color_over = .{.15, .15, 0, 1};
        button.surface_color_down = .{.3, .3, 0, 1};
        button.frame_color        = .{1, 1, 1, 1};
        button.frame_color_over   = .{1, 1, 0, 1};
        button.label_theme.text_color = .{1, 1, 1, 1};
    }
    set_button_colors(*theme.button_theme);
    set_button_colors(*theme.slider_theme.background);
    theme.slider_theme.foreground.text_color = .{1, 1, 1, 1};
    theme.slider_theme.fill_color      = .{.45, .45, 0, 1};
    theme.slider_theme.fill_color_over = .{.6, .6, 0, 1};
    set_button_colors(*theme.dropdown_theme.button_theme);
    set_button_colors(*theme.dropdown_theme.item_theme);
    theme.dropdown_theme.current_item_color = .{.45, .45, 0, 1};
    theme.dropdown_theme.arrow_color = .{1, 1, 0, 1};
    return theme;
}

set_default_theme :: (theme: Overall_Theme) {
    ui.theme = theme;
}

// Finds a font and makes the default theme current. Call it once, after the
// window exists.
ui_init :: () {
    ui.theme = default_theme();
    look_for_font();
}

// Hands an event from Input to the widgets. Only the left mouse button means
// anything to them.
getrect_handle_event :: (event: Input.Event) {
    if event.type != .KEYBOARD || event.key_code != .MOUSE_BUTTON_LEFT  return;
    if event.key_pressed {
        if !event.repeat  ui.pending_press = true;
        ui.pending_down = true;
    } else {
        ui.pending_release = true;
        ui.pending_down = false;
    }
}

// Starts a frame of widgets: where the mouse is, and what its button did
// since the last frame.
ui_per_frame_update :: (window: WT.Window_Type, width: s32, height: s32, current_time: float64) {
    ui.window = window;
    ui.width = width;
    ui.height = height;
    ui.time = current_time;

    x, y, success := get_mouse_pointer_position(window, right_handed());
    if success {
        ui.mouse_x = cast(float) x;
        ui.mouse_y = cast(float) y;
    }
    ui_set_mouse_button(ui.pending_press, ui.pending_release, ui.pending_down);

    array_reset_keeping_memory(*ui.blocking);
    for ui.popups  array_add(*ui.blocking, it.area);
    array_reset_keeping_memory(*ui.popups);
}

// The mouse as the widgets see it this frame, for a program that reads the
// pointer itself or has no window.
ui_set_mouse :: (x: float, y: float) {
    ui.mouse_x = x;
    ui.mouse_y = y;
}

// Whether the pointer is over a dropdown list that is open, which a program
// that also reads the mouse may want to leave alone.
ui_mouse_is_over_popup :: () -> bool {
    for ui.blocking  if contains(it, ui.mouse_x, ui.mouse_y)  return true;
    return false;
}

// A push button: true on the frame the mouse button is released over it,
// having gone down over it.
button :: (r: Rect, text: string, theme: *Button_Theme = null, identifier: s64 = 0, loc := #caller_location) -> bool {
    if !theme  theme = *ui.theme.button_theme;
    id := widget_id(loc, identifier);
    over := mouse_over(r);
    clicked, down := press_and_release(id, over);
    draw_button(r, theme, over, down);
    draw_label(r, text, *theme.label_theme);
    return clicked;
}

// A horizontal slider over `value`, from `min_value` to `max_value` in
// steps of `step`. Dragging anywhere along it sets the value; the text is
// `prefix`, the value and `suffix`. True on a frame the value changed.
slider :: (r: Rect, value: *$T, min_value: T, max_value: T, step: T, theme: *Slider_Theme = null, prefix := "", suffix := "", identifier: s64 = 0, loc := #caller_location) -> bool {
    if !theme  theme = *ui.theme.slider_theme;
    id := widget_id(loc, identifier);
    over := mouse_over(r);
    if over && ui.pressed  ui.active = id;
    held := ui.active == id;
    changed := false;
    if held && max_value > min_value && r.w > 0 {
        t := clamp((ui.mouse_x - r.x) / r.w, 0, 1);
        range := cast(float64) (max_value - min_value);
        wanted := cast(float64) min_value + t * range;
        if step > 0 {
            steps := floor((wanted - cast(float64) min_value) / cast(float64) step + 0.5);
            wanted = cast(float64) min_value + steps * cast(float64) step;
        }
        wanted = clamp(wanted, cast(float64) min_value, cast(float64) max_value);
        #if type_info(T).type == .INTEGER  new_value := cast(T) floor(wanted + 0.5);
        else                               new_value := cast(T) wanted;
        if new_value != value.*  {
            value.* = new_value;
            changed = true;
        }
    }
    if held && ui.released  ui.active = 0;

    draw_button(r, *theme.background, over || held, false);
    fraction := 0.0;
    if max_value > min_value  fraction = clamp(cast(float) (value.* - min_value) / cast(float) (max_value - min_value), 0, 1);
    inset := frame_width(r, *theme.background);
    if fraction > 0 {
        fill := ifx over || held then theme.fill_color_over else theme.fill_color;
        quad(r.x + inset, r.y + inset, r.x + inset + (r.w - 2 * inset) * fraction, r.y + r.h - inset, fill);
    }

    #if type_info(T).type == .INTEGER {
        text := tprint("%1%2%3", prefix, value.*, suffix);
    } else {
        text := tprint("%1%2%3", prefix, formatFloat(value.*, trailing_width = decimals_of(cast(float64) step), zero_removal = .NO), suffix);
    }
    draw_label(r, text, *theme.foreground);
    return changed;
}

Dropdown_State :: struct {
    open: bool;
    // Which item the pointer is over while the list is open, or -1.
    hovered := -1;
}

// A button showing `choices[current.*]` that opens a list of the choices
// below it; clicking one sets `current` to its index. The list is drawn by
// draw_popups, so that it lies over what was drawn after the dropdown.
dropdown :: (r: Rect, choices: [] string, current: *$T, theme: *Dropdown_Theme = null, identifier: s64 = 0, loc := #caller_location) -> *Dropdown_State {
    if !theme  theme = *ui.theme.dropdown_theme;
    id := widget_id(loc, identifier);
    state := dropdown_state(id);

    over := mouse_over_ignoring_popups(r);
    clicked := press_and_release_ignoring_popups(id, over);
    if clicked  state.open = !state.open;

    state.hovered = -1;
    area := r;
    if state.open && choices.count {
        area = list_area(r, choices.count);
        for 0..choices.count - 1 {
            item := item_rect(r, it);
            if contains(item, ui.mouse_x, ui.mouse_y)  state.hovered = it;
        }
        if ui.released && !clicked {
            if state.hovered >= 0  current.* = cast(T) state.hovered;
            state.open = false;
        }
    }

    draw_button(r, *theme.button_theme, over, false);
    index := cast(s64) current.*;
    if index >= 0 && index < choices.count  draw_label(get_rect(r.x, r.y, r.w - r.h, r.h), choices[index], *theme.button_theme.label_theme);
    draw_arrow(r, theme.arrow_color, state.open);

    if state.open {
        popup: Popup;
        popup.owner = r;
        popup.area = area;
        popup.choices = choices;
        popup.current = index;
        popup.hovered = state.hovered;
        popup.theme = theme;
        array_add(*ui.popups, popup);
    }
    return state;
}

// Draws the lists of the dropdowns that are open. Call it after every
// other widget of the frame.
draw_popups :: () {
    for popup: ui.popups {
        theme := popup.theme;
        for popup.choices {
            item := item_rect(popup.owner, it_index);
            draw_button(item, *theme.item_theme, it_index == popup.hovered, false);
            if it_index == popup.current {
                inset := frame_width(item, *theme.item_theme);
                quad(item.x + inset, item.y + inset, item.x + inset + item.h * .2, item.y + item.h - inset, theme.current_item_color);
            }
            draw_label(item, it, *theme.item_theme.label_theme);
        }
    }
}

#scope_module

Popup :: struct {
    owner: Rect;
    area: Rect;
    choices: [] string;
    current: s64;
    hovered: s64;
    theme: *Dropdown_Theme;
}

State :: struct {
    window: WT.Window_Type;
    width, height: s32;
    time: float64;

    theme: Overall_Theme;
    found_font_path: string;
    looked_for_font: bool;

    mouse_x, mouse_y: float;
    // What the left button did since the last frame, as getrect_handle_event
    // saw it, and what the widgets of this frame see.
    pending_press, pending_release, pending_down: bool;
    pressed, released, down: bool;

    // The widget the button went down on, while it is held.
    active: u64;

    popups: [..] Popup;
    // The lists that were open last frame, which nothing under them answers.
    blocking: [..] Rect;

    dropdown_ids: [..] u64;
    dropdown_states: [..] *Dropdown_State;
}

ui: State;

ui_set_mouse_button :: (pressed: bool, released: bool, down: bool) {
    ui.pressed = pressed;
    ui.released = released;
    ui.down = down;
    ui.pending_press = false;
    ui.pending_release = false;
}

widget_id :: (loc: Source_Code_Location, identifier: s64) -> u64 {
    hash: u64 = 14695981039346656037;
    mix :: (hash: *u64, value: u64) {
        hash.* = (hash.* ^ value) * 1099511628211;
    }
    for cast([] u8) loc.fully_pathed_filename  mix(*hash, it);
    mix(*hash, cast(u64) loc.line_number);
    mix(*hash, cast(u64) loc.character_number);
    mix(*hash, cast(u64) identifier);
    if hash == 0  hash = 1;
    return hash;
}

right_handed :: () -> bool {
    return !context.simp || context.simp.coordinate_system == .RIGHT_HANDED;
}

contains :: (r: Rect, x: float, y: float) -> bool {
    return x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h;
}

mouse_over :: (r: Rect) -> bool {
    return contains(r, ui.mouse_x, ui.mouse_y) && !ui_mouse_is_over_popup();
}

mouse_over_ignoring_popups :: (r: Rect) -> bool {
    return contains(r, ui.mouse_x, ui.mouse_y);
}

// Whether a click finished on the widget this frame, and whether it is being
// held down over it.
press_and_release :: (id: u64, over: bool) -> clicked: bool, down: bool {
    if over && ui.pressed  ui.active = id;
    held := ui.active == id;
    clicked := held && ui.released && over;
    if held && ui.released  ui.active = 0;
    return clicked, held && over && !ui.released;
}

press_and_release_ignoring_popups :: (id: u64, over: bool) -> bool {
    if over && ui.pressed  ui.active = id;
    held := ui.active == id;
    clicked := held && ui.released && over;
    if held && ui.released  ui.active = 0;
    return clicked;
}

dropdown_state :: (id: u64) -> *Dropdown_State {
    for ui.dropdown_ids  if it == id  return ui.dropdown_states[it_index];
    state := New(Dropdown_State);
    array_add(*ui.dropdown_ids, id);
    array_add(*ui.dropdown_states, state);
    return state;
}

// Where the i-th item of a list opened under `owner` is: below it, which is
// towards smaller `y` when `y` goes up.
item_rect :: (owner: Rect, index: s64) -> Rect {
    item := owner;
    step := owner.h * (index + 1);
    if right_handed()  item.y -= step;
    else                                                item.y += step;
    return item;
}

list_area :: (owner: Rect, count: s64) -> Rect {
    area := owner;
    area.h = owner.h * count;
    if right_handed()  area.y = owner.y - area.h;
    else                                                area.y = owner.y + owner.h;
    return area;
}

decimals_of :: (step: float64) -> s64 {
    if step <= 0  return 2;
    decimals := 0;
    scaled := step;
    while decimals < 6 && abs(scaled - floor(scaled + 0.5)) > 0.000001 {
        scaled *= 10;
        decimals += 1;
    }
    return decimals;
}

quad :: (x0: float, y0: float, x1: float, y1: float, color: Vector4) {
    set_shader_for_color(true);
    immediate_quad(x0, y0, x1, y1, color);
}

frame_width :: (r: Rect, theme: *Button_Theme) -> float {
    return max(1.0, cast(float) floor(r.h * theme.frame_thickness + 0.5));
}

draw_button :: (r: Rect, theme: *Button_Theme, over: bool, down: bool) {
    frame := ifx over then theme.frame_color_over else theme.frame_color;
    surface := theme.surface_color;
    if over  surface = theme.surface_color_over;
    if down  surface = theme.surface_color_down;
    inset := frame_width(r, theme);
    quad(r.x, r.y, r.x + r.w, r.y + r.h, frame);
    quad(r.x + inset, r.y + inset, r.x + r.w - inset, r.y + r.h - inset, surface);
}

draw_arrow :: (r: Rect, color: Vector4, open: bool) {
    size := r.h * .3;
    cx := r.x + r.w - r.h * .5;
    cy := r.y + r.h * .5;
    up := (right_handed()) == open;
    tip := ifx up then cy + size * .5 else cy - size * .5;
    base := ifx up then cy - size * .5 else cy + size * .5;
    set_shader_for_color(true);
    immediate_triangle(Vector2.{cx - size * .6, base}, Vector2.{cx + size * .6, base}, Vector2.{cx, tip}, color, color, color);
}

look_for_font :: () {
    if ui.looked_for_font  return;
    ui.looked_for_font = true;
    ui.found_font_path = find_system_font();
}

font_for :: (pixel_height: s64) -> *Dynamic_Font {
    look_for_font();
    path := ui.theme.font_path;
    if !path  path = ui.found_font_path;
    if !path  return null;
    return get_font_at_size(path_strip_filename(path), path_filename(path), max(pixel_height, 6));
}

draw_label :: (r: Rect, text: string, theme: *Label_Theme) {
    if !text  return;
    font := font_for(cast(s64) (r.h * theme.text_height + 0.5));
    if !font  return;
    width := prepare_text(font, text);
    pad := cast(s64) (font.character_height * theme.padding);
    x: s64;
    if theme.alignment == {
      case .Left;   x = cast(s64) r.x + pad;
      case .Right;  x = cast(s64) (r.x + r.w) - pad - width;
      case;         x = cast(s64) (r.x + (r.w - width) * .5);
    }
    // The capital letters are centered: the baseline is half their height
    // below the middle of the rect.
    middle := r.y + r.h * .5;
    y: s64;
    if right_handed()  y = cast(s64) (middle - font.typical_ascender * .5 + .5);
    else                                                y = cast(s64) (middle + font.typical_ascender * .5 + .5);
    draw_prepared_text(font, x, y, theme.text_color);
}

// Asks fontconfig for the sans-serif font, and looks in the usual places when
// there is no fontconfig.
find_system_font :: () -> string {
    result, output := run_command("fc-match", "-f", "%{file}", "sans", capture_and_return_output = true);
    path := trim(output);
    if result.type == .EXITED && result.exit_code == 0 && path && file_exists(path)  return copy_string(path);
    CANDIDATES :: string.[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    ];
    for CANDIDATES  if file_exists(it)  return it;
    return "";
}
