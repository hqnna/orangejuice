//
// `#overlay (member) name: T;` lays one member over another's bytes without
// moving anything after it (**L§8.6**): a second view of storage the struct
// already has, the way a `union` would give one, but declared next to the
// members it views.
//

#import "Basic";

Keys :: struct {
    shift: bool;
    control: bool;
    alt: bool;
    super: bool;
    // One word that is all four at once, so that "is anything held" is one
    // comparison.
    #overlay (shift) packed: u32;
    repeat: u16;
}

Vector2 :: struct {
    x, y: float64;
    #overlay (x) component: [2] float64 = ---;
}

offset :: (T: Type, name: string) -> s64 {
    for (cast(*Type_Info_Struct) T).members  if it.name == name  return it.offset_in_bytes;
    return -1;
}

main :: () {
    // The overlay starts where `shift` does, and `repeat` is where it would
    // have been without it. The struct does take the overlay's alignment, as
    // it would a union's, so it is 8 bytes rather than 6.
    print("Keys: size %, packed at %, repeat at %\n", size_of(Keys), offset(Keys, "packed"), offset(Keys, "repeat"));

    keys: Keys;
    print("nothing held: %\n", keys.packed == 0);
    keys.alt = true;
    print("alt held: %, packed = %\n", keys.packed != 0, keys.packed);

    // A named struct literal may set the overlay; the members under it then
    // read back its bytes.
    all := Keys.{packed = 0x0101_0101};
    print("% % % %\n", all.shift, all.control, all.alt, all.super);

    // A positional one fills the members the overlay covers, not the overlay.
    v := Vector2.{3, 4};
    print("% %\n", v.component[0], v.component[1]);
    v.component[1] = 10;
    print("%\n", v.y);

    // The type info says which members are views rather than storage.
    for type_info(Keys).members {
        if it.flags & .OVERLAY  print("% is an overlay\n", it.name);
    }
}
