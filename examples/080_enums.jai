//
// Enums, flag enums, and the `.MEMBER` form that needs no qualification.
//

#import "Basic";

Suit :: enum u8 {
    CLUBS;
    DIAMONDS;
    HEARTS;
    SPADES;
}

// A flag enum's members are bits, and the type prints as the set it holds
// (**L§3.9**).
Permissions :: enum_flags u8 {
    READ;
    WRITE;
    EXECUTE;
}

colour_of :: (suit: Suit) -> string {
    // Where the type is known, `.HEARTS` needs no `Suit.` in front of it.
    if suit == .HEARTS || suit == .DIAMONDS  return "red";
    return "black";
}

main :: () {
    for suit: Suit.CLUBS..Suit.SPADES {
        print("% is % (value %)\n", suit, colour_of(suit), cast(int) suit);
    }

    p: Permissions = .READ | .WRITE;
    print("%\n", p);
    print("can write: %\n", (p & .WRITE) != 0);
    p |= .EXECUTE;
    print("%\n", p);

    // An enum knows its own members at compile time, which is what the type
    // table makes available at runtime.
    info := type_info(Suit);
    print("Suit has % members: %\n", info.names.count, info.names);
}
