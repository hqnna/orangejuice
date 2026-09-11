//
// Structs: members with defaults, literals, `using`, and what the compiler
// lays out.
//

#import "Basic";

Vector2 :: struct {
    x: float;
    y: float;
}

// A member may have a default, and it is applied wherever the struct is made
// — including as a member of another struct (**L§8.1**).
Sprite :: struct {
    position: Vector2;
    scale:    float = 1.0;
    visible   := true;
    name:     string = "unnamed";
}

// `using` puts one struct's members into another's namespace, so an `Enemy`
// has `.position` without writing `.base.position` (**L§8.4**).
Enemy :: struct {
    using sprite: Sprite;
    health: int = 100;
}

main :: () {
    // A literal may be positional or name its members; a member nobody names
    // takes its default.
    origin := Vector2.{};
    corner := Vector2.{1.0, 2.0};
    named  := Vector2.{y = 5.0};
    print("% % %\n", origin, corner, named);

    s: Sprite;
    print("%\n", s);

    e: Enemy;
    e.position.x = 3.0;
    e.name = "goblin";
    print("% at % with % health\n", e.name, e.position.x, e.health);

    // The layout is the compiler's, and `size_of` is how to ask about it.
    print("Vector2 is % bytes, Sprite is %\n", size_of(Vector2), size_of(Sprite));

    // A struct is a value: assigning copies it.
    copy := e.sprite;
    copy.name = "changed";
    print("% %\n", e.name, copy.name);
}
