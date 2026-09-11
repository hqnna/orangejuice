//
// `using`, `#as`, and the variants that make a new type out of an old one
// (**L§3.13**, **L§8.4**).
//

#import "Basic";

Entity :: struct {
    id:   int;
    name: string;
}

// `#as` says this member is what the struct *is*, so a `Player` converts to an
// `Entity` wherever one is wanted; `using` puts its members in scope.
Player :: struct {
    using #as entity: Entity;
    score: int;
}

// A distinct type has the same shape and is not the same type: a `Meters`
// cannot be handed where `Seconds` is wanted, and neither takes a plain float
// without being asked to.
Meters  :: #type,distinct float64;
Seconds :: #type,distinct float64;

// An `isa` variant converts *to* what it was made from but not back, which is
// what makes it a label rather than a wall.
Celsius :: #type,isa float64;

report :: (e: Entity) {
    print("entity %: %\n", e.id, e.name);
}

speed :: (distance: Meters, time: Seconds) -> float64 {
    return cast(float64) distance / cast(float64) time;
}

main :: () {
    p: Player;
    p.id = 7;
    p.name = "ada";
    p.score = 100;

    // `using` made these reachable without naming `entity`.
    print("% % %\n", p.id, p.name, p.score);
    // `#as` made a `Player` usable as an `Entity`.
    report(p);

    d := cast(Meters) 100.0;
    t := cast(Seconds) 9.58;
    print("%\n", speed(d, t));
    print("% %\n", type_of(d), type_of(t));

    temperature := cast(Celsius) 21.5;
    // An `isa` variant is a float64 where one is wanted.
    print("% %\n", temperature + 1.0, type_of(temperature));

    // `using` works on a value inside a body too, which is what makes a long
    // member chain readable.
    {
        using p;
        print("%, scoring %\n", name, score);
    }
}
