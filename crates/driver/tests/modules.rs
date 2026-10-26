//! The distribution modules a graphical program leans on: `stb_image`,
//! `stb_image_write`, `stb_image_resize`, `stb_vorbis`, `freetype`, `X11`,
//! `GL`, `Clipboard`, `Unmapping_Allocator` and `Sound_Player`.
//!
//! Each is built against orangejuice's own modules and run. The bindings are
//! generated from the system's C headers, so what a header fixes — a struct's
//! size, a member's offset, a constant's value — is measured against a C
//! program compiled from those same headers rather than written down here.
//! What needs a display runs on an `Xvfb` of its own when the dev shell
//! provides one, and the `GL` context falls back to `$DISPLAY`; with neither,
//! that part says so and skips, and the rest still runs.

use std::collections::BTreeMap;
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

/// What `pkg-config` says about `packages`, or `None` when it does not know
/// one of them.
fn pkg_config(arguments: &[&str], packages: &[&str]) -> Option<Vec<String>> {
  let output = Command::new("pkg-config")
    .args(arguments)
    .args(packages)
    .output()
    .ok()?;
  if !output.status.success() {
    return None;
  }
  Some(
    String::from_utf8_lossy(&output.stdout)
      .split_whitespace()
      .map(String::from)
      .collect(),
  )
}

/// Whether the system has the libraries a module binds, saying why not when
/// it does not.
fn system_has(packages: &[&str]) -> bool {
  if pkg_config(&["--exists"], packages).is_some() {
    return true;
  }
  if oj_testsupport::linker_required() {
    panic!("pkg-config does not know {packages:?}, and this run requires everything to link");
  }
  eprintln!("skipping: pkg-config does not know {packages:?}");
  false
}

/// Where the shared libraries of `packages` are, for a program to find them
/// at run time.
fn library_path(packages: &[&str]) -> String {
  let mut directories: Vec<String> = Vec::new();
  for package in packages {
    if let Some(found) = pkg_config(&["--variable=libdir"], &[package]) {
      directories.extend(found);
    }
  }
  if let Ok(existing) = std::env::var("LD_LIBRARY_PATH") {
    directories.push(existing);
  }
  directories.join(":")
}

struct Built {
  _directory: tempfile::TempDir,
  executable: PathBuf,
}

/// Builds `body` against orangejuice's own modules.
fn build(body: &str) -> Option<Built> {
  if !linker_is_available() {
    oj_testsupport::skip_without_linker("no C driver on PATH to link with");
    return None;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, body).expect("the input should be writable");
  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let options = oj_driver::BuildOptions::new();
  let report = oj_driver::run(&path, &options, oj_driver::Stage::Executable, None);
  assert!(
    !report.failed,
    "the program should build against our own modules, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report.executable.expect("a successful build has one");
  Some(Built {
    _directory: directory,
    executable,
  })
}

fn run(built: &Built, arguments: &[&str], environment: &[(&str, String)]) -> Output {
  let mut command = Command::new(&built.executable);
  command.args(arguments);
  for (name, value) in environment {
    command.env(name, value);
  }
  command.output().expect("the produced program should run")
}

fn run_ok(built: &Built, arguments: &[&str], environment: &[(&str, String)]) -> String {
  let output = run(built, arguments, environment);
  assert!(
    output.status.success(),
    "the program should run, but exited {:?}:\n{}{}",
    output.status.code(),
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Compiles and runs a C program against the headers of `packages`.
fn measure_in_c(source: &str, packages: &[&str], environment: &[(&str, String)]) -> String {
  let directory = tempfile::tempdir().expect("a temporary directory");
  let c_path = directory.path().join("measure.c");
  std::fs::write(&c_path, source).expect("the C source is writable");
  let measure = directory.path().join("measure");
  let flags = pkg_config(&["--cflags", "--libs"], packages).unwrap_or_default();
  let built = Command::new(oj_link::driver(oj_types::Target::HOST))
    .arg("-o")
    .arg(&measure)
    .arg(&c_path)
    .args(&flags)
    .output()
    .expect("the C driver should run");
  assert!(
    built.status.success(),
    "the measuring program should compile, but:\n{}",
    String::from_utf8_lossy(&built.stderr)
  );
  let output = Command::new(&measure)
    .env("LD_LIBRARY_PATH", library_path(packages))
    .envs(
      environment
        .iter()
        .map(|(name, value)| (*name, value.as_str())),
    )
    .output()
    .expect("the measuring program should run");
  assert!(output.status.success(), "the measuring program should run");
  String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Every `name value` line C printed has to be one Jai printed too.
fn assert_same_facts(from_c: &str, from_jai: &str, at_least: usize) {
  assert!(
    from_c.lines().count() >= at_least,
    "the C side measured too little:\n{from_c}"
  );
  let ours: BTreeMap<&str, &str> = from_jai
    .lines()
    .filter_map(|line| line.split_once(' '))
    .collect();
  let mut wrong = Vec::new();
  for line in from_c.lines() {
    let Some((name, theirs)) = line.split_once(' ') else {
      continue;
    };
    match ours.get(name) {
      Some(mine) if *mine == theirs => {}
      Some(mine) => wrong.push(format!("  {name}: the system says {theirs}, we say {mine}")),
      None => wrong.push(format!("  {name}: the system says {theirs}, we do not say")),
    }
  }
  assert!(
    wrong.is_empty(),
    "{} of {} facts do not match the system headers:\n{}",
    wrong.len(),
    from_c.lines().count(),
    wrong.join("\n")
  );
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

fn host_triple() -> &'static str {
  match (std::env::consts::ARCH, std::env::consts::OS) {
    ("aarch64", "macos") => "aarch64-darwin",
    ("aarch64", _) => "aarch64-linux",
    _ => "x86_64-linux",
  }
}

const STB_PROGRAM: &str = r#"
#import "Basic";
#import "stb_image";
#import "stb_image_write";
#import "stb_image_resize";

W :: 5;
H :: 3;

Collected :: struct {
    bytes: [4096] u8;
    count: s64;
}

collect :: (user: *void, data: *void, size: s32) #c_call {
    collected := cast(*Collected) user;
    for 0..size - 1 {
        if collected.count < collected.bytes.count  collected.bytes[collected.count] = (cast(*u8) data)[it];
        collected.count += 1;
    }
}

main :: () {
    directory := get_command_line_arguments()[1];
    pixels: [W * H * 4] u8;
    for y: 0..H - 1  for x: 0..W - 1 {
        p := (y * W + x) * 4;
        pixels[p] = cast(u8) (x * 50);
        pixels[p + 1] = cast(u8) (y * 120);
        pixels[p + 2] = cast(u8) (x * y * 9);
        pixels[p + 3] = cast(u8) (255 - x);
    }

    for kind: string.["png", "bmp", "tga"] {
        path := tprint("%/out.%\0", directory, kind);
        written: s32;
        if kind == "png"  written = stbi_write_png(path.data, W, H, 4, pixels.data, W * 4);
        if kind == "bmp"  written = stbi_write_bmp(path.data, W, H, 4, pixels.data);
        if kind == "tga"  written = stbi_write_tga(path.data, W, H, 4, pixels.data);
        w, h, n: s32;
        info := stbi_info(path.data, *w, *h, *n);
        data := stbi_load(path.data, *w, *h, *n, STBI_rgb_alpha);
        same := data != null;
        if data  for 0..W * H * 4 - 1  if data[it] != pixels[it]  same = false;
        print("% written % info % size %x% same %\n", kind, written, info, w, h, same);
        stbi_image_free(data);
    }

    collected: Collected;
    written := stbi_write_png_to_func(collect, *collected, W, H, 4, pixels.data, W * 4);
    w, h, n: s32;
    data := stbi_load_from_memory(collected.bytes.data, cast(s32) collected.count, *w, *h, *n, 0);
    same := data != null && n == 4;
    if same  for 0..W * H * 4 - 1  if data[it] != pixels[it]  same = false;
    print("memory written % png % size %x% channels % same %\n", written, collected.count > 8 && collected.count <= collected.bytes.count && collected.bytes[1] == #char "P", w, h, n, same);
    stbi_image_free(data);

    flat: [8 * 8 * 4] u8;
    for 0..8 * 8 - 1 {
        flat[it * 4] = 200; flat[it * 4 + 1] = 100; flat[it * 4 + 2] = 50; flat[it * 4 + 3] = 255;
    }
    small: [3 * 3 * 4] u8;
    resized := stbir_resize_uint8(flat.data, 8, 8, 0, small.data, 3, 3, 0, 4);
    print("resized % to % % % %\n", resized, small[16], small[17], small[18], small[19]);

    missing := stbi_load("/nonexistent/image.png", *w, *h, *n, 4);
    print("missing % reason %\n", missing == null, to_string(stbi_failure_reason()));
}
"#;

/// The stb libraries are compiled from the C the modules vendor, the first
/// time a program imports them, and what one writes the other reads back
/// pixel for pixel — through a file in three formats, and through the
/// callback writer and the memory loader.
#[test]
fn stb_image_reads_back_what_stb_image_write_wrote() {
  let Some(built) = build(STB_PROGRAM) else {
    return;
  };
  let directory = tempfile::tempdir().expect("a temporary directory");
  let output = run_ok(&built, &[&directory.path().display().to_string()], &[]);
  assert_eq!(
    output,
    "png written 1 info 1 size 5x3 same true\n\
     bmp written 1 info 1 size 5x3 same true\n\
     tga written 1 info 1 size 5x3 same true\n\
     memory written 1 png true size 5x3 channels 4 same true\n\
     resized 1 to 200 100 50 255\n\
     missing true reason can't fopen\n"
  );
  for name in ["stb_image", "stb_image_write", "stb_image_resize"] {
    let archive = oj_testsupport::modules()
      .join(name)
      .join(".build")
      .join(host_triple())
      .join(format!("{name}.a"));
    assert!(
      archive.is_file(),
      "{} should have been built",
      archive.display()
    );
  }
}

const SOUND_PROGRAM: &str = r##"#import "Basic";
#import "Math";
#import "Sound_Player";
#import "stb_vorbis";

pcm :: (samples: [] s16, channels: u16, rate: u32) -> *Mixer_Sound_Data {
    data := New(Mixer_Sound_Data);
    data.type = .LINEAR_SAMPLE_ARRAY;
    data.samples = samples.data;
    data.nsamples_times_nchannels = samples.count;
    data.nchannels = channels;
    data.sampling_rate = rate;
    return data;
}

mix_frames :: (player: *Sound_Player, frames: int) -> [] s16 {
    output := NewArray(frames * 2, s16);
    mix_streams(player, output);
    return output;
}

// An IMA ADPCM encoder, the mirror of the decoder: what it writes is what a
// WAV file's `data` chunk holds for one channel.
encode_ima :: (samples: [] s16, per_block: int) -> [..] u8, [..] s16 {
    STEPS :: s32.[
        7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45,
        50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230,
        253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963,
        1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327,
        3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442,
        11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
        32767,
    ];
    CHANGE :: s32.[-1, -1, -1, -1, 2, 4, 6, 8];
    out: [..] u8;
    heard: [..] s16;
    at := 0;
    while at < samples.count {
        predictor := cast(s32) samples[at];
        index: s32 = 0;
        array_add(*out, cast(u8) (predictor & 0xFF), cast(u8) ((predictor >> 8) & 0xFF), cast(u8) index, 0);
        array_add(*heard, cast(s16) predictor);
        codes: [..] u8;
        for i: 1..per_block - 1 {
            target := ifx at + i < samples.count then cast(s32) samples[at + i] else predictor;
            step := STEPS[index];
            difference := target - predictor;
            code: u8 = 0;
            if difference < 0 { code = 8; difference = -difference; }
            delta := step >> 3;
            if difference >= step { code |= 4; difference -= step; delta += step; }
            if difference >= step >> 1 { code |= 2; difference -= step >> 1; delta += step >> 1; }
            if difference >= step >> 2 { code |= 1; delta += step >> 2; }
            if code & 8 predictor -= delta; else predictor += delta;
            predictor = clamp(predictor, -32768, 32767);
            index = clamp(index + CHANGE[code & 7], 0, 88);
            array_add(*codes, code);
            array_add(*heard, cast(s16) predictor);
        }
        for i: 0..(codes.count / 2) - 1  array_add(*out, codes[i * 2] | (codes[i * 2 + 1] << 4));
        at += per_block;
    }
    return out, heard;
}

main :: () {
    player := New(Sound_Player);

    // A mono ramp at half the mix rate is played on both channels, twice as
    // slowly, at the volume asked for.
    ramp: [8] s16;
    for 0..7  ramp[it] = cast(s16) (it * 1000);
    stream := make_stream(player, pcm(ramp, 1, 22050));
    stream.user_volume_scale = 0.5;
    out := mix_frames(player, 6);
    print("mono %\n", out);

    // It runs out and stops, and the next update lets go of it.
    out = mix_frames(player, 20);
    print("ended % %\n", stream.flags & .STOPPED != 0, out[out.count - 1]);
    pre_entity_update(player);
    for player.streams  it.marked = true;
    post_entity_update(player, 0);
    print("released %\n", player.streams.count);

    // A repeating stereo sound goes back to its start at the repeat end.
    stereo := s16.[100, -100, 200, -200, 300, -300, 400, -400];
    looping := make_stream(player, pcm(stereo, 2, 44100), .MUSIC);
    looping.flags |= .REPEATING;
    looping.repeat_end_position = 3;
    out = mix_frames(player, 7);
    print("repeat %\n", out);

    // A category's level and the master volume scale it; faster plays fewer
    // frames of the sound per frame of the mix.
    player.category_volumes[Sound_Category.MUSIC] = 0.5;
    player.master_volume = 2;
    looping.desired_rate = 2;
    looping.play_cursor = 0;
    out = mix_frames(player, 2);
    print("levels %\n", out);
    player.master_volume = 1;

    // An unmarked stream is stopped by the update that finds it unmarked.
    pre_entity_update(player);
    post_entity_update(player, 0);
    print("unmarked %\n", player.streams.count);

    // The history keeps what was mixed last.
    player.update_history = true;
    make_stream(player, pcm(ramp, 1, 44100));
    mix_frames(player, 3);
    print("history %\n", player.history);

    // IMA ADPCM decodes to what the encoder was given, within its error.
    wave: [512] s16;
    for 0..wave.count - 1  wave[it] = cast(s16) (12000 * sin(it * 0.05));
    PER_BLOCK :: 129;
    encoded, heard := encode_ima(wave, PER_BLOCK);
    adpcm := New(Mixer_Sound_Data);
    adpcm.type = .ADPCM_COMPRESSED;
    adpcm.buffer = xx encoded;
    adpcm.samples = cast(*s16) encoded.data;
    adpcm.nchannels = 1;
    adpcm.sampling_rate = 44100;
    adpcm.nBlockAlign = 4 + (PER_BLOCK - 1) / 2;
    adpcm.wSamplesPerBlock = PER_BLOCK;
    adpcm.nsamples_times_nchannels = wave.count;
    frames := sound_length_in_frames(adpcm);
    same := frames == wave.count && heard.count >= frames;
    for 0..frames - 1  if adpcm.decoded[it] != heard[it]  same = false;
    // Once the step size has adapted, the codec follows the wave closely.
    worst := 0;
    for 32..frames - 1  if it % PER_BLOCK >= 32  worst = max(worst, abs(cast(int) adpcm.decoded[it] - wave[it]));
    print("adpcm % frames, as encoded %, close %\n", frames, same, worst < 200);

    // Bytes that are not Ogg Vorbis do not decode, and a stream of them stops.
    bad := New(Mixer_Sound_Data);
    bad.type = .OGG_COMPRESSED;
    bad.name = "bad";
    bad.buffer = "not an ogg file";
    pre_entity_update(player);
    post_entity_update(player, 0);
    broken := make_stream(player, bad);
    mix_frames(player, 4);
    print("bad ogg % %\n", bad.decode_failed, broken.flags & .STOPPED != 0);

    // Through ALSA's null device: the update writes what the device has room
    // for and the streams move on by that much.
    playback_device_name = "null";
    opened := init(player, 0);
    tone: [44100] s16;
    played := make_stream(player, pcm(tone, 1, 44100));
    pre_entity_update(player);
    played.marked = true;
    post_entity_update(player, 0);
    print("device % moved %\n", opened, played.play_cursor > 0);
    shutdown(player);
    print("shut down % %\n", player.streams.count, player.initted);

    args := get_command_line_arguments();
    if args.count > 1 {
        File :: #import "File";
        bytes, ok := File.read_entire_file(args[1]);
        ogg := New(Mixer_Sound_Data);
        ogg.type = .OGG_COMPRESSED;
        ogg.buffer = bytes;
        print("ogg % % % %\n", ok, sound_length_in_frames(ogg) > 44100, ogg.nchannels, ogg.sampling_rate);
    }
}
"##;

const SOUND_MIXED: &str = "mono [0, 0, 250, 250, 500, 500, 750, 750, 1000, 1000, 1250, 1250]\n\
  ended true 0\n\
  released 0\n\
  repeat [100, -100, 200, -200, 300, -300, 100, -100, 200, -200, 300, -300, 100, -100]\n\
  levels [100, -100, 300, -300]\n\
  unmarked 0\n\
  history [0, 0, 1000, 1000, 2000, 2000]\n\
  adpcm 512 frames, as encoded true, close true\n\
  bad ogg true true\n\
  device true moved true\n\
  shut down 0 false\n";

/// Sound_Player mixes without a device: a mono sound on both channels at the
/// rate and volume asked for, a stream that ends and is let go of, one that
/// repeats from its repeat end, category levels and the master volume, the
/// entity marking that keeps a stream alive, the history, IMA ADPCM decoded
/// to exactly what an encoder produced, and bytes that are not Ogg Vorbis.
/// Then through ALSA's `null` device, which takes frames and plays nothing.
/// With `OJ_TTWJ_DIR` set, the book's own `.ogg` decodes through stb_vorbis.
#[test]
fn sound_player_mixes_decodes_and_writes_to_a_device() {
  if !system_has(&["alsa"]) {
    return;
  }
  let Some(built) = build(SOUND_PROGRAM) else {
    return;
  };
  let output = run(&built, &[], &[]);
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert_eq!(String::from_utf8_lossy(&output.stdout), SOUND_MIXED);
  if let Some(book) = std::env::var_os("OJ_TTWJ_DIR") {
    let ogg = Path::new(&book).join("examples/33/assets/sounds/lux_aeterna.ogg");
    let output = run_ok(&built, &[&ogg.display().to_string()], &[]);
    assert!(output.ends_with("ogg true true 2 44100\n"), "{output}");
  }
}

const UNMAPPING_PROGRAM: &str = r#"
#import "Basic";
UA :: #import "Unmapping_Allocator";

main :: () {
    args := get_command_line_arguments();
    context.allocator = UA.get_unmapping_allocator();
    page := UA.page_size();

    a := cast(*u8) alloc(100);
    for 0..99  a[it] = cast(u8) it;
    end := cast(u64) (a + 112);
    print("block ends at the guard page: %\n", end % cast(u64) page == 0);

    b := cast(*u8) realloc(a, 5000, 100);
    print("resize moved: % kept: % %\n", a != b, b[0], b[99]);
    for 100..4999  b[it] = 1;

    numbers: [..] int;
    for 1..1000  array_add(*numbers, it);
    sum := 0;
    for numbers  sum += it;
    print("sum %\n", sum);
    array_free(numbers);

    text := sprint("% and %", "mapped", 42);
    print("%\n", text);
    free(text.data);
    free(b);

    if args.count > 1 && args[1] == "use_after_free" {
        print("touching freed memory\n");
        b[0] = 1;
        print("survived\n");
    }
    if args.count > 1 && args[1] == "overrun" {
        c := cast(*u8) alloc(64);
        print("writing past the end\n");
        c[64] = 1;
        print("survived\n");
    }
}
"#;

/// Allocation, resizing and freeing work through the context; a block ends
/// where its guard page starts, and touching it after `free`, or writing one
/// byte past its end, faults.
#[test]
fn the_unmapping_allocator_faults_on_use_after_free_and_overrun() {
  let Some(built) = build(UNMAPPING_PROGRAM) else {
    return;
  };
  let expected = "block ends at the guard page: true\n\
                  resize moved: true kept: 0 99\n\
                  sum 500500\n\
                  mapped and 42\n";
  assert_eq!(run_ok(&built, &[], &[]), expected);

  for (mode, said) in [
    ("use_after_free", "touching freed memory\n"),
    ("overrun", "writing past the end\n"),
  ] {
    let output = run(&built, &[mode], &[]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
      !output.status.success(),
      "{mode} should fault, but the program exited cleanly:\n{stdout}"
    );
    assert!(
      stdout.starts_with(expected) && stdout.contains(said),
      "{mode}:\n{stdout}"
    );
    assert!(
      !stdout.contains("survived"),
      "{mode} should not have survived:\n{stdout}"
    );
  }
}

const FREETYPE_C: &str = r#"
#include <stdio.h>
#include <stddef.h>
#include <ft2build.h>
#include FT_FREETYPE_H
#define SIZE(name, type)        printf("%s %zu\n", name, sizeof(type))
#define OFF(name, type, member) printf("%s %zu\n", name, offsetof(type, member))
#define VAL(name, expr)         printf("%s %lld\n", name, (long long)(expr))
int main(int argc, char **argv) {
  SIZE("FT_FaceRec", FT_FaceRec);
  SIZE("FT_GlyphSlotRec", FT_GlyphSlotRec);
  SIZE("FT_Bitmap", FT_Bitmap);
  SIZE("FT_Glyph_Metrics", FT_Glyph_Metrics);
  SIZE("FT_Size_Metrics", FT_Size_Metrics);
  SIZE("FT_SizeRec", FT_SizeRec);
  SIZE("FT_Vector", FT_Vector);
  SIZE("FT_Matrix", FT_Matrix);
  SIZE("FT_Outline", FT_Outline);
  SIZE("FT_Open_Args", FT_Open_Args);
  OFF("FT_FaceRec.num_glyphs", FT_FaceRec, num_glyphs);
  OFF("FT_FaceRec.units_per_EM", FT_FaceRec, units_per_EM);
  OFF("FT_FaceRec.glyph", FT_FaceRec, glyph);
  OFF("FT_FaceRec.size", FT_FaceRec, size);
  OFF("FT_FaceRec.charmap", FT_FaceRec, charmap);
  OFF("FT_GlyphSlotRec.metrics", FT_GlyphSlotRec, metrics);
  OFF("FT_GlyphSlotRec.advance", FT_GlyphSlotRec, advance);
  OFF("FT_GlyphSlotRec.format", FT_GlyphSlotRec, format);
  OFF("FT_GlyphSlotRec.bitmap", FT_GlyphSlotRec, bitmap);
  OFF("FT_GlyphSlotRec.bitmap_left", FT_GlyphSlotRec, bitmap_left);
  OFF("FT_GlyphSlotRec.outline", FT_GlyphSlotRec, outline);
  OFF("FT_Bitmap.pixel_mode", FT_Bitmap, pixel_mode);
  VAL("FT_LOAD_RENDER", FT_LOAD_RENDER);
  VAL("FT_LOAD_NO_BITMAP", FT_LOAD_NO_BITMAP);
  VAL("FT_LOAD_TARGET_LCD", FT_LOAD_TARGET_LCD);
  VAL("FT_LOAD_TARGET_LIGHT", FT_LOAD_TARGET_LIGHT);
  VAL("FT_FACE_FLAG_KERNING", FT_FACE_FLAG_KERNING);
  VAL("FT_Err_Unknown_File_Format", FT_Err_Unknown_File_Format);
  VAL("FT_PIXEL_MODE_GRAY", FT_PIXEL_MODE_GRAY);
  VAL("FT_PIXEL_MODE_LCD", FT_PIXEL_MODE_LCD);
  VAL("FT_RENDER_MODE_SDF", FT_RENDER_MODE_SDF);
  VAL("FT_KERNING_UNFITTED", FT_KERNING_UNFITTED);
  VAL("FT_ENCODING_UNICODE", FT_ENCODING_UNICODE);
  VAL("FT_GLYPH_FORMAT_OUTLINE", FT_GLYPH_FORMAT_OUTLINE);
  VAL("FREETYPE_MAJOR", FREETYPE_MAJOR);

  FT_Library library;
  if (FT_Init_FreeType(&library)) return 1;
  FT_Int major, minor, patch;
  FT_Library_Version(library, &major, &minor, &patch);
  VAL("version.major", major);
  VAL("version.minor", minor);
  VAL("version.patch", patch);
  FT_Face face;
  VAL("garbage", FT_New_Memory_Face(library, (const FT_Byte *)"not a font", 10, 0, &face));
  const char *font = getenv("OJ_TEST_FONT");
  if (font && !FT_New_Face(library, font, 0, &face)) {
    VAL("face.num_glyphs", face->num_glyphs);
    VAL("face.units_per_EM", face->units_per_EM);
    VAL("face.ascender", face->ascender);
    VAL("face.descender", face->descender);
    VAL("face.kerning", FT_HAS_KERNING(face) != 0);
    FT_Set_Pixel_Sizes(face, 0, 32);
    VAL("size.ascender", face->size->metrics.ascender);
    const char *text = "Ag";
    for (int i = 0; text[i]; i++) {
      FT_UInt index = FT_Get_Char_Index(face, text[i]);
      FT_Load_Glyph(face, index, FT_LOAD_RENDER);
      FT_GlyphSlot g = face->glyph;
      printf("glyph.%c %u %ld %ld %u %u %d %d %d\n", text[i], index, g->advance.x, g->metrics.horiBearingY,
             g->bitmap.rows, g->bitmap.width, g->bitmap_left, g->bitmap_top, g->bitmap.pixel_mode);
    }
    FT_Done_Face(face);
  }
  FT_Done_FreeType(library);
  return 0;
}
"#;

const FREETYPE_JAI: &str = r#"
#import "Basic";
#import "POSIX";
#import "freetype";

main :: () {
    print("FT_FaceRec %\n", size_of(FT_FaceRec));
    print("FT_GlyphSlotRec %\n", size_of(FT_GlyphSlotRec));
    print("FT_Bitmap %\n", size_of(FT_Bitmap));
    print("FT_Glyph_Metrics %\n", size_of(FT_Glyph_Metrics));
    print("FT_Size_Metrics %\n", size_of(FT_Size_Metrics));
    print("FT_SizeRec %\n", size_of(FT_SizeRec));
    print("FT_Vector %\n", size_of(FT_Vector));
    print("FT_Matrix %\n", size_of(FT_Matrix));
    print("FT_Outline %\n", size_of(FT_Outline));
    print("FT_Open_Args %\n", size_of(FT_Open_Args));
    face_rec: FT_FaceRec;
    slot: FT_GlyphSlotRec;
    bitmap: FT_Bitmap;
    print("FT_FaceRec.num_glyphs %\n", cast(*u8) *face_rec.num_glyphs - cast(*u8) *face_rec);
    print("FT_FaceRec.units_per_EM %\n", cast(*u8) *face_rec.units_per_EM - cast(*u8) *face_rec);
    print("FT_FaceRec.glyph %\n", cast(*u8) *face_rec.glyph - cast(*u8) *face_rec);
    print("FT_FaceRec.size %\n", cast(*u8) *face_rec.size - cast(*u8) *face_rec);
    print("FT_FaceRec.charmap %\n", cast(*u8) *face_rec.charmap - cast(*u8) *face_rec);
    print("FT_GlyphSlotRec.metrics %\n", cast(*u8) *slot.metrics - cast(*u8) *slot);
    print("FT_GlyphSlotRec.advance %\n", cast(*u8) *slot.advance - cast(*u8) *slot);
    print("FT_GlyphSlotRec.format %\n", cast(*u8) *slot.format - cast(*u8) *slot);
    print("FT_GlyphSlotRec.bitmap %\n", cast(*u8) *slot.bitmap - cast(*u8) *slot);
    print("FT_GlyphSlotRec.bitmap_left %\n", cast(*u8) *slot.bitmap_left - cast(*u8) *slot);
    print("FT_GlyphSlotRec.outline %\n", cast(*u8) *slot.outline - cast(*u8) *slot);
    print("FT_Bitmap.pixel_mode %\n", cast(*u8) *bitmap.pixel_mode - cast(*u8) *bitmap);
    print("FT_LOAD_RENDER %\n", FT_LOAD_RENDER);
    print("FT_LOAD_NO_BITMAP %\n", FT_LOAD_NO_BITMAP);
    print("FT_LOAD_TARGET_LCD %\n", cast(s32) FT_Load_Target.LCD);
    print("FT_LOAD_TARGET_LIGHT %\n", FT_LOAD_TARGET_(.LIGHT));
    print("FT_FACE_FLAG_KERNING %\n", FT_FACE_FLAG_KERNING);
    print("FT_Err_Unknown_File_Format %\n", cast(s32) FT_Err.Unknown_File_Format);
    print("FT_PIXEL_MODE_GRAY %\n", cast(s32) FT_Pixel_Mode.GRAY);
    print("FT_PIXEL_MODE_LCD %\n", cast(s32) FT_Pixel_Mode_.FT_PIXEL_MODE_LCD);
    print("FT_RENDER_MODE_SDF %\n", cast(s32) FT_Render_Mode.SDF);
    print("FT_KERNING_UNFITTED %\n", cast(s32) FT_Kerning_Mode.UNFITTED);
    print("FT_ENCODING_UNICODE %\n", cast(s64) FT_Encoding.UNICODE);
    print("FT_GLYPH_FORMAT_OUTLINE %\n", cast(s64) FT_Glyph_Format.FT_GLYPH_FORMAT_OUTLINE);
    print("FREETYPE_MAJOR %\n", FREETYPE_MAJOR);

    library: FT_Library;
    if FT_Init_FreeType(*library) exit(1);
    major, minor, patch: FT_Int;
    FT_Library_Version(library, *major, *minor, *patch);
    print("version.major %\nversion.minor %\nversion.patch %\n", major, minor, patch);
    face: FT_Face;
    garbage := "not a font";
    print("garbage %\n", FT_New_Memory_Face(library, garbage.data, 10, 0, *face));
    font := getenv("OJ_TEST_FONT");
    if font && FT_New_Face(library, font, 0, *face) == 0 {
        print("face.num_glyphs %\n", face.num_glyphs);
        print("face.units_per_EM %\n", face.units_per_EM);
        print("face.ascender %\n", face.ascender);
        print("face.descender %\n", face.descender);
        print("face.kerning %\n", cast(s32) FT_HAS_KERNING(face));
        FT_Set_Pixel_Sizes(face, 0, 32);
        print("size.ascender %\n", face.size.metrics.ascender);
        names := string.["A", "g"];
        for names {
            index := FT_Get_Char_Index(face, it[0]);
            FT_Load_Glyph(face, index, FT_LOAD_RENDER);
            g := face.glyph;
            print("glyph.% % % % % % % % %\n", it, index, g.advance.x, g.metrics.horiBearingY,
                  g.bitmap.rows, g.bitmap.width, g.bitmap_left, g.bitmap_top, g.bitmap.pixel_mode);
        }
        FT_Done_Face(face);
    }
    FT_Done_FreeType(library);
}
"#;

fn system_font() -> Option<String> {
  let output = Command::new("fc-match")
    .args(["-f", "%{file}", "sans"])
    .output()
    .ok()?;
  let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
  (output.status.success() && Path::new(&path).is_file()).then_some(path)
}

/// The FreeType binding lays its records out the way `freetype.h` does and
/// names its constants what the header means by them; opened on a font the
/// system has, a face and two rendered glyphs measure the same through it as
/// through C.
#[test]
fn freetype_measures_a_system_font_the_way_c_does() {
  if !system_has(&["freetype2"]) {
    return;
  }
  let Some(built) = build(FREETYPE_JAI) else {
    return;
  };
  let mut environment = Vec::new();
  let font = system_font();
  match &font {
    Some(font) => environment.push(("OJ_TEST_FONT", font.clone())),
    None => {
      eprintln!("no font: fc-match found none, so only the layout and the library are compared")
    }
  }
  let from_jai = run_ok(&built, &[], &environment);
  let from_c = measure_in_c(FREETYPE_C, &["freetype2"], &environment);
  assert_same_facts(&from_c, &from_jai, 38);
  if font.is_some() {
    assert!(
      from_jai.contains("glyph.A "),
      "a glyph was measured:\n{from_jai}"
    );
  }
}

const X11_C: &str = r#"#include <stdio.h>
#include <stddef.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/Xatom.h>
#include <X11/keysym.h>
#include <X11/cursorfont.h>

#define SIZE(name, type)        printf("%s %zu\n", name, sizeof(type))
#define OFF(name, type, member) printf("%s %zu\n", name, offsetof(type, member))
#define VAL(name, expr)         printf("%s %lld\n", name, (long long)(expr))
int main(void) {
  SIZE("XEvent", XEvent);
  SIZE("XAnyEvent", XAnyEvent);
  SIZE("XKeyEvent", XKeyEvent);
  SIZE("XButtonEvent", XButtonEvent);
  SIZE("XMotionEvent", XMotionEvent);
  SIZE("XConfigureEvent", XConfigureEvent);
  SIZE("XClientMessageEvent", XClientMessageEvent);
  SIZE("XSelectionRequestEvent", XSelectionRequestEvent);
  SIZE("XSelectionEvent", XSelectionEvent);
  SIZE("XPropertyEvent", XPropertyEvent);
  SIZE("XSetWindowAttributes", XSetWindowAttributes);
  SIZE("XWindowAttributes", XWindowAttributes);
  SIZE("XSizeHints", XSizeHints);
  SIZE("XWMHints", XWMHints);
  SIZE("XClassHint", XClassHint);
  SIZE("XVisualInfo", XVisualInfo);
  SIZE("XImage", XImage);
  SIZE("XIMStyles", XIMStyles);
  SIZE("XKeyboardState", XKeyboardState);
  SIZE("XGCValues", XGCValues);
  SIZE("XColor", XColor);
  SIZE("XErrorEvent", XErrorEvent);
  SIZE("XTextProperty", XTextProperty);
  SIZE("XIMText", XIMText);
  OFF("XKeyEvent.keycode", XKeyEvent, keycode);
  OFF("XKeyEvent.state", XKeyEvent, state);
  OFF("XClientMessageEvent.data", XClientMessageEvent, data);
  OFF("XSelectionRequestEvent.property", XSelectionRequestEvent, property);
  OFF("XSelectionRequestEvent.time", XSelectionRequestEvent, time);
  OFF("XSizeHints.min_aspect", XSizeHints, min_aspect);
  OFF("XSizeHints.max_aspect", XSizeHints, max_aspect);
  OFF("XSizeHints.base_width", XSizeHints, base_width);
  OFF("XSizeHints.win_gravity", XSizeHints, win_gravity);
  OFF("XImage.f", XImage, f);
  OFF("XIMText.string", XIMText, string);
  OFF("XVisualInfo.depth", XVisualInfo, depth);
  OFF("XSetWindowAttributes.event_mask", XSetWindowAttributes, event_mask);
  OFF("XWindowAttributes.map_state", XWindowAttributes, map_state);
  VAL("XA_STRING", XA_STRING);
  VAL("XA_ATOM", XA_ATOM);
  VAL("XA_CARDINAL", XA_CARDINAL);
  VAL("SelectionNotify", SelectionNotify);
  VAL("SelectionRequest", SelectionRequest);
  VAL("KeyPress", KeyPress);
  VAL("ClientMessage", ClientMessage);
  VAL("LASTEvent", LASTEvent);
  VAL("KeyPressMask", KeyPressMask);
  VAL("StructureNotifyMask", StructureNotifyMask);
  VAL("PointerMotionMask", PointerMotionMask);
  VAL("CWEventMask", CWEventMask);
  VAL("CWColormap", CWColormap);
  VAL("PropModeReplace", PropModeReplace);
  VAL("XK_Escape", XK_Escape);
  VAL("XK_a", XK_a);
  VAL("XK_Return", XK_Return);
  VAL("XK_F12", XK_F12);
  VAL("XC_xterm", XC_xterm);
  VAL("AnyPropertyType", AnyPropertyType);
  VAL("None", None);
  VAL("CurrentTime", CurrentTime);
  VAL("XLookupBoth", XLookupBoth);
  VAL("XIMPreeditNothing", XIMPreeditNothing);
  VAL("Success", Success);
  VAL("BadWindow", BadWindow);
  VAL("InputOutput", InputOutput);
  VAL("AllocNone", AllocNone);
  VAL("ShiftMask", ShiftMask);
  VAL("Mod1Mask", Mod1Mask);
  VAL("PMinSize", PMinSize);
  VAL("USPosition", USPosition);
  printf("%s %s\n", "XNClientWindow", XNClientWindow);
  printf("%s %s\n", "XNInputStyle", XNInputStyle);
  return 0;
}
"#;

const X11_JAI: &str = r#"#import "Basic";
X :: #import "X11";
main :: () {
    print("XEvent %\n", size_of(X.XEvent));
    print("XAnyEvent %\n", size_of(X.XAnyEvent));
    print("XKeyEvent %\n", size_of(X.XKeyEvent));
    print("XButtonEvent %\n", size_of(X.XButtonEvent));
    print("XMotionEvent %\n", size_of(X.XMotionEvent));
    print("XConfigureEvent %\n", size_of(X.XConfigureEvent));
    print("XClientMessageEvent %\n", size_of(X.XClientMessageEvent));
    print("XSelectionRequestEvent %\n", size_of(X.XSelectionRequestEvent));
    print("XSelectionEvent %\n", size_of(X.XSelectionEvent));
    print("XPropertyEvent %\n", size_of(X.XPropertyEvent));
    print("XSetWindowAttributes %\n", size_of(X.XSetWindowAttributes));
    print("XWindowAttributes %\n", size_of(X.XWindowAttributes));
    print("XSizeHints %\n", size_of(X.XSizeHints));
    print("XWMHints %\n", size_of(X.XWMHints));
    print("XClassHint %\n", size_of(X.XClassHint));
    print("XVisualInfo %\n", size_of(X.XVisualInfo));
    print("XImage %\n", size_of(X.XImage));
    print("XIMStyles %\n", size_of(X.XIMStyles));
    print("XKeyboardState %\n", size_of(X.XKeyboardState));
    print("XGCValues %\n", size_of(X.XGCValues));
    print("XColor %\n", size_of(X.XColor));
    print("XErrorEvent %\n", size_of(X.XErrorEvent));
    print("XTextProperty %\n", size_of(X.XTextProperty));
    print("XIMText %\n", size_of(X.XIMText));
    { v: X.XKeyEvent; print("XKeyEvent.keycode %\n", cast(*u8) *v.keycode - cast(*u8) *v); }
    { v: X.XKeyEvent; print("XKeyEvent.state %\n", cast(*u8) *v.state - cast(*u8) *v); }
    { v: X.XClientMessageEvent; print("XClientMessageEvent.data %\n", cast(*u8) *v.data - cast(*u8) *v); }
    { v: X.XSelectionRequestEvent; print("XSelectionRequestEvent.property %\n", cast(*u8) *v.property - cast(*u8) *v); }
    { v: X.XSelectionRequestEvent; print("XSelectionRequestEvent.time %\n", cast(*u8) *v.time - cast(*u8) *v); }
    { v: X.XSizeHints; print("XSizeHints.min_aspect %\n", cast(*u8) *v.min_aspect - cast(*u8) *v); }
    { v: X.XSizeHints; print("XSizeHints.max_aspect %\n", cast(*u8) *v.max_aspect - cast(*u8) *v); }
    { v: X.XSizeHints; print("XSizeHints.base_width %\n", cast(*u8) *v.base_width - cast(*u8) *v); }
    { v: X.XSizeHints; print("XSizeHints.win_gravity %\n", cast(*u8) *v.win_gravity - cast(*u8) *v); }
    { v: X.XImage; print("XImage.f %\n", cast(*u8) *v.f - cast(*u8) *v); }
    { v: X.XIMText; print("XIMText.string %\n", cast(*u8) *v._string - cast(*u8) *v); }
    { v: X.XVisualInfo; print("XVisualInfo.depth %\n", cast(*u8) *v.depth - cast(*u8) *v); }
    { v: X.XSetWindowAttributes; print("XSetWindowAttributes.event_mask %\n", cast(*u8) *v.event_mask - cast(*u8) *v); }
    { v: X.XWindowAttributes; print("XWindowAttributes.map_state %\n", cast(*u8) *v.map_state - cast(*u8) *v); }
    print("XA_STRING %\n", X.XA_STRING);
    print("XA_ATOM %\n", X.XA_ATOM);
    print("XA_CARDINAL %\n", X.XA_CARDINAL);
    print("SelectionNotify %\n", X.SelectionNotify);
    print("SelectionRequest %\n", X.SelectionRequest);
    print("KeyPress %\n", X.KeyPress);
    print("ClientMessage %\n", X.ClientMessage);
    print("LASTEvent %\n", X.LASTEvent);
    print("KeyPressMask %\n", X.KeyPressMask);
    print("StructureNotifyMask %\n", X.StructureNotifyMask);
    print("PointerMotionMask %\n", X.PointerMotionMask);
    print("CWEventMask %\n", X.CWEventMask);
    print("CWColormap %\n", X.CWColormap);
    print("PropModeReplace %\n", X.PropModeReplace);
    print("XK_Escape %\n", X.XK_Escape);
    print("XK_a %\n", X.XK_a);
    print("XK_Return %\n", X.XK_Return);
    print("XK_F12 %\n", X.XK_F12);
    print("XC_xterm %\n", X.XC_xterm);
    print("AnyPropertyType %\n", X.AnyPropertyType);
    print("None %\n", X.None);
    print("CurrentTime %\n", X.CurrentTime);
    print("XLookupBoth %\n", X.XLookupBoth);
    print("XIMPreeditNothing %\n", X.XIMPreeditNothing);
    print("Success %\n", X.Success);
    print("BadWindow %\n", X.BadWindow);
    print("InputOutput %\n", X.InputOutput);
    print("AllocNone %\n", X.AllocNone);
    print("ShiftMask %\n", X.ShiftMask);
    print("Mod1Mask %\n", X.Mod1Mask);
    print("PMinSize %\n", X.PMinSize);
    print("USPosition %\n", X.USPosition);
    print("XNClientWindow %\n", X.XNClientWindow);
    print("XNInputStyle %\n", X.XNInputStyle);
    if get_command_line_arguments().count < 2  return;
    display := X.XOpenDisplay(null);
    if !display { print("no display\n"); return; }
    root := X.XDefaultRootWindow(display);
    window := X.XCreateSimpleWindow(display, root, 0, 0, 32, 32, 0, 0, 0);
    X.XStoreName(display, window, "orangejuice");
    name: *u8;
    X.XFetchName(display, window, *name);
    atom := X.XInternAtom(display, "OJ_TEST_ATOM", X.False);
    atom_name := X.XGetAtomName(display, atom);
    attributes: X.XWindowAttributes;
    X.XGetWindowAttributes(display, window, *attributes);
    print("window % %x% name % atom %\n", window != 0, attributes.width, attributes.height, to_string(name), to_string(atom_name));
    X.XFree(name);
    X.XFree(atom_name);
    X.XDestroyWindow(display, window);
    X.XCloseDisplay(display);
}
"#;

/// A display for a test to open windows on: an `Xvfb` of its own when there
/// is one, otherwise `$DISPLAY` when `desktop_is_fine` — a window that is
/// never mapped disturbs nobody, a clipboard owner would.
fn display_for_test(desktop_is_fine: bool) -> Option<(Option<VirtualDisplay>, String)> {
  if let Some(virtual_display) = VirtualDisplay::start() {
    let display = virtual_display.display.clone();
    return Some((Some(virtual_display), display));
  }
  if desktop_is_fine
    && let Ok(display) = std::env::var("DISPLAY")
    && !display.is_empty()
  {
    return Some((None, display));
  }
  None
}

/// Xlib's event structs, window attributes and size hints are the size the
/// headers make them, with their members where the headers put them —
/// including `XSizeHints`' two anonymous aspect structs and `XIMText`'s
/// member called `string` — and the protocol constants, atoms, keysyms and
/// input-method names are the header's values. On a display, a window is
/// created, named and measured.
#[test]
fn x11_lays_out_what_xlib_declares_and_talks_to_a_display() {
  if !system_has(&["x11"]) {
    return;
  }
  let Some(built) = build(X11_JAI) else {
    return;
  };
  let from_jai = run_ok(&built, &[], &[]);
  let from_c = measure_in_c(X11_C, &["x11"], &[]);
  assert_same_facts(&from_c, &from_jai, 70);

  let Some((_server, display)) = display_for_test(true) else {
    eprintln!("skipping the display half: no Xvfb and no $DISPLAY");
    return;
  };
  let output = run_ok(&built, &["display"], &[("DISPLAY", display)]);
  assert!(
    output.ends_with("window true 32x32 name orangejuice atom OJ_TEST_ATOM\n"),
    "{output}"
  );
}

const GL_C: &str = r#"#include <stdio.h>
#include <stddef.h>
#define GL_GLEXT_PROTOTYPES 1
#include <GL/gl.h>
#include <GL/glext.h>
#include <GL/glx.h>
#include <GL/glxext.h>

#define SIZE(name, type)        printf("%s %zu\n", name, sizeof(type))
#define OFF(name, type, member) printf("%s %zu\n", name, offsetof(type, member))
#define VAL(name, expr)         printf("%s %lld\n", name, (long long)(expr))
int main(void) {
  SIZE("GLsync", GLsync);
  SIZE("GLintptr", GLintptr);
  SIZE("GLsizeiptr", GLsizeiptr);
  SIZE("GLuint64", GLuint64);
  SIZE("GLint64", GLint64);
  SIZE("GLhalf", GLhalf);
  SIZE("GLenum", GLenum);
  SIZE("GLboolean", GLboolean);
  SIZE("GLbitfield", GLbitfield);
  SIZE("GLchar", GLchar);
  SIZE("GLdouble", GLdouble);
  SIZE("GLfixed", GLfixed);
  SIZE("GLhandleARB", GLhandleARB);
  SIZE("GLDEBUGPROC", GLDEBUGPROC);
  SIZE("GLXContext", GLXContext);
  SIZE("GLXFBConfig", GLXFBConfig);
  SIZE("GLXDrawable", GLXDrawable);
  VAL("GL_COLOR_BUFFER_BIT", GL_COLOR_BUFFER_BIT);
  VAL("GL_DEPTH_BUFFER_BIT", GL_DEPTH_BUFFER_BIT);
  VAL("GL_TEXTURE_2D", GL_TEXTURE_2D);
  VAL("GL_RGBA", GL_RGBA);
  VAL("GL_RGBA8", GL_RGBA8);
  VAL("GL_UNSIGNED_BYTE", GL_UNSIGNED_BYTE);
  VAL("GL_FLOAT", GL_FLOAT);
  VAL("GL_TRIANGLES", GL_TRIANGLES);
  VAL("GL_ARRAY_BUFFER", GL_ARRAY_BUFFER);
  VAL("GL_STATIC_DRAW", GL_STATIC_DRAW);
  VAL("GL_FRAGMENT_SHADER", GL_FRAGMENT_SHADER);
  VAL("GL_VERTEX_SHADER", GL_VERTEX_SHADER);
  VAL("GL_COMPILE_STATUS", GL_COMPILE_STATUS);
  VAL("GL_FRAMEBUFFER", GL_FRAMEBUFFER);
  VAL("GL_COLOR_ATTACHMENT0", GL_COLOR_ATTACHMENT0);
  VAL("GL_NUM_EXTENSIONS", GL_NUM_EXTENSIONS);
  VAL("GL_EXTENSIONS", GL_EXTENSIONS);
  VAL("GL_VERSION", GL_VERSION);
  VAL("GL_DEBUG_OUTPUT", GL_DEBUG_OUTPUT);
  VAL("GL_DEBUG_OUTPUT_SYNCHRONOUS", GL_DEBUG_OUTPUT_SYNCHRONOUS);
  VAL("GL_TEXTURE_MAX_ANISOTROPY_EXT", GL_TEXTURE_MAX_ANISOTROPY_EXT);
  VAL("GL_SRGB8_ALPHA8", GL_SRGB8_ALPHA8);
  VAL("GL_TIMEOUT_IGNORED", GL_TIMEOUT_IGNORED);
  VAL("GL_INVALID_INDEX", GL_INVALID_INDEX);
  VAL("GL_TRUE", GL_TRUE);
  VAL("GL_FALSE", GL_FALSE);
  VAL("GL_ALL_ATTRIB_BITS", GL_ALL_ATTRIB_BITS);
  VAL("GL_MULTISAMPLE", GL_MULTISAMPLE);
  VAL("GL_FRAMEBUFFER_SRGB", GL_FRAMEBUFFER_SRGB);
  VAL("GL_SHADER_STORAGE_BUFFER", GL_SHADER_STORAGE_BUFFER);
  VAL("GL_GLEXT_VERSION", GL_GLEXT_VERSION);
  VAL("GLX_RGBA", GLX_RGBA);
  VAL("GLX_DOUBLEBUFFER", GLX_DOUBLEBUFFER);
  VAL("GLX_DEPTH_SIZE", GLX_DEPTH_SIZE);
  VAL("GLX_SAMPLES", GLX_SAMPLES);
  VAL("GLX_SAMPLE_BUFFERS", GLX_SAMPLE_BUFFERS);
  VAL("GLX_CONTEXT_MAJOR_VERSION_ARB", GLX_CONTEXT_MAJOR_VERSION_ARB);
  VAL("GLX_CONTEXT_CORE_PROFILE_BIT_ARB", GLX_CONTEXT_CORE_PROFILE_BIT_ARB);
  VAL("GLX_RGBA_TYPE", GLX_RGBA_TYPE);
  VAL("GLX_DRAWABLE_TYPE", GLX_DRAWABLE_TYPE);
  VAL("GLX_WINDOW_BIT", GLX_WINDOW_BIT);
  printf("procedures %d\n", 2975);
  return 0;
}
"#;

const GL_JAI: &str = r#"#import "Basic";
#import "GL";
X :: #import "X11";
main :: () {
    print("GLsync %\n", size_of(GLsync));
    print("GLintptr %\n", size_of(GLintptr));
    print("GLsizeiptr %\n", size_of(GLsizeiptr));
    print("GLuint64 %\n", size_of(GLuint64));
    print("GLint64 %\n", size_of(GLint64));
    print("GLhalf %\n", size_of(GLhalf));
    print("GLenum %\n", size_of(GLenum));
    print("GLboolean %\n", size_of(GLboolean));
    print("GLbitfield %\n", size_of(GLbitfield));
    print("GLchar %\n", size_of(GLchar));
    print("GLdouble %\n", size_of(GLdouble));
    print("GLfixed %\n", size_of(GLfixed));
    print("GLhandleARB %\n", size_of(GLhandleARB));
    print("GLDEBUGPROC %\n", size_of(GLDEBUGPROC));
    print("GLXContext %\n", size_of(GLXContext));
    print("GLXFBConfig %\n", size_of(GLXFBConfig));
    print("GLXDrawable %\n", size_of(GLXDrawable));
    print("GL_COLOR_BUFFER_BIT %\n", GL_COLOR_BUFFER_BIT);
    print("GL_DEPTH_BUFFER_BIT %\n", GL_DEPTH_BUFFER_BIT);
    print("GL_TEXTURE_2D %\n", GL_TEXTURE_2D);
    print("GL_RGBA %\n", GL_RGBA);
    print("GL_RGBA8 %\n", GL_RGBA8);
    print("GL_UNSIGNED_BYTE %\n", GL_UNSIGNED_BYTE);
    print("GL_FLOAT %\n", GL_FLOAT);
    print("GL_TRIANGLES %\n", GL_TRIANGLES);
    print("GL_ARRAY_BUFFER %\n", GL_ARRAY_BUFFER);
    print("GL_STATIC_DRAW %\n", GL_STATIC_DRAW);
    print("GL_FRAGMENT_SHADER %\n", GL_FRAGMENT_SHADER);
    print("GL_VERTEX_SHADER %\n", GL_VERTEX_SHADER);
    print("GL_COMPILE_STATUS %\n", GL_COMPILE_STATUS);
    print("GL_FRAMEBUFFER %\n", GL_FRAMEBUFFER);
    print("GL_COLOR_ATTACHMENT0 %\n", GL_COLOR_ATTACHMENT0);
    print("GL_NUM_EXTENSIONS %\n", GL_NUM_EXTENSIONS);
    print("GL_EXTENSIONS %\n", GL_EXTENSIONS);
    print("GL_VERSION %\n", GL_VERSION);
    print("GL_DEBUG_OUTPUT %\n", GL_DEBUG_OUTPUT);
    print("GL_DEBUG_OUTPUT_SYNCHRONOUS %\n", GL_DEBUG_OUTPUT_SYNCHRONOUS);
    print("GL_TEXTURE_MAX_ANISOTROPY_EXT %\n", GL_TEXTURE_MAX_ANISOTROPY_EXT);
    print("GL_SRGB8_ALPHA8 %\n", GL_SRGB8_ALPHA8);
    print("GL_TIMEOUT_IGNORED %\n", GL_TIMEOUT_IGNORED);
    print("GL_INVALID_INDEX %\n", GL_INVALID_INDEX);
    print("GL_TRUE %\n", GL_TRUE);
    print("GL_FALSE %\n", GL_FALSE);
    print("GL_ALL_ATTRIB_BITS %\n", GL_ALL_ATTRIB_BITS);
    print("GL_MULTISAMPLE %\n", GL_MULTISAMPLE);
    print("GL_FRAMEBUFFER_SRGB %\n", GL_FRAMEBUFFER_SRGB);
    print("GL_SHADER_STORAGE_BUFFER %\n", GL_SHADER_STORAGE_BUFFER);
    print("GL_GLEXT_VERSION %\n", GL_GLEXT_VERSION);
    print("GLX_RGBA %\n", GLX_RGBA);
    print("GLX_DOUBLEBUFFER %\n", GLX_DOUBLEBUFFER);
    print("GLX_DEPTH_SIZE %\n", GLX_DEPTH_SIZE);
    print("GLX_SAMPLES %\n", GLX_SAMPLES);
    print("GLX_SAMPLE_BUFFERS %\n", GLX_SAMPLE_BUFFERS);
    print("GLX_CONTEXT_MAJOR_VERSION_ARB %\n", GLX_CONTEXT_MAJOR_VERSION_ARB);
    print("GLX_CONTEXT_CORE_PROFILE_BIT_ARB %\n", GLX_CONTEXT_CORE_PROFILE_BIT_ARB);
    print("GLX_RGBA_TYPE %\n", GLX_RGBA_TYPE);
    print("GLX_DRAWABLE_TYPE %\n", GLX_DRAWABLE_TYPE);
    print("GLX_WINDOW_BIT %\n", GLX_WINDOW_BIT);
    procedures := 0;
    for type_info(GL_Procedures).members  if it.type.type == .PROCEDURE  procedures += 1;
    print("procedures %\n", procedures);
    gl_load(*gl);
    print("loaded without a context: % % %, flags % %\n", glClear != null, glCreateShader != null, glDebugMessageCallback != null, GL_VERSION_2_0, GL_ARB_debug_output);
    if get_command_line_arguments().count < 2  return;

    display := X.XOpenDisplay(null);
    if !display { print("no display\n"); return; }
    defer X.XCloseDisplay(display);
    screen := X.XDefaultScreen(display);
    attributes := glx_visual_attributes(0);
    visual := glXChooseVisual(display, screen, attributes.data);
    if !visual { print("no visual\n"); return; }
    root := X.XRootWindow(display, screen);
    swa: X.XSetWindowAttributes;
    swa.colormap = X.XCreateColormap(display, root, visual.visual, X.AllocNone);
    window := X.XCreateWindow(display, root, 0, 0, 16, 16, 0, visual.depth, X.InputOutput, visual.visual, X.CWColormap | X.CWBorderPixel, *swa);
    gl_context := glXCreateContext(display, visual, null, X.True);
    if !gl_context || !glXMakeCurrent(display, window, gl_context) { print("no context\n"); return; }
    gl_load(*gl);
    major, minor := gl_version();
    print("context % % %\n", major >= 2, GL_VERSION_1_1, GL_VERSION_2_0);
    framebuffer, texture: GLuint;
    glGenFramebuffers(1, *framebuffer);
    glGenTextures(1, *texture);
    glBindTexture(GL_TEXTURE_2D, texture);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, 4, 4, 0, GL_RGBA, GL_UNSIGNED_BYTE, null);
    glBindFramebuffer(GL_FRAMEBUFFER, framebuffer);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, texture, 0);
    complete := glCheckFramebufferStatus(GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE;
    glViewport(0, 0, 4, 4);
    glClearColor(1, 0, 1, 1);
    glClear(GL_COLOR_BUFFER_BIT);
    pixel: [4] u8;
    glReadPixels(1, 1, 1, 1, GL_RGBA, GL_UNSIGNED_BYTE, pixel.data);
    print("framebuffer % complete % pixel % error %\n", framebuffer != 0, complete, pixel, glGetError());
    glXMakeCurrent(display, 0, null);
    glXDestroyContext(display, gl_context);
    X.XDestroyWindow(display, window);
}
"#;

/// `GL_Procedures` has a member for every entry point `gl.h` and `glext.h`
/// declare, the constants are the headers' values and the GLX types are
/// their size. Without a context `gl_load` still fills in the procedures and
/// leaves every version flag false; with one — on whichever display has GLX —
/// it reports the context's version, and a framebuffer cleared through the
/// loaded procedures reads back the colour it was cleared to.
#[test]
fn gl_loads_its_procedures_and_draws_through_them() {
  if !system_has(&["gl", "x11"]) {
    return;
  }
  let Some(built) = build(GL_JAI) else {
    return;
  };
  let from_jai = run_ok(&built, &[], &[]);
  let from_c = measure_in_c(GL_C, &["gl", "x11"], &[]);
  assert_same_facts(&from_c, &from_jai, 59);
  assert!(
    from_jai.ends_with("loaded without a context: true true true, flags false false\n"),
    "{from_jai}"
  );

  let mut displays = Vec::new();
  if let Some(virtual_display) = VirtualDisplay::start() {
    displays.push((Some(virtual_display.display.clone()), Some(virtual_display)));
  }
  if let Ok(display) = std::env::var("DISPLAY") {
    displays.push((Some(display), None));
  }
  for (display, _server) in &displays {
    let Some(display) = display else {
      continue;
    };
    let output = run_ok(&built, &["display"], &[("DISPLAY", display.clone())]);
    let last = output.lines().last().unwrap_or("");
    if last == "no visual" || last == "no context" || last == "no display" {
      eprintln!("{display} has no usable GLX ({last})");
      continue;
    }
    assert!(
      output.ends_with(
        "context true true true\nframebuffer true complete true pixel [255, 0, 255, 255] error 0\n"
      ),
      "{output}"
    );
    return;
  }
  eprintln!("skipping the context half: no display with GLX");
}

const CLIPBOARD_JAI: &str = r#"
#import "Basic";
#import "Clipboard";

main :: () {
    args := get_command_line_arguments();
    if args.count > 2 && args[1] == "own" {
        print("set %\n", os_clipboard_set_text(args[2]));
        // Long enough to outlast a reader started on a machine busy with the
        // rest of the suite; the test kills it as soon as it has read.
        sleep_milliseconds(60000);
        return;
    }
    if args.count > 1 && args[1] == "read" {
        print("[%]\n", os_clipboard_get_text());
        return;
    }
    print("empty [%]\n", os_clipboard_get_text());
    print("set %\n", os_clipboard_set_text("héllo, clipboard"));
    print("got [%]\n", os_clipboard_get_text());
    print("set %\n", os_clipboard_set_text("second"));
    print("got [%]\n", os_clipboard_get_text());
}
"#;

/// Text set is text got, in the program that set it and — through the X
/// selection protocol — in another one, while the first stays the owner.
/// This only ever runs on an `Xvfb` of its own: owning the desktop's
/// CLIPBOARD would overwrite what its user copied.
#[test]
fn the_clipboard_hands_text_to_another_program() {
  if !system_has(&["x11"]) {
    return;
  }
  let Some(built) = build(CLIPBOARD_JAI) else {
    return;
  };
  let Some((_server, display)) = display_for_test(false) else {
    eprintln!("skipping: no Xvfb to own a clipboard on");
    return;
  };
  let environment = [("DISPLAY", display.clone())];
  assert_eq!(
    run_ok(&built, &[], &environment),
    "empty []\nset true\ngot [héllo, clipboard]\nset true\ngot [second]\n"
  );

  let mut owner = Command::new(&built.executable)
    .args(["own", "from another program ✓"])
    .env("DISPLAY", &display)
    .stdout(Stdio::piped())
    .spawn()
    .expect("the owning program should start");
  let mut line = String::new();
  BufReader::new(owner.stdout.take().expect("its output"))
    .read_line(&mut line)
    .expect("it says it owns the selection");
  assert_eq!(line, "set true\n");
  let read = run_ok(&built, &["read"], &environment);
  let _ = owner.kill();
  let _ = owner.wait();
  assert_eq!(read, "[from another program ✓]\n");
}
