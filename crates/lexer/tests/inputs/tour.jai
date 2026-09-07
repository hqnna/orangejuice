#!/usr/bin/env jai
// A tour of the lexical structure, for the `oj dump tokens` snapshot.
#import "Basic";

Mode :: enum u8 { QUIET; LOUD; }

Config :: struct {
  read\_package: bool;      // backslash continuation
  tolerance: float64;
  name: string;             @nodoc
} @Serialized

TAU       :: 6.2831853071795864769;   /* nested /* comment */ still a comment */
BIG       :: 0xffff_ffff_ffff_ffff;
BITS      :: 0b1011;
NEG_INF   :: 0hff80_0000;
RANGE     :: 0..7;
GREETING  :: "hi\tthere \x41\d065é\%";

USAGE :: #string DONE
  usage: tour [-quiet]
  no \escapes here
DONE

main :: (mode := Mode.QUIET) -> bool #expand {
  x := 1.5e-3;
  y := x <<< 2 | 3 &&  4;
  `defer cleanup();
  for 0..7  if it == { case 3; break; }
  return cast(bool) x.* != y.{};
}
