# Jai Language Specification (beta 0.2.009)

This document specifies the Jai programming language as implemented by the reference compiler, beta 0.2.009 (6 February 2025), reconstructed from the shipped `how_to/`, `modules/`, `examples/`, `README.txt` and `CHANGELOG.txt`. It is the normative language reference for orangejuice. Where the reference compiler's behavior is known only from observed usage, this is stated. Where behavior is unknown, it is listed under "Open questions" at the end.

Terminology: "the compiler" means any conforming implementation. "Reference compiler" means Thekla's `jai` beta 0.2.009. "Error" means a compile-time diagnostic that stops compilation unless noted as a warning.

---

## 1. Program model

A Jai program is a set of source files, each parsed into a tree of *declarations* and *statements* and inserted into a tree of *scopes*. Compilation is a single whole-program process: all modules are always built from source, every time, in full. There is no separate compilation, no header files, and no preprocessor. Compile-time execution is a first-class part of the language: any procedure can be run at compile time, and a *metaprogram* (which is itself Jai code executing at compile time) drives the build.

Key properties:

- Statically typed, with full type inference for declarations.
- Whole-program, order-independent declarations in *data scopes* (file scope, struct bodies, enum bodies); ordered statements in *imperative scopes* (procedure bodies and nested blocks).
- Manual memory management through an explicit `Allocator` carried in an implicit `context`.
- No classes, constructors, destructors, exceptions, closures, or garbage collection.
- Polymorphic procedures and structs are fully monomorphized ("baked") at compile time.
- Programs are compiled through a bytecode interpreter (for compile-time execution) and one of two machine-code backends (LLVM or the native x64 backend).

---

## 2. Lexical structure

### 2.1 Source text

Source files are UTF-8. Line endings are `\n`, `\r\n` (normalized) or `\r`. A hashbang line `#!` at the very start of a file is skipped. Zero-width spaces (U+200B), Unicode directional formatting characters and non-breaking spaces (U+00A0) are skipped with a warning; other non-ASCII characters outside string literals and comments are errors. Identifiers are ASCII only.

### 2.2 Comments

- `//` to end of line.
- `/* ... */` block comments, which **nest** (depth counted).

### 2.3 Identifiers

An identifier starts with an ASCII letter or `_` and continues with letters, digits and `_`. Identifiers are case-sensitive. Maximum length is 512 bytes.

**Backslash continuation inside identifiers:** while scanning an identifier, a `\` followed by zero or more space characters is skipped and the identifier continues after the spaces. Thus `read\_package` is `read_package`, `time\        _report` is `time_report`, `frame\  _color` is `frame_color`, and `foo\   bar` is `foobar`. Only spaces (not tabs or newlines) are consumed. This is used for column alignment in the standard modules and must be supported.

`_` alone is a valid identifier with special meaning (the discard identifier, section 4.6). Names beginning with `__` are ordinary identifiers (e.g. `__reg`, `__bitfield`, `__jai_runtime_init`); the `__` prefix is used by convention for compiler-visible runtime symbols.

`it`, `it_index`, `context` (a keyword), `type`, `default`, `extern`, `remove` (a keyword) — of these, only `context` and `remove` are keywords. `it` and `it_index` are ordinary identifiers implicitly declared by `for`.

### 2.4 Keywords

The complete reserved word list:

```
if  ifx  then  else  case  for  while  break  continue  remove  return
struct  union  enum  enum_flags  interface  using  defer
null  true  false  cast  xx  inline  no_inline  operator
context  push_context  size_of  type_of  code_of  type_info
initializer_of  is_constant
```

All other language features are introduced by *directives*, which are `#` immediately followed by an identifier (e.g. `#import`, `#run`, `#if`). Directive names are not reserved as identifiers.

### 2.5 Operators and punctuation

Tokens, longest match first:

```
&&=  ||=  <<<=  >>>=  <<=  >>=  ---  ...(none)
&&  ||  <<<  >>>  <<  >>  ==  !=  <=  >=  +=  -=  *=  /=  %=  &=  |=  ^=
->  =>  ..  ,,  ===  --  .*  .{  .[
+  -  *  /  %  &  |  ^  ~  !  <  >  =  .  ,  ;  :  $  $$  #  @  `  ?  '
(  )  [  ]  {  }
```

Notes:

- `<<` is lexed as one token that the parser disambiguates between binary shift-left and the (deprecated) prefix pointer dereference.
- `<<<` / `>>>` are rotates and are single tokens; the lexer emits them directly (`Jai_Lexer` produces `ROTATE_LEFT` / `ROTATE_RIGHT`), so the parser never has to reassemble a `<<` `<` sequence.
- `.*` is the postfix dereference; `.{` begins a struct literal; `.[` begins an array literal; `..` is the range / spread token; `,,` introduces context arguments. `.` followed by a digit starts a number (`.5`).
- `?` is reserved and produces an error if used as an operator. `'` is a single-character token that is not used by the grammar (the old infix-call syntax was removed).
- `---` is the uninitialized-value marker; `--` is not an operator (no increment/decrement operators exist).
- `===` is used only inside `#asm` for register pinning.

### 2.6 Literals

#### Integer literals

- Decimal: `123`, `1_000_000` (underscores may appear anywhere after the first digit and are ignored).
- Hexadecimal: `0x` / `0X` followed by hex digits, e.g. `0xffff_ffff_ffff_ffff`, `0xaBadBabe_aBadBabe`.
- Binary: `0b` / `0B` followed by binary digits, e.g. `0b11`.
- The value must fit in 64 bits; otherwise "Integer literal is too big". Decimal literals in the range 2^63 .. 2^64-1 are accepted and behave as unsigned-only values (they cannot implicitly become a signed type). `-9223372036854775808` parses as a single negative literal.
- An integer literal has no fixed type. Its default type is `s64` (`int`); it converts implicitly to any integer type (or float type, enum, or pointer where allowed) that can represent it (section 5.10).
- `..` directly after a number (`0..7`) is a range, not a decimal point.

#### Floating-point literals

- Forms: `1.0`, `.5`, `2340.`, `1.5e-3`, `1.5e3`, `6.0E+2`. The exponent sign is optional; a capital `E` is accepted. An exponent is only recognised after a decimal point: `1e10` and `6E+2` are **not** floats (they lex as the integer followed by an identifier) and must be written `1.0e10` and `6.0E+2`. There is no `f` suffix (using one is an error, "In this language, we don't suffix our float constants with f."); an `e` not followed by a sign or a digit is the error "'e' in a float literal must be followed by + or - or a numerical digit.".
- Hex-float: `0h` / `0H` followed by exactly 4, 8 or 16 hex digits (underscores allowed) giving the IEEE bit pattern: 8 digits produce a `float32` (`0hff80_0000` is float32 −inf), 16 produce a `float64` (`0h7FF00000_00000000` is float64 +inf), 4 produce a 16-bit pattern reinterpreted through a 32-bit value.
- Default type: `float32` (`float`). The literal is stored with full float64 precision internally. A literal *requires* `float64` (and will not convert to `float32`) if it has more than 8 significant decimal digits after stripping leading/trailing zeros, or if its exponent is out of the float32 range (> 127 or < −126). A literal with exactly 8 significant digits *defaults* to `float64` but still converts to `float32` when the context demands it.
- Float literals convert implicitly to any float type subject to the above. A float literal never converts to an integer type. An integer literal converts implicitly to a float type when the context requires one (`f: float = 1;` is allowed; `f := 1;` infers `int`).

#### Boolean and null

`true`, `false` (type `bool`), `null` (a pointer literal that converts to any pointer type, any procedure type, `Code` (see `#code,null`), and is the default value of those types). `null` used with no target type has pointer type `*void`.

#### String literals

`"..."` on a single line; newlines inside are not allowed. Escape sequences:

| Escape | Meaning |
|---|---|
| `\n` `\r` `\t` `\0` | newline, CR, tab, NUL |
| `\e` | ESC (0x1B) |
| `\"` `\\` | quote, backslash |
| `\xHH` | one byte (two hex digits) |
| `\dDDD` | one byte, decimal (up to 3 digits, ≤ 255), e.g. `"\d010\d013"` |
| `\uHHHH` | Unicode code point, encoded as UTF-8 |
| `\UHHHHHHHH` | Unicode code point (8 hex digits), encoded as UTF-8 |
| `\%` | byte 31 (0x1F); `print` renders it as a literal `%` |

An unknown escape produces a warning and yields the character itself. String literal storage is zero-terminated by the compiler (the terminator is not counted in `.count`), so constant strings may be passed where a C `*u8` is expected (section 5.10). The empty literal `""` has `data == null`. Identical string literals are deduplicated when the executable is written.

#### Here-strings

```
#string IDENT
...verbatim text...
IDENT
```

`#string` (optionally `#string,cr`) followed by an identifier and a newline begins a here-string. The text runs verbatim (no escape processing at all, including `\%`) until a line that, after optional leading whitespace, consists of the terminator identifier followed by a non-identifier character. The trailing newline before the terminator line is included. Line endings are normalized to `\n`; with `,cr` they are normalized to `\r\n`. The here-string is an expression usable anywhere a string literal is (as a call argument spanning lines, as a constant, inside a `#run`...). Its `Code_Literal` carries the `HERE_STRING` flag.

#### Character literals

`#char "x"` is a `u8` constant equal to the first byte of the one-character string literal (`#char" "` without a space is also accepted). There is no dedicated char type.

#### Struct and array literals

See sections 5.7 and 5.8.

### 2.7 Notes

A *note* is `@` immediately followed by an identifier (`@PrintLike`, `@nodoc`, `@test`, `@---`) or by a string literal (`@"a string note with spaces"`). Notes may follow a declaration (after the closing `}` of a procedure or struct body, or after the `;` of a member or enum member) and attach to it. They are available to metaprograms (`Code_Declaration.notes`, `Code_Procedure_Header.notes`, `Code_Struct.notes`) and, for structs and struct members, at runtime through `Type_Info_Struct.notes` and `Type_Info_Struct_Member.notes`. Notes may also be placed directly on an enum declaration (`E :: enum_flags @Hi @There { ... }`). The `@---` member note marks a member that may be uninitialized (used by `print`).

### 2.8 Backtick

`` ` `` immediately followed by an identifier, or by one of the keywords `defer`, `return`, `push_context` or `operator`, marks that token as a *scope modifier*: inside a macro it refers to the caller's scope (section 7.13). Using a backtick outside a macro is an error (except in the `#insert(break=...)` remapping syntax).

---

## 3. Types

Every value has a static type. Types are themselves first-class compile-time values of type `Type` (section 3.10).

### 3.1 Basic types

| Type | Size | Notes |
|---|---|---|
| `s8` `s16` `s32` `s64` | 1,2,4,8 | two's complement signed |
| `u8` `u16` `u32` `u64` | 1,2,4,8 | unsigned |
| `int` | 8 | alias of `s64` (identical type) |
| `float32` | 4 | IEEE 754 |
| `float64` | 8 | IEEE 754 |
| `float` | 4 | alias of `float32` |
| `bool` | 1 | `true`/`false`; any other bit pattern is "invalid" |
| `void` | 0 | zero-sized; values of type void exist (`-> void` returns one void value) |
| `string` | 16 | `{ count: s64; data: *u8 }` |
| `Type` | 8 | a type value; at runtime it is representable as a `*Type_Info` |
| `Any` | 16 | `{ type: *Type_Info; value_pointer: *void }` |
| `Code` | 8 | an AST handle, usable only at compile time |
| `v128` | 16 | 128-bit vector (used with `#asm`) |
| `__reg` | 2 | `#type,distinct u16`; an assembly register handle for macros |

The identifiers `s8 s16 s32 s64 u8 u16 u32 u64 v128 int float float32 float64 string void bool Any Code` resolve directly to these types, bypassing normal identifier lookup (they cannot be shadowed). `Type` is looked up normally (it may be shadowed, which is discouraged).

`Workspace :: s64;` is a plain alias defined in Preload.

Pointers are 8 bytes. Enums have the size of their base type. Procedure values are 8-byte pointers.

### 3.2 Pointers

`*T` is a pointer to `T`. `*void` is the untyped pointer.

- `*expr` (unary `*`) takes the address of an lvalue. It binds to the whole postfix expression: `*r.x0` is the address of `r.x0`; `*array[i]` is the address of an element.
- Dereference: postfix `expr.*` (current), prefix `(.*) expr` (introduced 0.2.004) and prefix `<<expr` (deprecated but accepted). `(.*) cast(*u64) p = 5;` and `(cast(*u64) p).* = 5;` are both lvalues. `cast(T).* expr` (dereference after cast, a warning as of 0.2.006) does not produce an lvalue.
- Member access through a pointer auto-dereferences: `p.x` where `p: *S`. Only **one** level is auto-dereferenced (`pp.x` with `pp: **S` is an error).
- Auto-dereference at call sites: passing `p: *S` to a parameter `s: S` is an implicit dereference (structs only, pointer → value only), with a null-pointer check inserted (subject to `Build_Options.null_pointer_check`).
- `*void` converts implicitly to any `*T` and any `*T` converts implicitly to `*void`. `*[N] T` converts implicitly to `*T` (pointer to first element). Other pointer conversions require a cast.
- `null` converts to any pointer type.
- Pointer arithmetic: `p + n`, `p - n`, `p += n` advance by `n` *elements* (`size_of(T)` bytes) for `*T`; for `*void` the element size is 1. `p - q` (same pointee type) yields the element difference as `s64`; subtracting pointers to types of different sizes is an error; `p + q` is an error. Comparison operators apply to pointers. Bitwise `&`, `|`, `^` accept pointer–pointer and pointer–integer operands without a cast.
- A pointer converts to `bool` in conditions (non-null is true).
- Integer ↔ pointer conversions require an explicit cast; `cast(*void) -1`, `cast(*void) 0` and casting an integer to a procedure pointer type are permitted and are constant expressions.
- Relative pointers (`*~s32`) were removed from the language in 0.1.059.

### 3.3 Arrays

Three array kinds share element access syntax `a[i]`, `.count` and `.data`:

| Kind | Syntax | Layout |
|---|---|---|
| Fixed | `[N] T` | `N` elements inline; `N` is a constant expression; `.count` is a constant; `.data` is the address of the storage (constant for globals) |
| View | `[] T` | `{ count: s64; data: *T }` (16 bytes); a "slice" |
| Resizable | `[..] T` | `{ count: s64; data: *T; allocated: s64; allocator: Allocator }` (40 bytes) |

- `count` and `data` are at the same offsets in `[] T` and `[..] T`, so `*[..] T` may be cast to `*[] T`.
- Fixed and resizable arrays convert implicitly to views. A fixed array of `void` elements and iteration over `[] void` are allowed (zero-sized elements).
- `[0] T` (zero length) is a valid member/type (used by C bindings). Zero-length array literals have `data == null`.
- `.count` and `.data` of a fixed array are **not assignable** (error), even through a pointer; they are assignable for views and resizable arrays.
- A `[N] T` parameter is immutable inside the callee (like structs; see 7.6). Two fixed array types are the same type only if their lengths are equal. A fixed array converts implicitly to a view but not to a different fixed length.
- Multi-dimensional arrays are arrays of arrays: `[4][4] float`, `[691*3] u32`; `a.coef[row]` yields a `[4] float`, which is assignable as a whole from another `[4] float` value.
- Runtime bounds checks are performed on every subscript (`Build_Options.array_bounds_check`, default ON; `#no_abc` on a block or procedure disables them locally). Indexing with a constant index into a fixed array is checked at compile time. Bounds failure calls `__array_bounds_check_fail` and reports "Array bounds check failed. (The attempted index is N, but the highest valid index is M). Site is file:line."
- Assigning one fixed array to another copies all elements. Comparing arrays with `==`/`!=` is an error.
- Casting between arrays: `cast([] T) x` for a view; casting to `[] u8` from `string` and back is allowed (section 5.10); arrays with different element types require `cast,force`; `[N] T` ↔ `[N] U` with `cast,FORCE`. Casting a pointer to `[] T` or `[..] T` is an error.
- The type of the `Allocator` stored in a `[..] T` is used by `array_add` and friends (a `proc == null` allocator means "use `context.allocator`").
- The element type of a constant-declared array may not be `[]`/`[..]` in the type slot of a constant (`X :: ...` with a resizable array type is an error).

### 3.4 Strings

`string` is `struct { count: s64; data: *u8; }`. Strings are byte views; they are not zero-terminated in general (literals are, see 2.6). There is no character type; `s[i]` is a `u8` and is assignable. `.count` and `.data` are assignable. Strings compare by value with `==`, `!=` (compiler built-in, byte-wise) and convert to `bool` (nonempty is true). `for s` iterates the bytes (`it: u8`), forward or reverse. Strings may be initialized like struct literals: `s: string = .{n, ptr};`, `string.{count, data}`.

Conversions: constant string literals convert implicitly to `*u8` and `*s8` (dynamically computed strings do not: "Dynamically-computed strings do not cast to *u8; only literals do."). Explicit casts: `string` ↔ `[] u8` / `[] s8` (both directions; view aliasing), `cast(string) fixed_u8_array` (`[N] u8` → string view over the array), `cast([N] u8) "literal"` (constant), `cast(string) resizable_u8_array`. Note (0.0.091): casting `[] u8` to string *copies* according to the changelog; in practice the standard library treats it as aliasing — orangejuice implements aliasing (view) semantics for `[] u8` ↔ `string` and copying for `[N] u8` → string in constant contexts only.

### 3.5 Structs and unions

See section 8. `struct { ... }` and `union { ... }` types may be anonymous (as a member's type, a variable's type `V : struct { x, y: float; };`, or a struct argument).

### 3.6 Enums

See section 9. `enum`, `enum_flags`, base integer type, values `s64` by default.

### 3.7 Procedure types

`(params) -> returns` with the same syntax as a header without a body, e.g. `(x: int, y: float) -> bool`, `(user_data: *void, session_running: bool) -> bool`, `(*Procedure_Record) -> float` (unnamed params), `() -> ()` (no returns), `#type () -> void #c_call`. A procedure type includes: parameter types (names are informational), return types, the calling convention (`#c_call`), and the flags `#no_context`, `#symmetric`, `#cpp_method`, `#cpp_return_type_is_non_pod`, `#elsewhere`/`#intrinsic`/`#compiler` markers. Default argument values are *not* part of the type (but see the quirk in 7.6). Two procedures differing only in a directive that affects the type are different types; procedure values print as `procedure (args) -> (rets) #c_call`.

`#type expr` forces `expr` to be parsed as a type where the grammar would otherwise be ambiguous (needed for procedure types in value position: `SimpleProcedure :: #type () -> ();`, `PFN :: #type (a: s32) -> void #c_call;`). Procedure types in *type* position (parameter/member types) do not need `#type`: `proc: (x: int) -> int;`, `sa_handler: (sig: s32) #c_call;`. In a type slot a `(` always opens a procedure type, named parameters or not and returning or not — `f: (T)` is a procedure taking a `T`, never a parenthesized `T`.

Varargs (`..T`) belong to the procedure's *calling convention*, not to the parameter type; see 7.4.

Procedure values are pointers; `null` is a valid procedure value; calling a null procedure is a runtime crash (an error at compile time). `#c_call` procedure values may be stored and called through pointers. Polymorphic procedures and macros have **no runtime value** (section 7.8).

### 3.8 `Any`

`Any :: struct { type: *Type_Info; value_pointer: *void; }` (declared in Preload as `Any_Struct`; `type_info(Any)` is its own distinct tag `.ANY`). Any value of any type converts implicitly to `Any`; the conversion stores a pointer to the value's storage (no copy). For rvalues (constants, temporaries, call results) the compiler materializes stack storage. An `Any` assigned to `Any` copies the `Any` (no double boxing). Procedures, arrays, pointers, Types and Code convert to Any. `Any` values cannot be compared with `==`. Untyped struct/array literals and bare unary-dot enum names do not convert to `Any` (error). `x.type.type` is the `Type_Info_Tag`. Casting a `#type` variant to `Any` records the variant's own `Type_Info`.

### 3.9 `Code`

`Code` holds an AST fragment. It exists only at compile time (no runtime representation; storing one in runtime data is an error). Created with `#code expr`, `#code { statements }`, `#code,typed expr`, `#code,null`, `code_of(x)`, `#caller_code`, or implicitly when a non-Code argument is passed to a `Code` parameter of a macro. See section 13.

### 3.10 `Type`

Types are values: `T :: s32;`, `a: Type = float64;`, `thing.type = void;`. A `Type` value is 8 bytes; at runtime it is the address of the corresponding `Type_Info` (`<< cast(*Type)*tiv.variant_of` reinterprets a `*Type_Info` as a `Type`; the official conversion is `Compiler.get_type(*Type_Info) -> Type`). Types compare with `==`/`!=` (`type_of(a) == type_of(b)`, `T == string`) and can be `if ==` switch subjects. `cast(*Type_Info) T` is allowed at compile time on a constant `Type`, inside `#modify` blocks, and in `#compile_time` procedures on non-constant Types. Printing a `Type` yields source-like syntax (`*[17] **void`). A `Type` value may be passed as an `Any`.

### 3.11 Type variants: `#type,distinct` and `#type,isa`

```
Handle   :: #type,distinct u32;      // new type; no implicit conversion to/from u32
Filename :: #type,isa string;        // implicitly converts TOWARD string (base)
Fully_Pathed :: #type,isa Filename;  // chains: converts to Filename and string
Velocity3 :: #type,isa Vector3;      // struct variant; keeps Vector3's operators
Grid3i :: #type,distinct [3] s32;    // indexable; array literals assign
ID :: #type,distinct u64;            // nested inside a struct is fine
```

Rules:

- A variant is a new type with the base type's representation, size and alignment (`Type_Info_Tag.VARIANT`, `Type_Info_Variant { variant_of, variant_flags: DISTINCT | ISA }`).
- `isa` variants convert implicitly to their base (recursively); two isa variants of the same base do not convert to each other. `distinct` variants do not convert implicitly at all. Explicit casts between a variant and its base are always allowed (also for structs). Literals (integer, float, `.{}`, `.[]`) convert to a variant as they would to the base.
- Arithmetic, comparison, bit shifts and operator overloads of the base apply to `isa` variants; a `distinct` variant of a struct cannot use the struct's operator overloads, but a distinct variant of an integer supports integer arithmetic (with same-type operands or literals), `+= 1`, comparison, use as a condition, and use as a `Table` key.
- **Upcast-back rule:** when a call (including an operator overload) implicitly downcasts an isa argument to the base and returns the base type, the result is cast back up to the variant (`pa + pb` on two `Position3` gives `Position3`). Not applied through varargs or macros.
- Variants are dereferenceable with `.` (members of the base struct), convert to `bool` in `if`, are printed/formatted like the base, and overload resolution treats each isa step as one implicit-cast step (same distance as `#as`).
- Anonymous variants can appear inline: `#type,distinct float`, `#type,isa [5] *#type,distinct string`.
- `#type,isa` variants of integers may be used in `#c_call` signatures (ABI fixed in 0.2.007).
- Type restrictions `$T/Filename` do not currently work with variants.

### 3.12 Polymorphic types

`$T` in a type slot introduces a polymorphic type variable (section 7.8). A polymorphic struct `S(a, b)` is a family of types; each distinct argument list (after `#modify`) is a distinct type, deduplicated across the whole program. `Type_Info_Struct.polymorph_source_struct` links an instantiation to its source.

### 3.13 Type equality and identity

Two types are the same if they have the same structure *and* identity: every struct/enum/variant declaration is a distinct type (nominal); pointer, array, and procedure types are structural over their components. `int` and `s64` are the same type; `float` and `float32` are the same type. `type_info(T)` returns the same pointer for the same type (comparison `it.type != type_info(Vector3)` is meaningful). A struct with `using` members has a constants block that participates in identity (fixed in 0.1.007: two polymorph instantiations that are equivalent must dedup).

### 3.14 Sizes and alignment

`size_of(T)` is a constant `s64`, including trailing padding; `size_of` of a value (not a Type) is an error — use `size_of(type_of(v))`. Struct alignment is the largest member alignment (natural alignment: 1/2/4/8; 16 for `v128`; changed by `#align`). Struct end padding is rounded to the alignment. `#align N` on a member, global, stack variable, array literal or constant global data *replaces* the natural alignment and so may lower it as well as raise it (`b: s64 #align 4;` sits at offset 4 and gives its struct alignment 4); `#align` on a constant declaration — a struct declaration included — is an error. `#no_padding` on a struct drops only the *trailing* padding: members keep their natural offsets and the struct keeps its alignment, but its size is not rounded up (`struct { a: u8; b: s32; c: u8; } #no_padding` is 9 bytes, aligned to 4, where the same struct without it is 12). Members are laid out in declaration order; the compiler never reorders members (except `#place`, section 8.6). An empty struct, a `void` member and a `[0] T` member all occupy no bytes, but `[0] T` still aligns to `T`.

`align_forward` and `NewArray(..., alignment)` in Basic provide aligned allocation.

---

## 4. Declarations and scopes

### 4.1 Declaration forms

```
name : Type = value;    // variable with explicit type and initializer
name : Type;            // variable, zero/default-initialized
name := value;          // variable, type inferred from value
name : Type : value;    // constant with explicit type
name :: value;          // constant, type inferred
name : Type = ---;      // variable, explicitly uninitialized
name := ---;            // error: no type
```

- A declaration introduces exactly one name into the innermost enclosing scope (except compound declarations, 4.5).
- `::` declares a **constant**: its value must be a constant expression (section 5.11) — a literal, a type, a procedure, a struct/enum/union definition, an `#import`, a `#run` result, a `#code`, a `#library`, or an expression over constants. Constants have no storage unless their address is taken (`*cube_mesh` where `cube_mesh :: Mesh.{...}` is allowed and yields read-only storage). Taking the address of a "small constant" (a scalar) is an error.
- A variable declared without an initializer is initialized to the type's default value (zero for scalars, `null` for pointers, empty for strings/arrays, the struct's initializer for structs). `= ---` leaves it uninitialized (`Code_Declaration.flags.IS_UNINITIALIZED`).
- A declaration whose type slot is a struct or enum definition and that has no `=` may omit the trailing semicolon: `x: struct { a: int; }` at end of block.
- A trailing `;` after a procedure body or struct/enum body is permitted (`f :: () {};`, `E :: enum { A; };`).
- The right side of a declaration may be a multi-valued call (4.5).
- `name: T = expr` where `expr` is an `ifx` requires a terminating semicolon (0.1.085).

### 4.2 Scopes

Scopes form a tree:

```
Preload scope (compiler-injected declarations, section 17)
  Application (global) scope of the main program        Module scope (one per module instantiation)
    File scope (one per loaded file)                       File scope (one per file of the module)
      Procedure constants block → arguments/returns → body → nested blocks
      Struct: arguments block → constants block → members block
      Enum body
```

Two kinds of scope:

- **Data scopes** (file scope, struct bodies, enum bodies, `#module_parameters` blocks): unordered. Declarations may be referenced before their textual position; mutual references are resolved by the compiler's dependency scheduler. Only declarations and directives are legal; imperative statements (calls, assignments) are errors — except that a struct body may contain assignment statements to set member defaults (8.2) and `#run`/`#assert`/`#if`/`#insert`/`#load`/`#import`/`#placeholder`/`#add_context`/`#scope_*`/`#program_export`/`#poke_name` directives.
- **Imperative scopes** (procedure bodies, blocks, `case` blocks, bodies of `if`/`for`/`while`/`defer`/`push_context`, `#run` blocks): ordered. A variable must be declared before use. **Constant declarations inside imperative scopes must also precede their use** (since 0.1.065; locally declared procedures cannot call each other out of order or recurse through a later declaration). Statements are executed in order.

Blocks: `{ ... }` creates a child imperative scope. `if`, `for`, `while` and `defer` bodies create a child scope even without braces (`if true x := 6;` declares `x` in a child scope). `case` blocks are scopes. `#if` branches do **not** create a scope: declarations inside a `#if` branch land in the enclosing scope (so `#if X { v := 1; } else { v := 2; }` followed by a use of `v` works, and the two branches may declare `v` with different types). Struct literal braces and `#asm` blocks do not create scopes.

### 4.3 Name lookup

Lookup starts in the innermost scope and proceeds outward through enclosing blocks, the procedure body, the procedure's arguments and constants block, then the *file scope of the file containing the reference*, then the module (or application) scope, then Preload. Modules do not see the application scope or each other except through `#import`. Struct member access `x.name` searches only the struct's members block and its arguments/constants blocks (through `using` members, transitively); it does not search outward. Identifiers inside a struct body resolve lexically outward like any other code.

Special cases:

- The basic type names (3.1), `OS`, `CPU`, `IS_CROSS_COMPILING`, `MACHINE_OPTIONS_SIZE` and module parameters resolve directly without waiting for imports that might shadow them.
- Names introduced by `using`, `#import`, `#insert` and compound declarations are subject to a dependency wait: a lookup that could be satisfied by a not-yet-resolved importing construct in the same scope waits for it (see `#placeholder`, 11.8, and `#exists(x, wait_for)`).
- Identifier lookup does not wait on unfinished `#insert`s that have no declared placeholder; order-dependent results are possible — use `#placeholder`.
- **Redeclaration**: declaring the same name twice in one scope is an error ("redeclared identifier"), including names brought in by `using`/`#import` that collide. Overloaded procedures are the exception (7.7). A file-scope declaration may shadow an application-scope one from another file; two application-scope declarations of the same name in different files are an error.
- **Shadowing** in imperative scopes: a nested block may declare a name that shadows an outer local, a parameter, a global or an imported procedure (`length := ...` shadows `Math.length`; `dot := dot_product(a, b)`; `result := ...` inside a `for` body shadowing an outer `result`). Using a name and then redeclaring it later in the *same* scope via a compound declaration is an error (0.1.063). A `for` loop's `it`/`it_index` shadow outer ones.
- A parameter or `using`-imported name may be shadowed by a local variable only in a nested block ("using shadows a parameter" errors exist for the same scope).

### 4.4 Scope directives (data scopes only)

```
#scope_export;   // default at file start: declarations go to the module/application scope and are exported
#scope_module;   // visible to every file of the module, not exported (in the main program = #scope_export)
#scope_file;     // private to this file
```

The directive applies to all following declarations in the file until the next scope directive; they may alternate any number of times. The `;` is optional. `#load`ed files always get their own file scope. `using` adds names to the *currently active* scope (file or module/export); an unnamed `#import` is the exception — its names reach the whole module whatever directive is in effect (11.2), and only the binding a *named* import declares is subject to `#scope_file`. `#insert` at toplevel obeys the active scope directive. A `#placeholder` is filled by an `add_build_string` targeted at the same scope.

A `#if` branch does not create a scope, so a `#scope_*` written inside one keeps applying after the branch ends — the modules that do this (`#scope_file` … `#scope_export` inside a platform branch) restore the previous directive themselves.

### 4.5 Compound (comma-separated) declarations and assignments

```
x, y, z: float;                       // three variables of one type
a, b := 1, 2;                         // two variables, two values
i, j = x+1, y+1;                      // two assignments; all RHS evaluated first, then assigned (so a, b = b, a; swaps)
mask, lower, upper : u32 = 0xffffffff; // one RHS broadcast to all names
p0, p1, p2, p3 := pos;                // broadcast in a declaration
a, b += 1;                            // compound op broadcast (works with heterogeneous types if the literal fits)
r, s := f();                          // f returns two values
ok : bool; r, ok = f();               // assignment from multiple returns
array[i], array[i+1] += get_u8s();    // compound assignment from a multi-return call, arbitrary lvalues
v.x, v.y, v.z = 1, 4, 9;
```

Rules:

- The number of names must equal the number of values, or there must be exactly one RHS value that is broadcast, or the single RHS is a call returning exactly that many values. A call returning *one* value cannot be split across two names (error), and a call returning more values than names is allowed only for a single receiver (extra values dropped, subject to `#must`).
- Per-name modifiers mix declaration and assignment in one statement: in a declaration `a, b=, c := 1, 2, 3;` the `b=` means "`b` already exists; assign to it"; in an assignment `a, d:, c = 4, 5, 6;` the `d:` means "declare `d` (type inferred)". Thus `success=, pdb := parse_pdb(...)`, `target_triple, target_triple_with_sdk= := get_android_target_triple(cpu)`, `line_info, success= := decode(...)`, `reference_space_types:, result = xrEnumerate(...)`, `view:, result = make_image_view(...)` (inside a loop, `view` is declared each iteration).
- Member targets: `anim.joints.count, ok = parse(...)`, `renderer.vma, result = make_vma(...)`, `it.local_position, it.local_orientation, it.local_scale = decompose(m);` (may span lines), `text.* = ...` / `result, success, text.* = string_to_int(<<text)` (dereference lvalues).
- Exported to metaprograms as `Code_Compound_Declaration { comma_separated_assignment; declaration_properties; operator_type (0 = declaration, else the assignment operator) }` with each argument carrying a `Code_Comma_Separated_Argument.modifier` of `NONE`, `DECLARE` or `ASSIGN`. Multi-return extraction uses `Code_Extract { from; index }`.
- A compound declaration does not block name lookups for names it does not define.
- Compound declarations may appear in struct bodies (`x, y, z: float;`, `vao, vbo, ibo: GLuint;`, `w = 1;` afterwards to set a default) and with `#place`.

### 4.6 The discard identifier `_`

`_` is always declared. It may appear as a target in declarations and assignments (`_, name := path_decomp(s);`, `a, _, _, d = f();`) and as the name of a declaration. Evaluating `_` as an rvalue is an error. `__` (two underscores) is an ordinary identifier.

### 4.7 Global variables

A variable declared in a data scope is a global with static storage. Globals are initialized from constant data (their initializer must be a constant expression or a struct/array literal thereof; `x := string.[]`, `ints: [] int = .[1, 2, 3];`, `test_pointer1 := TEST_LITERAL.data;`, `foozle := *global_buffer[10];`), or by `#run` mutation before the executable is written. Zero-valued globals go to BSS; nonzero constant-initialized data goes to DATA; string/array/struct literals and `Source_Code_Location`s go to the READ-ONLY segment (writing to them crashes at runtime; the compiler rejects direct assignment to elements of a constant literal). At the end of compilation, all writable global data is **reset** to its declared initial values before the executable is written; `#no_reset x: T;` keeps the values written by `#run`s (pointers in `#no_reset` data are not remapped, except constant pointers to known globals). `#no_reset` on struct members is an error.

Global data at compile time lives in the compiler's process; constant pointers to globals are remapped between compile time and runtime (`P :: *some_float; << P *= 2;` works at runtime).

### 4.8 Declaration directives

Placed before the declaration or between its parts:

- `#no_reset decl;` — see 4.7.
- `#align N decl;` / `member: T #align 64;` — alignment (constant integer N).
- `#program_export decl;` / `#program_export "name" decl;` — export the symbol from the executable/library with its own name or an explicit name; also on data declarations. Not allowed on constants or things without a concrete representation.
- `name: T #elsewhere lib;` / `name: T #elsewhere lib "symbol";` — the storage is defined in another object/library (extern data); on procedures, `#elsewhere lib` declares an externally defined procedure with the *Jai* calling convention (a `#program_export` from a Jai DLL); since 0.1.077 the library identifier is optional (symbol resolved at link time, not callable at compile time).
- `#as`, `using` — on struct members and parameters (8.4).
- `#discard` — on procedure parameters (7.3).
- `$name`, `$$name` — on procedure parameters (7.8).
- `#must` — on return values (7.2).
- `#specified`, `#complete` — on enums (9).
- `#deprecated "msg"` — on procedure declarations.
- `#type_info_none`, `#type_info_procedures_are_void_pointers`, `#type_info_no_size_complaint`, `#no_padding`, `#foreign` — on struct declarations (8.7).
- `#dump` — after a procedure header (or `#modify`): print the procedure's bytecode at compile time.
- `#placeholder name;` — declares that `name` will be provided later (11.8).
- `#poke_name Module name;` — inject a name into another module's scope (11.9).
- `#add_context name: T = v;` — add a member to the Context type (10.2).
- Notes `@x` — after the declaration.

---

## 5. Expressions

### 5.1 Operator precedence

From highest (tightest) to lowest. The parser uses precedence climbing (0.1.082).

| Level | Operators | Notes |
|---|---|---|
| postfix | `a.b`, `a[i]`, `f(args)`, `a.*`, `a.(T)`, `a.(T, mods).*`, `S.{...}`, `T.[...]`, `#insert` in `a.(#insert y)` | left-to-right |
| prefix | `-` `!` `~` `*` (address) `<<` `(.*)` `cast(T)` `cast,mods(T)` `xx` `xx,no_check` `..` (spread) `inline`/`no_inline` (call) `#run` `#code` `$`… | Unary `~` has the same precedence as unary `-`: `~a & b` is `(~a) & b`. `-Thing.{5}` is `-(Thing.{5})`. Prefix `cast(T) e` and `xx e` bind to the single following unary/postfix operand: `cast(u64) 1 << 63` is `(cast(u64) 1) << 63`, `cast(*u8) p + off` is `(cast(*u8) p) + off`, `xx f() * 0.1` is `(xx f()) * 0.1`, `cast(s32) (a + b) / 8` is `(cast(s32)(a+b)) / 8`. Prefix `<<` binds tighter than `&`/`\|`: `<<x & y` is `(<<x) & y`. |
| multiplicative | `*` `/` `%` | |
| additive | `+` `-` | |
| shift | `<<` `>>` `<<<` `>>>` (with optional `,small` / `,logical` modifiers) | |
| bitwise and | `&` | binds tighter than comparison: `mode & S_IFMT == S_IFDIR` is `(mode & S_IFMT) == S_IFDIR` |
| bitwise xor | `^` | |
| bitwise or | `\|` | |
| comparison | `==` `!=` `<` `<=` `>` `>=` | non-associative in practice |
| logical and | `&&` | short-circuit |
| logical or | `\|\|` | short-circuit |
| `ifx` | `ifx c then a else b` | lowest expression form |
| assignment (statements) | `=` `+=` `-=` `*=` `/=` `%=` `&=` `\|=` `^=` `<<=` `>>=` `<<<=` `>>>=` `&&=` `\|\|=` | statements, not expressions |

Parentheses group. An expression statement that consists solely of a binary `<<` at statement start is parsed as a shift, so `<<p = 3;` must be written `(<<p) = 3;`, `p.* = 3;` or `(.*) p = 3;` — `if x <<pointer = 3;` is a parse error that the compiler diagnoses specially. `#if` followed by a block followed by a unary `<<` may need a `;`.

The exact binding strength of bitwise `&`/`^`/`|` relative to each other and to comparison follows the table (and matches usage `flags & (A | B)`, `(a & b) != 0`, `if !(x & BIT) || !(y & BIT)`).

### 5.2 Arithmetic and integer semantics

- Binary `+ - * / %` on integers of the same type produce that type. Mixed integer types are unified by the Match rules (5.10): the smaller operand widens if the larger type can represent it. Signed/unsigned mismatches that could lose information are errors ("Number sizes don't match", signedness info).
- `/` on integers truncates toward zero; `%` has the sign of the dividend. Division by zero is a runtime crash (SIGFPE).
- Overflow: wraps two's-complement by default. `Build_Options.arithmetic_overflow_check` (`.OFF` default, `.NONFATAL`, `.FATAL`) inserts checks for `+ - *` that call `__arithmetic_overflow`; `#no_aoc` on a block/procedure disables them. Constant-expression overflow is a compile-time error.
- Unary `-` on an unsigned value is an error ("can only negate a signed number"). `-` on a literal out of the s64 range is detected.
- Float arithmetic follows IEEE 754; `%`, `&`, `|`, `^`, `~` on floats are errors. `float32` → `float64` is an implicit widening; `float64` → `float32` requires a cast. In an *arithmetic* operation the two widths meet at the wider one (`float64 * float32` is `float64`) and a runtime integer widens into the float (`f * j` and `f / j` are `float32`, `f += j` is fine) — measured with the reference; a *comparison* between them is still the error "Number mismatch", and so is `x: float = j`.
- Shifts: `a << n`, `a >> n` (arithmetic for signed, logical for unsigned), `a >>,logical n` (logical even when signed), `a <<,small n` / `a >>,small n` (skip the shift-width safety check; only meaningful for non-constant `n`). Shifts by an amount ≥ the operand width produce a *defined* result (0 / sign bits) at a small runtime cost unless `,small`. Shift amount may be any integer type; the result type is the left operand's type. `1 << 63` fits in u64. Shifting an enum value by an integer yields an integer constant. `<<<` / `>>>` are bitwise rotates. Variants accept shifts.
- Bitwise `& | ^ ~` on integers, enums and enum_flags (of the same type or with literals), booleans (constant-folded), and pointers (3.2).
- Bitwise `& | ^ ~` on integers, enums and enum_flags (of the same type or with literals), booleans (constant-folded), and pointers (3.2). Unlike arithmetic, a bitwise operator whose **left** operand is an explicit `cast` keeps that cast's type instead of widening: with `a`, `b: u64`, `cast,trunc(u32) a ^ b` is a `u32` (so is `&` and `|`), while `cast,trunc(u32) a + b` is a `u64`, and so is `v ^ b` for a `v: u32` variable. The cast still binds to the operand, not to the whole expression — `cast,trunc(u32) a - b` with `a == 0, b == 1` is `0xFFFF_FFFF_FFFF_FFFF`.
- Logical `! && ||` on `bool`; `&&`/`||` short-circuit and require `bool` operands (or values with truth value — see 5.9); `&&=`/`||=` short-circuit as well.
- `#type,isa`/`distinct` integer variants participate as described in 3.11.

### 5.3 Member access and `using`

`a.b` accesses a struct member, a struct parameter (`holder.T`, `floats.N`, `table.LOAD_FACTOR_PERCENT`), a struct constant (`it.kind.DECLARATION` reaches an enum type through a *value's* type; `ts.Overflow_Page` names a nested type through a value), a nested type of a struct type (`Renderer.Swapchain`, `Summary.Variable`, `Skeleton_Pose.Joint`), an enum member (`Fruit.APPLE`), a member of a named import (`Math.length`, `Simp.Texture`), a member of a Type value's struct (`Thing.numbers.count` is constant for fixed arrays), `.count`/`.data`/`.allocated`/`.allocator` of arrays and strings, `.type`/`.value_pointer` of `Any`, and `.type` of a constant `Code`. `Enum.loose` names the enum's underlying integer type (a type that allows out-of-range values). Type-valued expressions cannot be dereferenced with `.` except for enum members and struct constants/nested declarations; accessing a non-constant member through a struct *type* is an error.

Swizzle-style members (`v.xy`, `v.xyz`, `m._11`, `m.floats[i]`, `q.component`) are ordinary members provided by `#place`/union layout in the Math module, not language features.

### 5.4 Subscripts

`a[i]` for arrays, strings, pointers (`p[i]` indexes as an array), `[N] T` values through a pointer (`(<<border)[k]`), and user types via `operator []`. The index must be an integer type (enum values are accepted as indices — `border[Border_Direction.WEST]`); a string index is an error. Fixed-array element assignment via a pointer to the array uses `(<<p)[i]` or `p.*[i]`.

### 5.5 Calls

`f(a, b, name = c, ...)`. Rules (see 7.4 for parameters):

- Positional arguments first, then named arguments (`name = value`) in any order. A positional argument that follows a named argument fills the first parameter (in declaration order) that has not been filled yet; when the named argument is a varargs parameter, following positional arguments continue filling that varargs parameter until another name appears (`v = 1, "are", "you", i = 3`). This covers `visit_files(args.path, recursive=true, symbols, visitor)` and `log("...", action, flags = .WARNING)`. An argument given twice is an error. Trailing commas are allowed.
- A varargs parameter receives all remaining positional arguments; `..array` spreads an array (or fixed array) into a varargs parameter; `..` on an array literal (`.. .["a", "b"]`) is allowed. Spread is exported as `node_flags.EXPRESSION_IS_SPREAD` and produces a `Code_Make_Varargs` node.
- `,,` **context arguments**: `f(a, b,, allocator = temp, logger = my_logger)`. The second list names members of `#Context` to set for the duration of the call (a local `push_context`); an unnamed expression in the `,,` list means `allocator = expr`. When only `allocator` is pushed and its `proc` is `null`, nothing is pushed. Arguments are evaluated before the context change. The LHS of a `,,` assignment must be a simple identifier. Exported as `Code_Procedure_Call.context_modification`.
- `inline f(...)` and `no_inline f(...)` force/forbid inlining of that call (bytecode inliner). `f(...) #no_debug` suppresses debug stepping into a macro call.
- `#procedure_of_call f(args)` yields the concrete (possibly polymorphed) procedure that the call would invoke, as a constant.
- Calling through a procedure-typed member: `source.process(app, source)`, `platform.poll(...)`, `vm.*.AttachCurrentThread(vm, ...)`, `persistent_allocator.proc(.IS_THIS_YOURS, ...)`.
- A call expression has the type of the procedure's **first** return value (or `void`). Extra return values are accessible only through compound assignment/declaration. `#must` return values must be received (checked at code generation, so passing through macros is fine).
- Macro calls (`#expand`) and polymorphic struct instantiations (`Table(K, V)`) share the call syntax and the `Code_Procedure_Call` node.
- Passing a polymorphic procedure or lambda where a concrete procedure type is expected instantiates it for that type (7.8). Passing an overload set stores a `Code_Resolved_Overload`.

### 5.6 Casts

Three equivalent syntaxes (all accepted; the function-call style is the planned future default):

```
cast(T) expr                    // prefix; binds to the following unary/postfix operand only
cast,no_check(T) expr           // modifiers after "cast,"
cast,trunc(u32) a ^ b           // = (cast,trunc(u32) a) ^ b
cast(T, expr)                   // function style
cast(T, expr, trunc)            // modifiers as trailing arguments
cast(T, expr, trunc).*          // postfix dereference after cast
expr.(T)                        // postfix
(p + off).(*u8, trunc).*
cast(T).* expr                  // dereference-after-cast (warning; going away)
xx expr                         // autocast to the type required by context
xx,no_check expr
```

Modifiers:

| Modifier | Meaning |
|---|---|
| (none) | value-checked conversion: numeric conversions verify the value fits at runtime (`Build_Options.cast_bounds_check`: `.FATAL` default, `.NONFATAL`, `.OFF`); failure calls `__cast_bounds_check_fail` ("Cast bounds check failed.  Number must be in [lo, hi]; it was V.  Site is file:line.") |
| `no_check` | skip the runtime range check |
| `trunc` | truncate to the target width (no check); required intent marker when narrowing (e.g. `cast,trunc(u32) h ^ (h >> 32)`) |
| `force` | reinterpret between struct types (or arrays) of the **same size** |
| `FORCE` | reinterpret a struct as a **smaller** struct / fixed array as a different fixed array |

Redundant or inconsistent modifiers (e.g. `trunc` with `no_check`) are errors. Allowed casts: numeric ↔ numeric (int/float/bool/enum in all combinations; `cast(bool) x` is "x != 0"; float → int truncates and range-checks; bool → int gives 0/1; bool → float allowed; int → enum unchecked value; enum → int), pointer ↔ pointer, pointer ↔ integer (`cast(u64) ptr`, `cast(*void) -1`, `cast(*u64) p`), integer → procedure pointer, procedure → `*void`, `*Type_Info` ↔ `Type` (compile time), `Any` from anything (`cast(Any) v`), `string` ↔ `[] u8`/`[] s8`, `[N] u8` → `string`, `"literal"` → `[N] u8`, arrays to views, `*[N] T` → `*T` implicitly, struct ↔ struct with `force`/`FORCE`, `[] T` ↔ `[] U` with `force`, variant ↔ base, `cast(Windows.BOOL) true` (bool → enum), `cast([] string) x` (identity). Errors: pointer → `[] T`/`[..] T`, `+`/`~` on void, casting to void, casts to `Any` of untyped literals.

`xx` autocasts to whatever type the context requires (assignment target, parameter, member, comparison operand, return) and is subject to the same checks as `cast` (e.g. struct→struct needs `force`). In a declaration without a type slot (`x := xx y;`) the autocast is a no-op and `x` takes the type of `y` (allowed since 0.1.061). `xx` is not allowed on the expression value of a quick lambda. When matching against a polymorphic `$T`, auto-casts are stripped. `xx` always exports a `Code_Cast` with `cast_flags.IS_AUTO` and a known `type`.

Implicit casts (5.10) are recorded as `Code_Cast` nodes with `IS_IMPLICIT`; auto-dereference uses `IS_POINTER_DEREFERENCE`; `#as` conversions use `AS`; `isa` conversions use `ISA`.

Constant expressions: casts of constants are constants (5.11); `cast([5] u8) "Hello"`, `cast(*u8) *global_int`, `cast(#type () -> int) 0` are constant; `cast(s64) pointer` and `cast(string) fixed_array` (in general) are not.

### 5.7 Struct literals

```
Vector3.{1, 2, 3}                 // positional: exactly as many values as settable members, in order
Vector4.{w = .7}                  // named: any subset; others take defaults
.{x = 1, y = 2}                   // type inferred from context (declared type, parameter, member, return, comparison)
.{}                               // all defaults
Allocator.{allocator_proc, null}  // procedure and pointer values
string.{count, data}              // strings and array views ([] T) are struct-literal-able
Renderer.Buffer.{size = xx n}     // nested type
XrPosef.{orientation = .{w = 1}}  // nested inferred literal
.{color = .{_float32 = .[0, 0, 0, 0]}}  // union member selection + array member literal
Model.{ name = "x", mesh = *cube_mesh, transform = Matrix4_Identity }
```

Rules:

- A literal is *constant* if every value is a constant expression (then it may be a global/`::` initializer, a default argument, a `case` value... no: case values must be scalar constants). Non-constant struct literals are allowed only in imperative scopes (procedure bodies) and in `#run` results.
- Named form may use any assignment legal in a struct body: `values[1] = 7`, a name imported through `using`, a member of a union.
- Positional literals for a struct with `using` base members write the base as a nested literal: `Mangled_Type_Info_Float.{.{.FLOAT}, 4}`.
- Members declared `= ---` in the struct are left uninitialized only when not mentioned. Holes (padding) are zero-initialized.
- A literal with no type designation (`.{...}`) gets its type from the Match against the expected type; it can also match a polymorphic variable in a header when `#modify` assigns the type (0.2.006). Struct literals for polymorphic structs without an argument list use the struct's default parameters.
- `-Thing.{5}` negates the literal (operator overload).
- Struct literals in global data have holes zeroed and pointers to other globals resolved. Several literals may be deduplicated by the compiler (not ones containing pointers, except compiler-generated `Source_Code_Location`s).
- Exported as `Code_Literal { value_type = .STRUCT, struct_literal_info: { type_expression (null when undesignated), arguments } }`.
- A struct literal can be `push_context`ed (`push_context .{allocator = temp} { }`) and used as a `using` source (`using Ice_Cream.{...};`).

### 5.8 Array literals

```
.[1, 2, 3]                       // element type from context (e.g. a [] s8 parameter)
int.[1, 3, 5]                    // typed; the literal's type is [3] int (fixed), converting to [] int
string.["a", "b"]
(*u8).[a.data, b.data]           // parenthesized type before .[
*u8.[a.data, b.data]             // the same: a `*` before a literal's type designation belongs to the type, not to the literal
(*Model).[*models[0], *models[1]]
([3] float).[.[0,0,0], .[1,1,1]] // nested
Level_Config.[ .{n = 2}, .{n = 3}, ]   // trailing comma allowed; comments between elements
.[]                              // empty; data == null
(*u8).[]                         // empty typed: type [0] *u8
D3D_DRIVER_TYPE.[.HARDWARE, .WARP]     // unary-dot elements
.[tprint("-L%", p), "-lc++"]     // non-constant elements (imperative scopes only)
```

- The literal's type is `[N] T` (fixed). It converts implicitly to `[] T`. `.count` of any array literal is constant; `.data` is constant only if all elements are constant.
- An array literal is constant if all elements are constant; non-constant literals are allowed in procedure bodies (0.1.069) and as `#run` results, not in data scopes.
- Constant literals live in read-only memory; assigning to an element of a *constant-declared* literal is a compile error; writing through a view at runtime crashes.
- Element type inference is downward: `f(.[1, 2, 3])` for a `[] s8` parameter checks each element against `s8`.
- `#align N` applies to constant array literals in global data.
- Exported as `Code_Literal { value_type = .ARRAY, array_literal_info: { element_type, alignment, array_members, array_literal_flags.WAS_UNTYPED } }`.
- `..` spreads a literal into varargs.

### 5.9 Conditions and truth values

`if`, `while`, `ifx`, `!`, `&&`, `||`, `assert` and `cast(bool)` accept any value with a *truth value*:

| Type | true when |
|---|---|
| `bool` | `true` |
| integers, enums, enum_flags, variants of them | nonzero |
| floats | nonzero (negative zero is false; NaN is true) |
| pointers, procedure values | non-null |
| `string`, `[] T`, `[..] T` | `count != 0` |
| `Code` | not `#code,null` |
| `Type`, structs, arrays `[N] T`, `Any`, `void` | **error** ("cannot implicitly coerce to bool"); structs never have a truth value, even with an `operator ==` |

Comparison results are `bool`. `if x` on an integer `x` is exactly `if x != 0`.

### 5.10 Implicit conversions and type inference

Type inference is bottom-up per statement (leaves to roots) with a small set of downward flows. The core operation is *Match(a, b)*:

- **One side fixed** (declaration with a type, parameter, member, return, assignment target): the value is converted to the fixed type if an implicit conversion exists; else error "Type mismatch. Type wanted: X; type given: Y." (with call-site detail: "in checking argument 2 of call to f").
- **Both flexible** (binary operator): the operand types are unified: if one is an untyped literal it takes the other's type; integers of different sizes unify to the larger if it can hold the smaller's range (`Match(s32, u8) = s32`, `Match(u8, s16) = s16`); an implicit cast node is inserted on the converted operand.

Implicit conversions (in the fixed-target case), each recorded as a `Code_Cast` node:

1. Integer literal → any integer type that can hold the value; → any float type; → any enum/enum_flags type (0.0.082: `x + 1` where `x` is an enum; `flags = 0`). Integer literals do not convert to pointers (only `null` does). Literal typing is exact: `foo : s32 = 0x0001_0203_0405_0600;` is a "loss of information" error. Since 0.1.084, small hex literals are not sign-extended in constant comparisons (`x == 0xffff_ffff` with `x : s64 : -1` is false).
2. Constant declared with `::` of numeric type behaves like a literal of its value (adapts to the expected type if it fits), except when the constant was declared with an explicit type bound (then the declared type is used for casting distance).
3. Integer variable → wider integer that holds the full range (`u8 → u16/s16/…`, `u32 → s64`, `s32 → s64`); not `s8 → u64`, not narrowing. A runtime integer does not convert to a float in a *fixed-target* position (`x: float = j` is an error) but does as an arithmetic operand (5.2).
4. `float32` → `float64`. Float literals: see 2.6.
5. `*T` → `*void`, `*void` → `*T`, `null` → any pointer/procedure, `*[N] T` → `*T`, `*Derived` → `*Base` when `Derived` has `#as` for `Base` (8.4), `*T` → `T` at call sites (auto-dereference of struct arguments).
6. `[N] T` → `[] T`; `[..] T` → `[] T`. Not `[] T` → `[..] T` (use `resizable()`).
7. `T` → `Any` (everything). Untyped literals and unary-dot enum names are not allowed.
8. `S` → member type `M` when `S` has a member `#as m: M` (value and pointer forms), transitively; `#type,isa V` → base, transitively; `#Context` → `Context_Base` (its `base` member is `#as`).
9. String literal (constant) → `*u8`, `*s8`.
10. Unary-dot enum name `.X` → the enum type demanded by context (5.12).
11. Untyped `.{}` / `.[]` literal → the struct/array type demanded by context.
12. Polymorphic procedure / quick lambda → concrete procedure type (7.8).
13. Struct literal `Vector4.{...}` → variant of that struct.

No implicit conversions exist between distinct structs, between enums, from enum to integer variables, from integer to bool (use `cast(bool)`, `!`, or a condition), from `bool` to integer, from a runtime `string` to `*u8`, or from a *computed value* to a `#type,distinct` variant. A **literal** does convert to a variant of the type it is a literal of — `a: Handle = 5` on `Handle :: #type,distinct u32`, and `"Hello"` passed where a `#type,distinct string` is wanted — which is the point of the variant: the type safety is against values the program computed.

**Downward flow exceptions** (types pushed from the root to the leaves after bottom-up fails): integer literals; unary-dot enum identifiers (an "unknown enum" placeholder type propagates through `|`, `&`, `~`, `ifx`, binary operators and is resolved in a post-pass — `d = .WEST | .EAST;`, `d &= ~.SOUTH`, `flags = ifx c then .A else .B`); `.{}`/`.[]` literals; `null`.

**Binary operators with a literal:** in a binary operation between an integer expression and a *float literal* (`level_index * 1.2`, `ms / 1000.0`, `(w - pw) / 2.`), the integer operand is converted to the literal's float type and the result has that float type. Between a runtime integer and a runtime float the arithmetic operators widen the integer and the result is the float type (`f * j`, `d + j`); a *comparison* between them is still an error, and so is a fixed-target assignment. `u64 % u32` widens to `u64`. `float32 * float64` is `float64`.

Return types are never inferred from a body (except for quick lambdas). Parameter types are fixed by the declaration; a parameter declared `name := "Hello"` is a `string` parameter with a default.

### 5.11 Constant expressions

An expression is *constant* if it can be evaluated at compile time without executing user code (except `#run`). `is_constant(expr)` reports this without evaluating `expr` (nested `#run`s still run). Constant:

- literals of all kinds; `null`; `true`/`false`; here-strings; `#char`
- unary and binary operators applied to constants (including comparisons, `&&`/`||`, casts of numeric constants, integer/pointer arithmetic on constant pointers: `K + 2`, `(7 + p)`); `#ifx` on constants; simple `ifx` on constants
- `::` declarations of any type, including procedures, types, struct literals of constants, `#import` bindings
- `.count` and `.data` of a constant string or constant array literal; `[i]` of a constant string/array; `.count` of a non-constant fixed-array literal or of a fixed-array member (`Thing.numbers.count`); `global_fixed_array.data` and `global_fixed_array.data + offset`; `*global_variable` and `*global_var.member`? (the latter is *not* currently constant); `*global_int` cast to `*u8`; `TEST_LITERAL.data`; `.data` of an empty array literal (null)
- `type_of(...)`, `size_of(...)`, `type_info(T)` (constant since 0.1.090), `type_info(T).type`, `type_info(T).runtime_size`, `initializer_of(T)`, `is_constant(...)`, `code_of(...)`, `#location(...)`, `#caller_location` (a struct literal), `#this`, `#procedure_name(...)`, `#file`, `#filepath`, `#line`, `#exists(...)`, `#Context`, `#type ...`, `#run ...`, `#procedure_of_call ...`, `#bake_*`, `#code ...`, `OS`, `CPU`, `IS_CROSS_COMPILING`
- constant struct literals, constant array literals, `cast([N] u8) "..."`, casts to `#type` variants of constants, casts of integer constants to pointers/procedure types
- enum members, struct constants and parameters accessed through a Type or instance (`Holder.N`, `holder.N`)
- `$` baked parameters and polymorphic type variables inside the instantiation

Not constant: `:=` variables, `*local`, `cast(s64) ptr`, pointer − pointer, `P[i]`/`<<P` of a constant pointer, members of a non-constant struct literal, `.data` of global `[]`/`[..]` arrays, `#compile_time`, `context.*`, calls to procedures (unless wrapped in `#run`), `#insert`.

`#run`/`#assert` on an expression that is already constant does not execute bytecode.

### 5.12 Unary-dot enum names

`.NAME` refers to the member `NAME` of an enum type determined by context: assignment/declaration targets with a known enum type, parameters and default values, struct members and literal fields, `return` values, comparison operands (`x == .A`, `.A == x`), `case` values in a switch on an enum, array literal elements of an enum type (`Stuff.[.A, .B]`), `cast(E) .X`, `xx .X`, polymorphic struct arguments, and the operands of `|`, `&`, `^`, `~`, `ifx` and binary operators whose other side has the enum type (`Flags.A | .B | .C`, `~.HUNGRY`, `.TRANSFER_DST_BIT | buffer_usage`, `.VERTEX_BIT | .FRAGMENT_BIT` as a member value, `Allocator_Caps.X|.Y|.Z`). Not allowed where no type is available (`z := .A;` is an error; `.THIRD + 1` in an untyped arithmetic subexpression is an error). Unary-dot lookup does not search the enum's parent scopes for the name. The `#if` condition may use unary dots against `OS`/`CPU` (`#if OS == .WINDOWS`).

### 5.13 `ifx`

```
v := ifx cond then a else b;      // both branches converted to a common type (implicit casts; Any unifies anything)
v := ifx cond a else b;           // "then" may be omitted
v := ifx cond then a;             // no else: else-value is the default value of type_of(a) (struct defaults included)
v := ifx cond else b;             // no then: value is cond itself; if cond is `!x`, `x >= y`, or `f(x, y)`, the implicit then-value is x / x / x (the innermost left operand), one level deep; for `a.b` the then-value is the bool `a.b`
v := ifx cond { stmts; last_expr; } else { stmts; expr; }   // block form: the value is the last expression statement of each block (a declaration cannot be the final clause)
timeout := cast(s32) ifx a != b && !c then -1 else 0;        // ifx binds looser than everything; a cast applies to the whole ifx
x := ifx renderer.sample_count == renderer.max_sample_count
        then ._1_BIT
        else renderer.max_sample_count;   // multi-line
```

An `ifx` used as an expression is not followed by its own semicolon (it is part of the enclosing statement). `return ifx ...;` is fine. Branches of `void`/zero-sized type are allowed. `ifx` with an unknown-enum branch resolves against the other branch. Exported as `Code_If` with `if_flags.IS_IFX`.

### 5.14 Compile-time query expressions

| Expression | Value |
|---|---|
| `type_of(expr)` | the static type of `expr` (not evaluated); `type_of(context)` is `#Context`; `type_of(x.member)`; `type_of(Build_Options.output_type)` names a member's type through the struct type; `type_of(T.loose)`; `type_of(poly_proc)` only at compile time |
| `size_of(T)` | size in bytes (constant `s64`); argument must be a type |
| `type_info(T)` | `*Type_Info_xxx` for type `T`, statically typed as the specific `Type_Info_*` struct when `T` is a known struct/enum/procedure/pointer/array/integer type, else `*Type_Info`; constant; `type_info(type_of(v))` for a value |
| `initializer_of(T)` | the struct's initializer procedure `(memory: *void) #no_context` or `null` if it is all zeros |
| `is_constant(expr)` | `bool` constant |
| `code_of(expr)` | `Code` for the expression after constant substitution (a declaration's name gives its definition: `code_of(factorial)`, `code_of(string_a)`) |
| `#run expr` | evaluate at compile time; the result becomes a constant (section 12) |
| `#location()` / `#location(expr)` / `#location(#this)` | `Source_Code_Location` struct literal (constant) of the current position / of `expr`'s position (`#location(code)` for a Code); `#location().line_number` |
| `#caller_location` | as a parameter default: the call site's `Source_Code_Location` (constant per call site); for macros, the macro invocation site |
| `#caller_code` | as a `$` parameter default of a macro: the `Code` of the entire call expression at the call site |
| `#file` | the full path of the current source file (string) |
| `#filepath` | the directory of the current source file, with trailing `/` |
| `#line` | the current line number |
| `#this` | the enclosing procedure (as a procedure constant), the enclosing struct (as a type), or the enclosing data scope; inside a macro, the procedure the macro is expanded into; not allowed in procedure headers or struct argument lists |
| `#procedure_name()` / `#procedure_name(p)` | the compile-time name (string) of the current procedure (after macro expansion) / of procedure `p`; error outside a procedure |
| `#exists(a)` / `#exists(a.b.c)` / `#exists(a, wait_for)` | `bool` constant: whether the whole dotted chain resolves (through pointers and variants; sees backticked variables); with `wait_for`, evaluation waits until `wait_for` is declared |
| `#compile_time` | `bool` (not a constant expression!) — true when the code is executing at compile time; folded when the answer is known |
| `#Context` | the type of the context struct |
| `#procedure_of_call f(args)` | the concrete procedure a call resolves to |
| `#bake_arguments f(x = 1)` | a new constant procedure with the given arguments baked (7.10) |
| `#bake_constants f(T = int)` | a polymorphic procedure with its polymorph variables bound |
| `#dynamic_specialize f(...)` | returns a `Type` (experimental) |
| `#code ...`, `#code,typed ...`, `#code,null` | Code literals (13.1) |
| `#insert ...` | in expression position, inserts code (13.2) |
| `#type T`, `#type,distinct T`, `#type,isa T` | type expression |
| `#char "c"` | u8 constant |
| `#string X ... X` | string constant |
| `#asm { }` | assembly block (statement; 15) |
| `#bytes .[...]` | raw machine-code bytes (statement) |
| `#library "x"`, `#system_library "x"`, `#library,system "x"` | library handle constant (14) |
| `#import ...` | module binding (11) |
| `context` | the current context (an lvalue of type `#Context`) |

---

## 6. Statements

Statements appear in imperative scopes. Each simple statement ends with `;`. Block statements end with `}` (an optional `;` after `}` is accepted and is an empty statement).

### 6.1 Expression statements and assignment

- `expr;` — a call, a macro invocation, or any expression (an expression with no effect, such as a bare identifier `result;`, is accepted by the reference compiler; orangejuice accepts it and may warn).
- `lvalue = expr;` and the compound forms `+= -= *= /= %= &= |= ^= <<= >>= <<<= >>>= &&= ||=`. A compound assignment `a op= b` is rewritten as `a = a op b` with the lvalue evaluated once (also through `operator []=`). `&&=`/`||=` short-circuit. Assignment is a statement, not an expression (`a = b = c` is invalid).
- Lvalues: variables, dereferences (`p.*`, `<<p`, `(.*) p`), members, subscripts, `it` in `for *` loops (through the pointer), members of `using`ed values. Not lvalues: parameters of struct/array/string type (7.6), constants, members of constants, `it` in by-value loops, `.count`/`.data` of fixed arrays, results of calls, casts (`cast(*T) p .* = v` is written `(cast(*T) p).* = v;`).
- Assigning to a constant member of a struct (`s.CONST = 1`) is an error; "invalid lvalue in assignment".

### 6.2 Blocks and `defer`

`{ ... }` groups statements in a child scope. `defer stmt;` / `defer { ... }` schedules the statement to run when the *enclosing block* exits by any path (falling off the end, `return`, `break`, `continue`); defers run in reverse order of registration, after the return values have been evaluated. Inside a loop body, a `defer` runs at the end of every iteration. `defer` captures variables, not values (`defer seen.count = old_count;` sees the current `old_count`; `defer { if result deinit(*renderer); }` tests `result` at scope exit). `` `defer `` inside a macro attaches to the caller's block (only allowed in the macro's top-level statement list). `defer` with complex contents (loops, for_expansions) is supported.

### 6.3 `if`

```
if cond { ... }
if cond then stmt;                 // "then" is optional when unambiguous
if cond stmt;
if cond
    stmt;                          // statement may be on the next line
if cond { ... } else { ... }
if cond then a(); else b();        // ";" ends the then-statement; "else" may follow it
if a then if b x(); else y();      // else binds to the nearest if
if cond { } else if cond2 { } else { }
```

The condition is any expression with a truth value (5.9). `then` followed by a block is allowed (`if x then { ... }`). A `;` immediately after the condition (empty then-statement) is an error. `#if`/`#ifx` are the static forms (6.10).

### 6.4 `if ==` switch

```
if value == {
    case 1;  stmt; stmt;           // case value; then statements until the next case
    case 2;  #through;             // falls into the next case
    case 3;
    case;    default_stmts;        // default; must be last; may not be #through
}
if #complete e == { case .A; ... case .B; ... }   // every enum member must be covered (error otherwise)
```

- The subject may be any type with built-in `==` whose values are constant-representable: integers, enums, `bool`, strings, `Type` (`case [10] *(**void) -> ([..] #Context);`), procedure values (`case metric_if_density;`), pointers? (constants only), characters via `#char`. Not structs, arrays, `Any`, floats? (floats are accepted as constants by `==`; the reference compiler permits numeric types). Structs and arrays are errors.
- Each `case` value must be a compile-time constant, unique (duplicate case values are reported), no ranges. Unary-dot enum names are allowed and resolve against the subject type. `case` bodies are their own scopes and may be empty. Constants may be declared inside a case (`MIN_SIZE :: 10; height = ...`).
- No fallthrough by default; `#through;` as the *last* statement of a case continues into the next case. The final case may not be `#through`.
- `if #complete` on an enum requires all members be present (members with duplicate values count once); `#complete` with a `case;` default is allowed. A switch without `#complete` and without a default in a procedure with return values triggers the "Not all control paths return a value" warning if nothing follows.
- `#if x == { case ...; }` is the static form (6.10). Exported as `Code_If { if_flags.IS_SWITCH_STATEMENT, MARKED_AS_COMPLETE }` with `Code_Case { condition; then_block; marked_as_fallthrough }`.

### 6.5 `while`

```
while cond { ... }
while cond stmt;                       // single statement
while 1 { ... }                        // any truth value
while name := expr { ... }             // declares `name` (value of expr, usable in body) AND labels the loop
while s := shorten() { continue s; }   // condition may be non-bool; break/continue by label
```

The condition is re-evaluated each iteration (including the declaration form; `name` is reassigned). `break;` / `continue;` affect the innermost loop; `break label;` / `continue label;` target a labeled `while` (by its condition variable) or a `for` (by its iterator name). Exported as `Code_While { condition; block }`.

### 6.6 `for`

Forms:

```
for 0..7 { }                       // integer range, inclusive, `it` (a range counts with `it` alone: it declares no `it_index`)
for i: 0..n-1 { }                  // named iterator
for #v2 < a..b { }                 // reverse range (see rules below)
for < 0..3 { }                     // reverse (with #v2): 3, 2, 1, 0
for array { }                      // it: element (by value, a constant copy), it_index: s64
for * array { }                    // it: *element (pointer); writes go through
for < array { }                    // reverse
for <* array { } / for < * array { }
for value, index: array { }        // named
for * value, key: table { }        // for_expansion (Hash_Table): value then key
for s { }                          // string: it is u8
for << items { }                   // any expression whose type is iterable (here a dereferenced pointer to array)
for x: expr stmt;                  // single-statement body
for :name value, index: container { }   // named for_expansion
for `it, `it_index: a.data { }     // backticked names inside a for_expansion macro
for *=cast(bool)(flags & .POINTER), <=cast(bool)(flags & .REVERSE) a.data { }  // modifiers with expressions, comma-separated
```

Rules:

- **Ranges** `a..b` iterate integers from `a` to `b` inclusive; empty if `a > b`. The endpoint expressions are evaluated **once** before the loop (0.0.037). The iterator type is the unified type of the endpoints (u32 endpoints give a u32 `it`; a literal and an `s64` give `s64`). Range loops never overflow at the type's extremes. Reverse ranges: `for #v2 < a..b` visits the same numbers as forward, in reverse; `a` must be ≤ `b` (else empty). Without `#v2` the old (buggy) reverse semantics warn; `#v2` is a temporary marker that will become the default. Negative starts are fine (`for -10..10`).
- **Arrays** (`[N] T`, `[] T`, `[..] T`): `it` is a copy of the element (not assignable; since 0.1.069 taking `*it` gives the address of the actual element when possible); `for *` makes `it` a pointer. `it_index` is `s64`. The array expression is evaluated once. Elements of `void` type are allowed.
- **Strings** iterate bytes, forward or reverse.
- Modifiers: `<` reverse, `*` by pointer; both combine (`<*`). Expression forms `<=expr`, `*=expr` need a comma between them.
- `it` and `it_index` are ordinary (implicitly declared) identifiers; nested loops shadow them. A loop over a *range* declares only the iteration value, named or not, so `it_index` inside `for 0..3` is an undeclared identifier (measured against the reference); a loop over a container declares `it_index` even when the value is named — `for x: xs` still has one — and a name written for either slot replaces the implicit one. **`it_index` and `it` may be assigned** to skip elements (`it += 1;`, `it_index += n;`) — this affects the loop's counter in the reference implementation. orangejuice implements: the loop counter *is* the `it` variable for ranges and the `it_index` variable for arrays, so assignments to them affect iteration.
- `break;`, `continue;`, `break name;`, `continue name;` where `name` is the iterator variable of an enclosing `for` (`break tbd;`, `continue condition;`).
- `remove it;` (or `remove;`, or `remove p;` for a named pointer iterator) removes the current element by **unordered removal** (moves the last element into the current slot, decrements `count`) and re-visits the slot; legal on `[..] T` and `[] T` (the view's count is decremented; the backing storage is not freed) but not on fixed arrays or immutable values ("remove on immutable values is an error"). `remove` inside a `for <` downward loop is supported.
- **Custom iteration (`for_expansion`)**: a `for` over a value of struct type `S` (or `*S`) whose scope defines a macro `for_expansion :: (container: *S, body: Code, flags: For_Flags) #expand` expands that macro with the loop body; `for :name x` selects a named expansion macro `name`. Native arrays use built-in iteration unless `for :name` is written. Details in 7.14.
- Exported as `Code_For { iteration_expression; iteration_expression_right; block; ident_it; ident_it_index; ident_decl; index_decl; for_flags: For_Flags (POINTER, REVERSE, TEMPORARY_V2); macro_expansion_procedure_call; want_replacement_for_expansion; want_pointer_expression; want_reverse_expression }`. Loop controls are `Code_Loop_Control { control_type: BREAK | CONTINUE | REMOVE; target_ident }`.

### 6.7 `return`

```
return;                    // no return values, or all defaults
return a, b, c;            // positional
return second = "x", first = "y";   // named (parenthesized names optional in the header)
return .{ ... }, true;     // literals inferred from the return types
return 0, error_out(...);  // a call as one of several values
return target, theta, true;
```

A procedure with return values must return values on every path; the compiler emits the warning "Not all control paths return a value" when it cannot prove it (procedures ending in a `#bytes` `ret` or an `#insert` are handled). Named return values with defaults (`-> app: Xr_App = .{}, success := false`) allow a bare `return;`, which returns the defaults; a partial `return x;` fills the first. Inside a macro, `return` returns from the *macro* (the macro's value); `` `return `` returns from the caller. `return` inside `push_context { }` returns from the procedure (the context is popped). A `return` in a quick lambda is implicit for the expression form. Exported as `Code_Return { arguments_unsorted; arguments_sorted; return_flags: IS_BACKTICKED | AUTO_INSERTED_FOR_QUICK_LAMBDA }`.

### 6.8 `using`

`using` imports the members of a namespace-like value or type into the current scope:

```
using x;                         // statement: members of variable x (struct, or pointer to struct — auto-deref)
using x: T;                      // declaration + using
using x := make_thing();
using bullet.emitter;            // any expression
using Enum_Type;                 // enum members become bare names
using Struct_Type;               // nested constants/types of a struct type (using Summary;)
using Module_Name;               // members of a named import (Basic :: #import "Basic"; using Basic;)
using X :: #import "X";          // import and use in one statement (deduplicated)
using Sound :: #import "Sound_Player";
using Ice_Cream.{...};           // an anonymous read-only literal
using enum u16 { A; B; }         // anonymous enum
using E :: enum { ... }          // at file scope: declare and use
using,except(a, b) x;            // modifiers
using,except .["x","y"] x;  using,except CONST_ARRAY x;  using,except #run f() x;
using,only(w, y) q;
using,map(proc) procs: Procs;    // proc: ([] string) modifies names in place at compile time; "" rejects one
using,no_parameters s: PolyStruct(...);   // don't import the struct parameters into the constants block
#as using base: Base;  /  using #as base: Base;   // struct members: import names AND allow implicit cast (8.4)
(using v: V, using,except(x) q: Q)    // on parameters
using g_params;                  // a global anonymous-struct variable
using c := cast(*T) node;        // with a cast
```

- Name conflicts caused by `using` are errors ("redeclared identifier"); `,except`/`,only`/`,map` resolve them. Names listed in `only`/`except` need not exist.
- A bare `using X :: #import "X";` is deduplicated across a scope; with modifiers it is not (only allowed once per namespace).
- `using` on a constant member of a struct, on a block, on a non-struct value, or where a declaration is required are errors. Modifying a by-value parameter through `using` is an error.
- `using` a struct parameter of a *polymorphic* struct imports its members and its constants block (unless `,no_parameters`); `using` inside a struct is transitive (`i.x` through `using entity` → `using position`).
- `using` respects `#scope_module`: only exported names of a module are imported by `using Module`.
- Exported as `Code_Using { expression; filter_type: NONE | ONLY | EXCEPT | MAP; filter_expression; no_parameters }`; parameter usings are recorded in `Code_Procedure_Header.parameter_usings`.

### 6.9 `push_context`

```
push_context new_context { ... }          // push a #Context value (lvalue or rvalue) for the block
push_context .{allocator = temp} { ... }  // a read-only literal is copied in
push_context <<context_pointer { ... }
push_context { ... }                      // no argument: push a default-initialized #Context (for #c_call procs)
push_context,defer_pop ctx;               // no block: the context stays pushed until the enclosing scope ends
`push_context ...                          // backticked, from a macro: applies to the caller's scope
```

The pushed value is copied; modifications inside the block are visible to callees and remain in the pushed copy (a `push_context` followed by reading `context` after the block sees the *previous* context). `break`/`continue`/`return` inside the block pop the context. Exported as `Code_Push_Context { to_push; block; push_context_flags: IS_BACKTICKED | DEFER_POP }`.

### 6.10 Static `#if` / `#ifx`

```
#if CONST { ... } else #if OTHER { ... } else { ... }
#if OS == .WINDOWS decl_or_stmt;            // single declaration/statement
#if cond then member: T;                    // in struct bodies
#if #run f() > 100 { }
#if x == { case a; ...; case; ... }         // switch form (0.1.084)
#if BITS == 8 #asm { } else #asm { }        // #asm blocks as branches
x := #ifx K > 2 then "Hello" else 42.0;     // both branches required; types may differ
```

- The condition must be a constant expression (a `:=` variable is an error). `#if` may appear in file scope, struct bodies (conditional members), enum bodies (conditional members), procedure bodies, `#run` blocks and inside `#if` branches. The rejected branch must still **parse** but is not typechecked (identifiers need not exist). Neither branch creates a scope. Missing return values in the else branch of a `#if` inside a procedure are warned about.
- `#ifx` requires both branches and no `;` between them.
- Exported as `Code_If { if_flags.IS_STATIC (and IS_IFX / IS_SWITCH_STATEMENT); static_if_flags.EVALUATED_AS_TRUE; static_if_accepted_case }`; nodes in the untaken branch are not typechecked.
- `if #compile_time { }` is a *runtime* `if` on a value that is constant-folded per backend (bytecode: true; machine code: false).

### 6.11 `#run` and `#assert` statements

`#run stmt_or_block;` inside a procedure body executes at compile time when the body is typechecked (once per polymorph instantiation). In statement position the run takes the whole *statement*, so the assignment in `#run counter += 1;` is part of what runs rather than something done to what it produced; as an operand — `x := 1 - #run f();` — it takes one expression. `#assert cond;`, `#assert cond "message";`, `#assert(cond)`, `#assert,stallable cond;` check a constant condition at compile time (only when the enclosing body is live) and cannot depend on non-constants (a `#assert` on a non-constant fails even in a dead runtime branch; guard with `#if is_constant(x)`). See section 12.

### 6.12 `#insert`, `#asm`, `#bytes`

Statement-level `#insert` (13.2), `#asm { }` blocks (15) and `#bytes .[...];` (raw bytes, must be a constant `[] u8`/array literal) are statements. `#no_abc { ... }` and `#no_aoc { ... }` prefix a block to disable array-bounds / arithmetic-overflow checks in it (also usable on procedure bodies and macro bodies; `Code_Block.block_flags`).

### 6.13 Declarations inside imperative scopes

Anything declarable in a data scope may be declared inside a procedure body: variables, constants, nested procedures (7.11), structs, enums, `#import`s (`Basic :: #import "Basic";` or bare `#import "String";`, `#system_library`/`#library` handles, `#foreign` procedures), macros, operator overloads. Constants must precede use in the same imperative scope. Nested procedure declarations may appear anywhere, including inside loop bodies and `case` blocks.

---

## 7. Procedures

### 7.1 Declaration syntax

```
name :: (params) -> returns directives { body }
name :: (params) { body }                      // no returns
name :: () -> () { }                           // explicitly no returns
name :: (a: int, b := 2, $T: Type, c: T = .{}) -> (x: int, y: string = "s") #must { }
name :: inline (params) { }                    // inline procedure (also `no_inline`)
name :: (x, y) => x + y;                       // quick lambda (7.9)
name :: (params) #expand { }                   // macro
name :: (params) #foreign lib "cname";          // foreign
name :: (params) -> T #modify { ... } { body } // #modify block between returns and body
reinit_fonts :: init_fonts;                    // alias (constant binding); may precede the target
```

A procedure declaration is a constant declaration whose value is a procedure literal; procedure literals may also appear inline as values (`quick_sort(arr, (x: T) -> float { ... })`, `poll = (u: *void, b: bool) -> bool { ... }` in a struct literal, `will_print_bindings = () { ... }`). Headers may span lines; a newline is allowed before `{`. The body may be preceded by directives in any order (0.1.039). A header without a body (`-> ()` followed by `;`) is a type-only declaration when used with `#foreign`/`#elsewhere`/`#compiler`/`#intrinsic`/`#entry_point`, and a parse error otherwise ("procedure with no return declarations after `->`" is an error: `->` must be followed by at least one return type).

### 7.2 Return values

- `-> T`, `-> T1, T2`, `-> (T1, T2)`, `-> name: T`, `-> (a: int, b: string)`, `-> a: int, b: string` (parentheses optional), `-> success: bool, Coff_Relocation` (mixed), `-> [] T #must, bool` (`#must` per value), `-> result: [] u8` (single named), `-> app: Xr_App = .{}, success := false` (named with defaults, enabling bare `return;`), `-> first: string = "Hello"`.
- `-> void` returns **one** value of type void (relevant for polymorphism); for zero returns, omit `->` or write `-> ()`. For `#c_call`/`#foreign` procedures `-> void` means no return (C compatibility; a non-`#c_call` `-> void` type gets a transitional warning).
- `#must` on a return value makes ignoring it an error (calling as a statement, or passing the call as a single argument where the extra values are dropped). Checked at code generation time; violations that disappear through inlining/macros are fine.
- Return-type polymorph variables (`-> $R`) must be solvable from arguments or set by `#modify`; otherwise "Compile-time variable 'R' is needed but was not specified."
- Return values are passed as hidden out-pointers allocated by the caller (7.6). Returning fixed arrays by value is allowed (not from `#c_call`).

### 7.3 Parameters

```
(a: int, b: float = 1.5, c := "s", d: Vector3 = .{}, e: Ice_Cream = .{cone_style="cake"}, f: (x: int) -> int = null,
 using v: V, $T: Type, $n: int, $$m: int, x: $U, y: *[..] *$W/Base, z: [$N] float, args: ..string, any: ..Any,
 #discard dbg: bool, loc := #caller_location, $call := #caller_code, r: __reg, code: Code, t := void)
```

- A parameter is `name: Type`, `name: Type = default`, or `name := default` (type inferred from the default, fixed). Names may be omitted in procedure *types* (`(T) -> R`) and in `#foreign` declarations written as types (`(sigval_t) #c_call`).
- Defaults: any expression; `context.x` is allowed; defaults are applied at the call site (one entry point), so they do not affect the procedure type. Non-default parameters may follow default ones (callers use names).
- `using` on a parameter imports its members into the body (`using v: V`, `using renderer: *Renderer` — pointer parameters auto-deref; the struct's nested types and constants also become visible, so a nested `Model` shadows a global `Model` inside the body).
- `#discard p` marks a parameter that may not be referenced in the body; arguments for it are typechecked but **not evaluated** (no code generated) — used by disabled `assert`.
- `$T: Type` bakes a type; `$x: int` bakes a value (must be constant at the call); `$$x` optionally bakes (constant if the argument is; `#if is_constant(x)`); `x: $T` introduces a type variable (7.8).
- `loc := #caller_location`, `$call := #caller_code` (macros), `code: Code` (implicit `#code` wrapping of arguments in macros).
- Varargs: `args: ..T` (spaces allowed: `.. T`); inside the body `args` is `[] T`. Only one varargs parameter, and for `#c_call` it must be last (non-`#c_call` procedures may have parameters after varargs, filled by name). `..Any` collects mixed types; Type values may be passed as `Any`. For `#c_call` foreign procedures `..Any` means C varargs (integers narrower than 32 bits are promoted; floats/float64 passed per the C ABI; fixed arrays and spread are disallowed). Varargs passed to a `$T`-typed varargs with all-constant arguments can be baked to a `[N] T`. A single varargs argument is passed by pointer to the lvalue when possible (no copy).
- `t := void` — a parameter of type `Type` with default `void`.
- Parameters of struct/array/string type are immutable in the body (7.6); scalar parameters are mutable local copies (`t = clamp(t, 0, 1);`, `theta += 360;`, `Clamp(*index, 0, n);` are fine).
- A parameter may be named `default`, `type`, `extern`, `context`? — `context` is a keyword and is renamed by binding generators (`_context`).

### 7.4 Calling conventions and header directives

| Directive | Effect |
|---|---|
| `#c_call` | C ABI (System V AMD64 on x64, AAPCS64 on arm64 — a homogeneous float aggregate of at most four elements travels in that many vector registers there, and anything over sixteen bytes indirectly; Darwin additionally makes widening a narrow integer argument the caller's job); no implicit context; `context` may not be used inside unless a `push_context` block is entered; cannot call non-`#c_call` Jai procedures without pushing a context (error); varargs must be last; may not return multiple values, fixed arrays, or take fixed arrays as varargs; empty structs, structs by value, strings, views and dynamic arrays are passed by the C rules for a struct of that layout; `#type,isa` integer variants pass as their base |
| `#no_context` | Jai ABI but no context parameter; callable from `#c_call` or native code; `context` is an error inside; Runtime_Support's low-level printers are `#no_context`; struct initializers are implicitly `#no_context` |
| `#foreign lib` / `#foreign lib "symbol"` / `#foreign "symbol"` / `#foreign` | externally defined C-ABI procedure (implies `#c_call`); with a symbol string the Jai name may differ; two Jai declarations may bind the same C symbol; without a library (0.1.077) the symbol is resolved at link time and not callable at compile time |
| `#elsewhere lib` / `#elsewhere lib "symbol"` / `#elsewhere` | externally defined procedure or global with the **Jai** calling convention (or `#c_call` if also given); used for symbols exported from Jai DLLs and for `__runtime_info`; multiple returns allowed |
| `#intrinsic` / `#intrinsic "llvm.name"` | compiler intrinsic (`memcpy`, `memcmp`, `memset`, `compare_and_swap`) / direct LLVM intrinsic (LLVM backend only, e.g. `"llvm.bitreverse.i64"`, `"llvm.debugtrap"`) |
| `#compiler` / `#compiler "internal_name"` | implemented by the compiler when executed at compile time; the Jai body (if any) is the runtime fallback (`get_current_workspace`, `write_string`, `compiler_wait_for_message`, `add_build_string` with the internal name `"add_build_string_scoped_by_message"`) |
| `#runtime_support` | marks a procedure provided by Runtime_Support (flag exported as `RUNTIME_SUPPORT`) |
| `#expand` | macro (7.13) |
| `#no_call` | naked: no prologue/epilogue/return (body is inline assembly) |
| `#entry_point` | declares the user entry procedure symbol (`__program_main :: () #entry_point;` in Runtime_Support); `Build_Options.entry_point_name` selects the user's main |
| `#symmetric` | for two-argument operator overloads: arguments may be given in either order (stripped when arguments are baked) |
| `#cpp_method` | C++ member-function convention: first argument must be the `this` pointer |
| `#cpp_return_type_is_non_pod` | C++ non-POD struct return convention (hidden sret) |
| `#deprecated` / `#deprecated "message"` | calls warn (not inside other deprecated procedures) |
| `#no_debug` | no line info; the debugger steps over the macro/procedure |
| `#compile_time` | callable only at compile time; no machine code emitted; may cast non-constant `Type` to `*Type_Info`; calling it at runtime is a link/typecheck error when `Build_Options.prevent_compile_time_calls_from_runtime` |
| `#program_export` / `#program_export "name"` | export the symbol from the output binary (place the directive before the declaration or in the header) |
| `#must` | on return values |
| `#modify { ... }` | constrain/rewrite polymorph variables (7.8) |
| `#dump` | print bytecode at compile time (also on quick lambdas: `f :: x => x.name #dump;`) |
| `#no_abc`, `#no_aoc` | disable checks in the body |
| `inline` / `no_inline` (before the parameter list) | inlining preference; `inline` is guaranteed by the bytecode inliner (except procedures containing `#asm`) |
| `#type` | in *type* position: `#type (params) -> R #c_call` |

Notes attach after the body: `} @PrintLike`, `} @NoProfile`, `} @test`.

A Jai procedure's ABI (non-`#c_call`) is implementation-defined by the compiler and consistent across a build: the context pointer is passed as a hidden parameter; return values are written through hidden pointers; "big" arguments (any struct or array, or any value larger than 8 bytes: strings, `Any`, `[] T`) are passed by pointer to a caller-owned copy ("maybe by reference"); small values in registers. Callers of `#program_export` Jai procedures from other Jai binaries use `#elsewhere`.

### 7.5 Overloading and resolution

Multiple procedures with the same name in the same scope (or across `using`/`#import`ed scopes, or in an enclosing scope) form an *overload set*: `Basic`'s `to_string(*u8, s64)` and the `to_string(*u8)` Runtime_Support makes visible everywhere are one set, and a call with one argument picks the second even inside `Basic`. A declaration that is not a procedure shadows instead, and when candidates tie on score the one from the nearest scope wins — a program may declare `compare_and_swap` next to its call although Preload declares the same signature (both measured against the reference compiler). Overloads may differ in parameter count, types, names, calling convention, or presence of `#modify`; two overloads with identical signatures are an error unless both have `#modify`. Overload sets are open: overloads may be added any time (even after uses); the compiler errors only if a late overload would *change* the result of an already-resolved call. Named imports (`Debug :: #import "Debug";`) do not contribute to global overload sets.

Resolution scores each candidate by the implicit conversions needed for each argument, choosing the unique best candidate or erroring with "matches multiple possible overloads" / "did not match any" (both print the argument types). Ranking rules (least to most costly): exact match; `using`/`#as`-based casts (each `#as` step and each `#type,isa` step counts as one, smaller than other implicit casts); numeric literal conversions (a constant literal's cast distance depends on its explicit type bound if any); implicit numeric widening; polymorphic instantiation (each polymorphic argument in the declaration counts as one point, so less-polymorphic candidates win; an implicit numeric conversion beats polymorphing — `get_hash(3)` picks the `float` overload over a `$T` one); varargs (considers the casting distance of the first varargs member; a non-varargs candidate wins over a varargs one when both match). Candidates whose `#modify` rejects, whose type restriction `$T/R` fails, or whose polymorph solve fails are excluded. Anonymous struct literals and unary-dot enum arguments match by type inference against each candidate. Auto-dereference (`*S` → `S`) and `#as` are considered. Named arguments participate normally.

An overload set cannot be stored in a variable directly (a variable of type "overload set" is illegal); passing an overloaded name where a procedure value is required resolves it by the expected type and records a `Code_Resolved_Overload`. `#bake_*` on an overload set is an error (pick one).

### 7.6 Parameter passing semantics

- Values ≤ 8 bytes that are not structs (integers, floats, bools, enums, pointers, procedure values, `Type`) are passed by value and are mutable locals in the callee.
- Everything else (any struct regardless of size, fixed arrays, `string`, `Any`, `[] T`, `[..] T`) is passed "maybe by reference": semantically a **const copy**. Assigning to it or its members (`a.count = 5`, `v.x = 1`, `arr[0] = 1`), `remove` on it, or modifying it through `using` is an error ("immutable"). Taking its address (`*a`) is allowed and may yield the caller's storage; a callee that takes the address does not get a temporary copy. C callees (`#c_call`) receive a copy per the C ABI. Since 0.0.038 C procedures cannot modify Jai struct arguments in place.
- Return values: the caller allocates the result slots and passes hidden pointers. Structs with non-power-of-two sizes are handled.
- Default arguments are evaluated at the call site. A variable of procedure type declared as `f: type_of(proc_with_defaults)` inherits the defaults for calls through it (a quirk: defaults live on the caller's annotation of the type).
- Auto-dereference: a `*S` argument for an `S` parameter is dereferenced with a null check.
- Varargs create a `[] T` on the stack (or reuse a single lvalue's address).

### 7.7 Operator overloading

```
operator + :: (a: Vector3, b: Vector3) -> Vector3 { ... }
operator + :: (a: Vector3, s: float) -> Vector3 #symmetric { ... }
operator - :: (a: Quaternion) -> Quaternion { ... }         // unary (one parameter)
operator ! :: (a: S128) -> bool
operator == :: (a: T, b: T) -> bool                          // enables != as !(a == b) unless != is defined
operator += :: (a: *Complex, b: Complex, loc := #caller_location)  // explicit compound op; first argument is a pointer
operator [] :: (w: Wrapping, i: int) -> int                  // read subscript
operator []= :: (w: *Wrapping, i: int, v: int)               // write subscript; enables compound assignment (index evaluated once)
operator *[] :: (b: *Bucket, i: int) -> *int                 // address subscript: enables read, write, compound, and *b[i]
operator << / >> / <<< / >>> / & / | / ^ / ~ / * / / / % / < / <= / > / >=   // any binary/unary operator except = and .
```

- At least one parameter must be a struct (or a variant/pointer thereof); operators on built-in types cannot be overloaded. Operator declarations are constants named by the operator text (`"+"`, `"[]"`, `"[]="`, `"*[]"`) and follow lexical scoping like procedures (may be declared in nested scopes, in modules and imported). Overload resolution applies to operators.
- `a op= b` uses `operator op=` if declared, else `a = a op b`. `!=` falls back to `!(a == b)`. `a[x] = v` uses `[]=` then `*[]`; `a[x]` (read) uses `[]` then `*[]`. Subscript index must be an integer type.
- `#symmetric` permits reversed argument order for two-parameter operators (checked in the typechecker; the flag is dropped when arguments are baked).
- Unary `-` on a struct literal: `-Thing.{5}`. Struct literals in operator arguments are matched by inference.
- Isa variants: see the upcast-back rule (3.11). `#type,distinct` structs do not inherit operators.
- Exported: binary/unary operator nodes whose operand is a struct are desugared into `Code_Procedure_Call`s with `Code_Ident.flags.GENERATED_BY_OPERATOR_OVERLOAD`.

### 7.8 Polymorphism

A procedure (or struct) is *polymorphic* if its header contains `$` markers or bare polymorphic struct names. It is instantiated ("baked") per unique set of compile-time constants, cached program-wide.

Type variables:

```
square :: (x: $T) -> T { return x * x; }             // $T defines T at its first (authoritative) occurrence; T reused elsewhere
f :: (a: *$T, b: T)                                  // pointer level must match: passing a string to *$T is "Mismatching levels of indirection"
g :: (x: [] $T) / (x: [..] $T) / (x: [$N] float)      // [$N] matches only fixed arrays; N.count is constant
h :: (x: $T/Entity)                                  // restriction: T must be Entity or #as-derived from it (via using/#as chain)
h :: (x: $T/Blentity)                                // any instantiation of polymorphic struct Blentity
h :: (x: $T/interface Matchable)                      // duck typing: T has members with the same names and types as Matchable
h :: (x: $T/ALLOWED_TYPES)                            // a constant array literal of Types, or any constant expression
h :: (holder: Holder($T, $N)) / (holder: Holder)      // bare polymorphic struct = any instantiation; parameters via holder.T
map :: (array: [] $T, f: (T) -> $S) -> [..] S         // S solved from the return type of the passed procedure
stomp :: (p: *[] $T, other: type_of(<<p))             // type_of in a type slot; solving is iterative
reduce :: (op: (x: T, total: T) -> T, values: .. $T) -> T   // $T may be defined later in the list than its first use
test :: ($hash: (key: $Key) -> u32, gen: (i: int) -> Key)   // $Key introduced inside a procedure-type parameter
```

Value bakes:

```
memcpy_v :: (dst: *void, src: *void, $bytes: s64)     // callers must pass a constant; `bytes` is a constant in the body (#if bytes & 7, N :: bytes / 8)
opt :: ($$x: s64)                                     // baked if the argument is constant, else runtime; one polymorph per constant value + one for runtime
combo :: ($s: string, x: $T, $a: [$N] float)
make_action :: ($name: string, $localized: string, ...)
load_xr :: (instance, $proc_name: string) #expand     // baked strings usable in #insert -> string
string_to_int :: (t: string, base := 10, $T := int)   // a baked parameter with a default: a call that says nothing bakes `int`
```

Rules:

- Each type variable is defined exactly once (`$`); redefinition is an error. `$T` in a return type only cannot be solved (error) unless `#modify` sets it or `#bake_constants` supplies it. The placement of `$` decides which argument is authoritative for error messages.
- Solving is iterative over the call site and header: `$T` may be bound from a later parameter, from a nested type (`*[..] *$T`), from a passed procedure's return type, from `type_of` of another parameter, or from a struct argument (`Holder($T, $N)`; `S($A, $B)` also matches through a pointer to the struct). `xx` on an argument is stripped when matching against `$T`. `null` does not bind a `$T`. Fixed-size array dimensions (`[$N]`) bind integers. Untyped struct literals can match `$T` only via `#modify`. Type restrictions match `float` with `float32` and `int` with `s64`.
- Passing a polymorphic procedure or quick lambda as an argument for a concrete procedure-typed parameter instantiates it to that type (only at argument positions). `apply1 :: (f: (x: $X) -> X, x: X)` + a polymorphic `square` is an error (nothing determines X); `apply2 :: (f: (x: X) -> X, x: $X)` works.
- Polymorphic procedures and macros have **no runtime value**: `h := poly;` and `type_of(poly)` at runtime are errors; `g :: poly;` (constant binding) is fine; `type_of(poly)`/`type_info(type_of(poly))` are allowed at compile time (`#run`, `#modify`), and `type_info()` of a polymorphic procedure type infers `*Type_Info_Procedure`. `#procedure_of_call` and `#bake_constants` produce concrete procedures.
- The procedure's scope chain is: constants block (holding `T :: ...`, baked values, and the specialization's constants) → arguments & returns → body. The constants block is visible from nested procedures and `#run`s inside the body; `#if T == string { ... }` and `#if is_constant(x)` are used inside bodies. `#run`s and `#import`s inside a polymorphic body execute once per instantiation (order nondeterministic).
- Deduplication: identical constant sets → one instantiation (before code generation); additionally, instantiations whose bytecode is identical are merged after bytecode generation (`Build_Options.enable_bytecode_deduplication`). `#procedure_of_call` on equal constants gives equal procedures.
- Recursion depth is bounded by `Build_Options.maximum_polymorph_depth` (100).
- Errors mention "Info: While generating a polymorph of this procedure." and "The polymorph was triggered here:"; `Build_Options.info_flags.POLYMORPH_MATCH/POLYMORPH_DEDUPLICATE` add detail.
- Polymorphic varargs `values: ..$T` infer `T` from the first argument; other arguments must convert.
- `#modify { ... }` after the header runs at compile time after solving, with the polymorph variables as mutable values (a `Type` is a `Type`; `cast(*Type_Info) T` allowed; `size_of(T)`, `y: T;` and `type_info(T)` are *not* allowed inside since `T` is not constant there): its implicit signature is `(vars...) -> (accept: bool, fail_reason := "")`; `return false, "why"` rejects the candidate (excluded from overloading; the reason is printed if nothing matches); it may reassign variables (`if N < 8 N = 8;`, `T = s64;`, set return-slot variables) and may contain `#import`. Dedup happens after `#modify`. `#dump` may follow `#modify`. `#modify` may not be used with a polymorphic-typed argument that has its address taken.

### 7.9 Quick lambdas

```
square :: x => x * x;                       // ≡ (x: $T) -> $S { return x * x; } — return type inferred from the body
sum :: (x, total) => x + total;             // untyped multi-parameter
quick_sort(a, x => -x.key); quick_sort(a, (a, b) => compare(a.name, b.name));
f :: x => { print("%", x); return x; }      // block form
get_name :: x => x.name #dump;
p => { free(p); }                           // as a struct-literal member value
```

Quick lambdas are polymorphic procedures whose parameter types are all `$`-inferred and whose return type is inferred (the only case of return-type inference). The expression form auto-inserts a `return` (`Code_Return.return_flags.AUTO_INSERTED_FOR_QUICK_LAMBDA`; `Code_Procedure_Header.procedure_flags.QUICK`, `QUICK_IN_BLOCK_FORM`). `xx` is not allowed on the expression. Nested quick lambdas are allowed. An "argument list" that contains non-identifiers is an error. A header with untyped parameters followed directly by a body (`swap :: (a, b) -> { return b, a; }`) is a lambda with implicit return types.

### 7.10 Bakes

- `#bake_arguments f(y = 42)` → a new constant procedure with `y` fixed (arguments must be constants; works on structs too: `#bake_arguments New(initialized = false)`; chains: `formatAddress :: #bake_arguments formatHex(minimum_digits = 8);`). Partial bakes of polymorphic structs/procedures allowed. Consistency of parameter-bake types is checked in a post-pass.
- `#bake_constants f(T = int)` → binds polymorph variables. `#bake_constants printer(T = #this)`.
- Auto-bakes via `$x`/`$$x` parameters (7.8). A procedure with only optional (`$$`) polymorphism still goes through polymorph instantiation when called.
- `#dynamic_specialize f(...)` is experimental and returns a `Type`.
- Exported as `Code_Directive_Bake { procedure_call; bake_type: CONSTANTS_BAKE | PARAMETER_VALUE_BAKE | DYNAMIC_SPECIALIZE }`.

### 7.11 Nested procedures and closures

Procedures may be declared inside procedure bodies (and inside struct bodies, `#run` blocks, loop bodies). A nested procedure **cannot capture** local variables or parameters of the enclosing procedure ("closures are not supported"; "Attempt to use a variable from an outer stack frame"). It *can* use the enclosing procedure's constants (including baked `$` parameters and polymorph variables), globals, and other nested constants declared earlier in the same imperative scope. Struct bodies may contain procedure declarations that see the members only in non-runtime ways (`type_of(x)`) and are called as `Thing.proc()`. Nested procedures with value bakes are allowed.

### 7.12 `inline`

`f :: inline (...)` marks a procedure for guaranteed inlining by the bytecode inliner; `inline f(x)` at a call site forces it; `no_inline f(x)` prevents it. Procedures containing `#asm` cannot currently be inlined (`/* inline */ random_get()`). Inlined procedures keep debug info. The middle-end inlines only things marked `inline` (`Build_Options.enable_bytecode_inliner`, default true); LLVM performs its own inlining in optimized builds. A call marked `inline` on a non-constant procedure value is an error.

### 7.13 Macros (`#expand`)

```
Sum :: (metric: float, factor: float) #expand { if metric >= 0 { num_valid += 1; sum += metric * factor; } }   // sees caller locals (num_valid, sum) directly
loop :: (n: int, code: Code) #expand { for 1..n { #insert code; } }   // Code parameters; non-Code arguments are implicitly wrapped in #code
Profile :: () #expand { note_entry(#this, #location(#this)); `defer note_exit(#this); }   // #this = the procedure the macro expands into; `defer attaches to the caller
check :: () #expand #no_debug { `return false, .{}; }   // backticked return returns from the CALLER
load_xr :: (instance, $proc_name: string) -> XrResult #must #expand { #insert -> string { ... "`% := proc;" ... } }  // `name declares into the caller's scope
acquire :: () #expand { `result = check_vk(...); }     // assign to the caller's variable
call_enumerate :: ($is_count_call: bool) #expand { ... }
A :: (ident: Code) #expand { if #insert ident inline f(#insert ident); }   // Code inserted as an expression
add_regs :: (c: __reg, d: __reg) #expand { #asm { add c, d; } }             // register parameters
one_time_init :: (synch_value: *s32, to_insert: Code) #expand { ... }
```

Semantics:

- A macro is expanded at each call site into the caller's block (`Code_Procedure_Call.macro_expansion_block`). Its parameters bind like procedure parameters. Its body is *hygienic by default*: identifiers in the macro body resolve in the macro's own scope (declarations inside the macro are private) except that **the macro body can see the caller's local variables by name** (the "Sum" pattern above: unqualified `num_valid` resolves in the caller's scope when the macro scope has no such name). Backticked names (`` `x ``) explicitly refer to the caller's scope and are required to *declare* into the caller's scope, to `` `return `` from the caller, to attach `` `defer ``/`` `push_context `` to the caller, and to export names such as `` `it `` from a `for_expansion`.
- `return` inside a macro returns *from the macro* (macros may return values usable in expression context; the return type is declared with `->`). A macro used as a runtime value is an error. `#must` applies to macro return values.
- `Code` parameters: arguments that are not already `Code` are implicitly wrapped as `#code expr` with the *caller's* scope attached (`Code_Directive_Code.code_flags.IMPLICITLY_GENERATED`). A constant `Code` parameter (`$c: Code` or any Code param of a macro) exposes `c.type` (the root expression's type). `#insert code;` (13.2) splices the code; `#insert,scope() code` makes the inserted code see the macro's scope; loop controls inside inserted code are remapped with `#insert(break=break y, continue=..., remove=...) body;` or apply to the loop containing the insertion point.
- `$call := #caller_code` receives the entire call expression as `Code`, so the macro (or a `#run` inside it) can inspect it with `compiler_get_nodes` (`check_api_result` prints the name of the API call whose result it checks).
- `#import` inside a macro body, `#run` inside a macro (operating on Code parameters: `modified :: #run f(c);`), nested procedure declarations, `#asm` blocks with pinned registers, `defer` blocks, `#no_debug` on the macro or on a call `f() #no_debug`, `#no_abc` on the body, and struct/enum declarations (exported with backticks) are all allowed. Macros may be declared in struct bodies and used as members.
- `#this` and `#procedure_name()` inside a macro denote the *enclosing procedure of the expansion site*. `#caller_location` in a macro gives the invocation site.
- Backticked `defer` is only allowed in the macro's top-level statement list. Backticked argument names in calls (`f(`x = 3)`) are errors.
- Macros cannot be invoked as statements in data scopes; a value-returning macro may be used inside a `#run` to produce a constant.

### 7.14 `for_expansion`

```
for_expansion :: (table: *Table, body: Code, flags: For_Flags) #expand {
    #assert(!(flags & .REVERSE)) "This container does not support reverse iteration.";
    for *=cast(bool)(flags & .POINTER) entry, i: table.entries {
        `it := entry.value;   // or *entry.value when flags & .POINTER
        `it_index := i;
        `key := entry.key;    // extra exported names become the user's extra loop variables (for v, k: table)
        #insert(break=break entry, continue=continue entry, remove=remove entry) body;
    }
}
```

- The compiler looks up `for_expansion` in the scope of the container's *type* (the type's declaring scope), taking a pointer to the container automatically (the parameter may be `*T` or `T`; a `for` over a pointer to the struct also works). `for :name container` selects the macro named `name` instead (for native arrays too). `For_Flags` (Preload: `POINTER 1`, `REVERSE 2`, `TEMPORARY_V2 4`) tells the macro which modifiers the user wrote; unsupported ones should `#assert`. The reverse flag is passed even if the macro ignores it.
- The macro must export both `` `it `` and `` `it_index `` (error otherwise; export dummies). User-written names (`for v, n: holder`) are remapped onto the exported names; `for value, key: table` yields (value, key) in that order for Hash_Table.
- `Build_Options.debug_for_expansions` (default false) controls whether the debugger steps into expansion macros. The general iteration expression may be any expression (0.1.016).

---

## 8. Structs and unions

### 8.1 Declaration

```
Name :: struct {
    a: int;                  // member; memory layout = declaration order, never reordered
    b, c: float;             // compound declaration
    d := 3;                  // member with default (type inferred)
    e: Vector3 = .{1, 2, 3};
    f: [4] u8;
    g := ---;                // error: needs a type; write g: T = ---; (uninitialized member)
    using pos: Vector2;      // import members
    #as using base: Base;    // import + implicit cast to Base
    #as handle: u32;         // implicit cast to u32 (non-struct member)
    p: *Name;                // self-referential pointers are fine
    children: [] Name;       // views/resizable arrays of the struct itself are fine
    union { x: float; i: s32; }    // anonymous union member (with optional trailing ;)
    struct { m, n: s16; }          // anonymous struct member
    arr: [1] struct { k: int; };   // anonymous struct as element type
    flags: enum_flags u16 { A :: 1; B :: 2; }   // anonymous enum as member type
    Nested :: struct { ... }       // nested type declaration (referenced as Name.Nested)
    K :: 16;                       // constant member (no storage; accessed as Name.K or value.K)
    proc :: (x: int) -> int { ... } // procedure member (Name.proc); sees members only non-runtime-ly
    ID :: #type,distinct u64;
    #if OS == .WINDOWS { win: HWND; }   // conditional members; `#if cond then member: T;` also allowed
    #place a;                       // subsequent members overlay at a's offset (8.6)
    #insert s;                      // generated members (13.2)
    b = 2.5;                        // assignment statement: sets a default for a member declared above
    e.z = 9;  base.type = X;        // nested default assignment
} @Note
Vec :: struct { x, y: float; } ;    // trailing ; allowed
U :: union { a: u32; b: float; }    // union: all members at offset 0; size = largest; default = first member's? (zero-initialized unless a default given)
Empty :: struct {}                  // zero-sized; used for opaque handle types (H_T :: struct {} H :: *H_T;)
```

- Member types may be any type, including `void` (`OptionalHeader: void;`, zero size), `[0] T`, fixed arrays with constant-expression dimensions (`[6*16] u8`, `[Thing.numbers.count] int`, `[max_frames_in_flight] Frame_Sync` using a struct constant, `[E.NUMBER] u32` using an enum value), procedure types (`cb: (s: s32) #c_call;`, `poll: (u: *void, b: bool) -> bool;`), anonymous struct/union/enum types, and arrays of those.
- Member directives: `#align N` (`x: u8 #align 64;`), `#as`, `using`, `using,except(...)`, notes (`entries: [] E; @---`).
- A struct's *initializer* is the aggregate of member defaults; it is representable as static data (memcpy-able) and, when non-trivial, as a compiler-generated `#no_context` procedure `(memory: *void)` obtainable through `initializer_of(T)` (null when all zero). Holes/padding are zeroed. Members declared `= ---` are skipped by the initializer. Unions are zero-initialized unless a default is given (`Quaternion.{}` yields `{0,0,0,1}` because `w := 1`). Initializers with more than `Build_Options.max_bytecode_instructions_for_inlined_initializer` (8) instructions are emitted as procedures.
- Assignment statements inside the body set defaults and must target members (declared earlier or later); assigning to a global from a struct body is an error; non-constant expressions in defaults are errors (defaults must be constant; a struct-typed default may be a constant literal).
- Struct copy is a memcpy. Struct comparison requires an `operator ==`. Structs have no truth value.
- Local struct declarations inside procedures are allowed (`Pair :: struct { first, second: u64; }`), as are structs inside structs, and struct declarations that shadow file-scope names.
- Struct notes are visible on `Type_Info_Struct.notes` and copied to polymorph instances.
- A struct declaration is exported as `Code_Struct { block; arguments_block; constants_block; modify_directives; notes; textual_flags; alignment; defined_type: *Type_Info_Struct }`.

### 8.2 Member defaults and literals

Defaults participate in `.{}` literals (unmentioned members take their default), in `New(T)`, in declarations without initializers, and in `#run` results. Types for `.{}` come from context. See 5.7.

A struct body may also set a default *through a member*, with an assignment statement written after that member's declaration: `default_format_absolute_pointer.base = 16;` in `Basic`'s `Print_Style` gives the member's own `FormatInt` a base of 16. The left side is a path rooted at a member of this struct: a dotted chain of member names, and `[i]` to reach one element of a fixed array, whose index has to be a constant — `elements[0] = 1;` is how a generated identity matrix writes its diagonal. The assignment is applied after the member has taken the defaults of its own type, so a path default overrides them.

### 8.3 Nested declarations and constants

A struct body is a data scope: it may hold constants, nested struct/enum/variant types, procedures and macros. They are accessed with `Type.Name` or `value.Name` (`Renderer.Swapchain`, `renderer.max_frames_in_flight`, `ts.Overflow_Page`, `it.kind.DECLARATION`, `table.LOAD_FACTOR_PERCENT`, `String_Builder.Buffer`). Constants and nested types do not occupy storage; `Type_Info_Struct.members` lists constants with `flags.CONSTANT` and `offset_into_constant_storage`. Nested declarations may reference each other regardless of order (`Descriptor_Pool.ID` used before `Descriptor_Pool` is declared). `using Summary;` at file scope imports a struct's nested names. Size expressions may reference the struct's own nested types (`STRING_BUILDER_BUFFER_SIZE :: 4096 - size_of(String_Builder.Buffer);`).

### 8.4 `using` and `#as` members

- `using m: T;` imports `T`'s members (and nested constants/types) into the struct's namespace, transitively (`using entity: Entity` where `Entity` has `using position: Vector2` makes `i.x` valid). Conflicts are errors; use `using,except(...)`/`using,only(...)`. `using` on a pointer member auto-dereferences. `using,no_parameters` keeps a polymorphic member's parameters out of the constants block.
- `#as m: T;` makes the struct implicitly convertible to `T` (by value, and `*S` → `*T` when `m` is the first member or the layout permits; the compiler takes the address of the member), also for non-struct members (`#as handle: u32;` lets the struct pass where a `u32` is expected; `#as` with a pointer member converts to that pointer). `#as` on constant declarations is allowed (0.2.009). `#as` steps count in overload resolution and in `$T/Base` restrictions (`is_subclass_of` follows `.AS` members). `#as` and `using` are orthogonal (since 0.1.033): `#as using base: Base;` or `using #as base: Base;` for both.
- `Type_Info_Struct_Member.flags`: `CONSTANT`, `IMPORTED` (member reached via `using`), `USING`, `AS`, `PROCEDURE_WITH_VOID_POINTER_TYPE_INFO`.

### 8.5 Polymorphic structs

```
Holder :: struct (T: Type, N: s64) { array: [N] T; }        // parameters are implicitly $ (constants)
Holder :: struct ($T: Type = string, $N: s64 = 3) { ... }   // explicit $ allowed; defaults; `Holder` alone uses defaults
Ticket :: struct (_order: Stuff = .FIRST) { order := _order; }   // value parameter; Ticket(.SECOND)
Thing :: struct (x: $T) { y := x; }                         // type variable in the parameter's type slot; Thing("Hello"), Thing(main)
Bling :: struct (x: [$N] $T)
Table :: struct (Key_Type: Type, Value_Type: Type, given_hash_function: (Key_Type) -> u32 = null, given_compare_function: (Key_Type, Key_Type) -> bool = null, LOAD_FACTOR_PERCENT := 70, REFILL_REMOVED := true)
Bitmap :: struct (Width: s16, Height: s16) #modify { return Width >= Height, "Width must be >= Height"; } { pixels: [Width*Height] u32; }
Holder :: struct (N: int, T: Type) #modify { if N < 8 N = 8; return true; } { ... }   // dedup happens AFTER #modify
```

- Instantiation: `Holder(float, 5)`, named `Holder(N = 5, T = float)`, `Table(Key_Type=Workspace, Value_Type=*Summary)`, with a `null` argument for a procedure parameter meaning the default, with `..` variadic parameters (`Tagged_Union(..types)`), with unary-dot values (`Ticket(.SECOND)`), with anonymous struct types (`Blentity(struct { name := "x"; })`), and inside imperative scopes. Same arguments → same type (deduplicated after `#modify`). Missing/extra arguments: "Not enough struct arguments: Wanted 2, got 1.", "Struct instantiation is missing argument 'T'.", "Attempt to use polymorphic struct 'Holder' ..." for a bare use where an instantiation is required (in a *procedure header*, a bare `Holder` matches any instantiation).
- Parameters become members of the instantiation's constants block, accessible as `h.T`, `h.N`, `Holder(u8).N`; they are listed in `Type_Info_Struct.specified_parameters` with values in `constant_storage`. `Type_Info_Struct.polymorph_source_struct` points to the source; `nontextual_flags.POLYMORPHIC` marks the uninstantiated source.
- Members may use parameters in types and defaults (`values: [N] T; base.size = size_of(T);`), may declare constants from parameters (`NUM_SQUARES :: count_x*count_y;`), and may reference another parameter's struct constants (`value: holder.T` in a procedure header).
- Bakes on structs: `#bake_arguments`/`#bake_constants` produce partially or fully instantiated struct types. Bakes cannot depend on each other (`foo :: ($T: Type, $R: Type = Thing(T))` is not allowed).
- A non-polymorphic struct with polymorphic non-constant data members is an error. Polymorphic structs inside polymorphic procedures are supported. Printing a polymorphic struct type shows its parameters.

### 8.6 `#place`

`#place member_name;` inside a struct body makes subsequent member declarations overlay memory starting at `member_name`'s offset (a union-like overlay without a `union` block), until the next `#place` or the end of the struct. The overlaid members may be imported (`using`) sub-struct members or union members. Struct size is rounded up to the alignment after `#place`; initializers are generated correctly for overlapping regions (later defaults win; overlapping pointer/string initializers are handled). `#place` cannot target a constant declaration. Used by Math (`Vector4` exposing `xyz`, `component`, `floats`; `Matrix4` exposing `_11`, `coef`, `floats`; `Quaternion` exposing `xyz`) and by generated bindings for vtables. Exported as `Code_Directive_Place { ident }`.

### 8.7 Struct directives and flags

| Directive (after `struct`/`union` and its parameters, or after the closing `}`) | Effect |
|---|---|
| `#type_info_none` | no runtime `Type_Info` contents (members omitted); `print` cannot print it |
| `#type_info_procedures_are_void_pointers` | procedure-typed members are reported as `*void` in `Type_Info` (reduces type table) |
| `#type_info_no_size_complaint` | suppress the "this Type_Info is large" complaint |
| `#no_padding` | drop the struct's trailing padding; member offsets and struct alignment are unchanged (3.14) |
| `#foreign` | struct from foreign code (textual flag; informational) |
| `#modify { }` | see 8.5 |

`#align N` is *not* a struct directive: the reference rejects it on a struct declaration ("#align does not have any meaning on constant declarations, since they do not have storage."), so a struct's alignment only ever comes from its members (3.14).

These flags live in `Type_Info_Struct.textual_flags` (`FOREIGN 1, UNION 2, NO_PADDING 4, TYPE_INFO_NONE 8, TYPE_INFO_NO_SIZE_COMPLAINT 0x10, TYPE_INFO_PROCEDURES_ARE_VOID_POINTERS 0x20`) and may also be set from a metaprogram with `compiler_set_type_info_flags(T, .NO_TYPE_INFO | .PROCEDURES_ARE_VOID_POINTERS | .NO_SIZE_COMPLAINT)`. `nontextual_flags`: `NOT_INSTANTIABLE 4, ALL_MEMBERS_UNINITIALIZED 0x40, POLYMORPHIC 0x100`; `status_flags`: `INCOMPLETE 1, LOCAL 4`.

Only the types passed to `type_info()` (and those they reference) get full `Type_Info` in the executable (`Build_Options.runtime_storageless_type_info`).

---

## 9. Enums

```
Fruit :: enum { BANANA; APPLE :: 12; CHERRY; }        // base type s64 by default; auto values: 0, 12, 13
Color :: enum u8 { RED :: 0; GREEN; BLUE :: 5; };      // trailing ; allowed
Flags :: enum_flags u16 { A; B; C :: 0x40; D; }         // enum_flags auto values are successive powers of two: 1, 2, 0x40, 0x80
Result :: enum s32 { OK :: 0; FAIL :: -1; }             // negative values need a signed base type
Kind :: enum u32 #specified { X :: 1; Y :: 2; }         // every member must have an explicit value
Mode :: enum #complete { ... }                          // #complete on the declaration (also usable on if == as `if #complete`); #complete and #specified in any order
E :: enum_flags @Hi @There { x :: 1; }                  // notes on the declaration
Values :: enum { A :: 1; B :: 2; C :: Values.A | .B; ALIAS :: A; NEXT; }   // members may reference other members (qualified or bare), constant expressions, members of other using-imported enums (`1 << NSEventTypeLeftMouseDown`), and file-scope constants
Big :: enum u32 { COUNT :: 70; NONE :: 4294967295; }    // duplicate values allowed in plain enums
TEST_ALLOC : enum { NO; DEFAULT; FLAT_POOL; } : .NO;   // anonymous enum type in a typed constant; unary-dot value
using Colors;  using E :: enum { ... }                  // import members as bare names
x: enum u8 { A; B; }                                    // anonymous enum member/variable type
```

Semantics:

- The base type may be any integer type; default `s64`. Values are constant expressions of integer type, assignable to the base type without loss (out-of-range is an error). An unspecified value is the previous member's value + 1 (0 for the first; for `enum_flags`, the next power of two after the previous *value* — 1 for the first). Aliases (`B :: A`) continue counting from the alias's value.
- `#if` blocks (8-space or otherwise) may appear inside enum bodies to conditionally declare members; `using` and `#insert` may appear inside enum bodies (`#insert`ed identifier-only statements get values by insertion order). Only constants of the enum type are allowed as members (a string constant is an error); `struct`/statements inside are errors.
- Member access: `Fruit.APPLE`, unary-dot `.APPLE` where the type is known (5.12), bare after `using`. Enum members through a *value*'s type (`v.kind.DECLARATION`) and through struct nesting (`Type_Info_Struct_Member.Flags.CONSTANT`) are allowed; member enums of a nested struct via `it.CONSTANT` (instance-reaching-enum semantics) also compile in current betas.
- Conversions: integer *literals/constants* convert implicitly to any enum type (`e + 1`, `flags = 0`, `member + cast(E) 50`); enum → integer requires a cast (`cast(s64) e`, `xx e`); enum ↔ enum requires a cast; `cast(E) int_value` is unchecked. Enums convert to `bool` in conditions (nonzero). `Enum.loose` is an integer-compatible type admitting any value of the base type.
- Operators: `== != < <= > >=`, arithmetic `+ - * / %` between an enum and its own type or integer literals (result type: the enum), bitwise `& | ^ ~` (for `enum_flags` and plain enums alike), `<<`/`>>` (result integer), compound assignments (`|=`, `&=`, `+=`). `~.MEMBER` and `Flags.A | .B | .C` are allowed. `min`/`max`/`clamp` from Basic accept enums.
- `enum_flags` differs from `enum` only in auto-value assignment and printing (`print` shows `A | B | 0x8` and the name for zero if one exists); `Type_Info_Enum.enum_type_flags.FLAGS`.
- `#specified` forbids implicit values (`Enum_Type_Flags.SPECIFIED`); `#complete` marks intent for `if #complete` (`COMPLETE`).
- Runtime info: `Type_Info_Enum { name; internal_type: *Type_Info_Integer; names: [] string; values: [] s64; status_flags; enum_type_flags }` (values are stored as `s64` even for unsigned bases; `type_info(E)` is typed `*Type_Info_Enum`). Basic provides `enum_names`, `enum_values_as_s64`, `enum_values_as_enum`, `enum_highest_value`, `enum_range`; Reflection provides name↔value helpers.
- Exported as `Code_Enum { internal_type_inst; internal_type; external_type: *Type_Info_Enum; block; notes; marked_as_complete; marked_as_specified; is_flags }`.

---

## 10. The context

### 10.1 Overview

Every non-`#c_call`, non-`#no_context` procedure receives an implicit pointer to a *context* struct of type `#Context`. The keyword `context` is an lvalue of that type. Modifications made through `context.x = ...` are visible to callees and persist after the callee returns (it is shared memory), unless made inside a `push_context` block or `,,` call, which operate on a copy. Each thread owns its own context (thread-local); `Thread` creates a fresh one for new threads. Context values may be copied (`new_context := context;`).

`#Context` is the compiler-generated struct type. `#Context` implicitly converts to `Context_Base` (its first member is `#as using base: Context_Base`). The name `Context` is *not* declared by the compiler (since 0.2.002); modules use `#Context`. The struct's first field is `context_info: *Type_Info_Struct`, always initialized to `type_info(#Context)` (used by DLLs and `Remap_Context`).

### 10.2 `#add_context`

`#add_context name: T = default;` (anywhere in a data scope, including inside modules and `#if` blocks) adds a member to `#Context`. Conflicting names are errors. Members other than `base` are sorted alphabetically for a stable layout across binaries. The context is padded to `Build_Options.context_size_max` bytes (default 4096; `-context_size`) and cannot grow beyond it; its layout may be back-patched during compilation (0.2.001). Basic adds `print_style`, Memory_Debugger adds `inside_memory_debugger`, Bindings_Generator adds `generator`, Program_Print adds `program_print`, Android adds `android_app`, user programs add their own (`#add_context backchannel: *Backchannel;`, `#add_context filename: string;`). Errors: `#add_context` inside a general expression.

### 10.3 `Context_Base` (Runtime_Support)

```
Context_Base :: struct {
    context_info: *Type_Info_Struct;              // type_info(#Context)
    thread_index: u32;
    allocator := default_allocator;               // Allocator { proc, data }
    logger := runtime_support_default_logger;     // Logger
    logger_data: *void;
    log_source_identifier: u64;
    log_level: Log_Level;                         // NORMAL, VERBOSE, VERY_VERBOSE
    temporary_storage: *Temporary_Storage;
    stack_trace: *Stack_Trace_Node;               // null when stack traces are disabled
    assertion_failed := runtime_support_assertion_failed;   // (loc: Source_Code_Location, message: string) -> bool (true = break)
    handling_assertion_failure := false;
    default_allocator :: Allocator.{runtime_support_default_allocator_proc, null};   // constant
}
```

The layout is fixed ("@Volatile: must match the compiler"). `#add_context` members follow `base`.

### 10.4 Allocators

```
Allocator_Proc :: #type (mode: Allocator_Mode, requested_size: s64, old_size: s64, old_memory: *void, allocator_data: *void) -> *void;
Allocator :: struct { proc: Allocator_Proc; data: *void; }
Allocator_Mode :: enum { ALLOCATE :: 0; RESIZE :: 1; FREE :: 2; STARTUP :: 3; SHUTDOWN :: 4; THREAD_START :: 5; THREAD_STOP :: 6; CREATE_HEAP :: 7; DESTROY_HEAP :: 8; IS_THIS_YOURS :: 9; CAPS :: 10; }
Allocator_Caps :: enum_flags { MULTIPLE_THREADS; CREATE_HEAP; FREE; ACTUALLY_RESIZE; IS_THIS_YOURS; HINT_I_AM_A_FAST_BUMP_ALLOCATOR :: 0x0100_0000; HINT_I_AM_A_GENERAL_HEAP_ALLOCATOR; HINT_I_AM_PER_FRAME_TEMPORARY_STORAGE; HINT_I_AM_A_DEBUG_ALLOCATOR; }
```

`context.allocator` is used by `alloc`, `free`, `realloc`, `New`, `NewArray`, `array_add`, `copy_string`, `String_Builder`, etc. `temp` (Basic) is the temporary allocator (`Allocator.{temporary_allocator_proc, null}`) backed by `context.temporary_storage` (a linear per-thread arena reset by `reset_temporary_storage()`; overflow pages come from the overflow allocator). `Temporary_Storage` layout is fixed (`data, size, current_page_bytes_occupied, total_bytes_occupied, high_water_mark, last_set_mark_location, overflow_allocator, overflow_pages, original_data, original_size`) with `TEMPORARY_STORAGE_SIZE` from `Build_Options.temporary_storage_size` (32768). A `[..] T`, `Hash_Table`, `String_Builder`, `Pool` etc. remember their allocator; an `Allocator` whose `proc` is null means "use `context.allocator` at time of use". The default allocator is rpmalloc (stripped) from `Default_Allocator`, imported by the compiler; it may be replaced by remapping the `Default_Allocator` module (`remap_import(w, "*", "Default_Allocator", "Walloc")`). Mode semantics: `ALLOCATE` returns `requested_size` bytes (8-byte aligned for temp; 16 for the heap); `RESIZE` receives `old_memory`/`old_size`; `FREE` receives `old_memory`; `STARTUP/SHUTDOWN/THREAD_START/THREAD_STOP` are notifications; `CREATE_HEAP`/`DESTROY_HEAP` create sub-heaps; `IS_THIS_YOURS` returns non-null if the pointer belongs to the allocator; `CAPS` returns `Allocator_Caps` cast to `*void` and writes a version string through `old_memory` if non-null.

### 10.5 Logging and assertions

`Logger :: #type (message: string, data: *void, info: Log_Info);` `Log_Info :: struct { source_identifier: u64; location: Source_Code_Location; common_flags: Log_Flags; user_flags: u32; section: *Log_Section; }` `Log_Flags :: enum_flags u32 { NONE :: 0; ERROR :: 1; WARNING :: 2; CONTENT :: 4; TO_FILE_ONLY :: 8; VERBOSE_ONLY :: 0x10; VERY_VERBOSE_ONLY :: 0x20; TOPIC_ONLY :: 0x40; }` `Log_Level :: enum u8 { NORMAL; VERBOSE; VERY_VERBOSE; }` `Log_Section :: struct { name: string; }`. `log(fmt, ..args, loc := #caller_location, flags := .NONE, user_flags := 0, section := null)` and `log_error` (sets `.ERROR`) in Basic call `context.logger`; the default logger writes to stdout (stderr for `.ERROR`) appending a newline if missing. `assert(cond, message := "", ..args, loc := #caller_location)` is a `#no_debug #expand` macro: if the condition fails it calls `context.assertion_failed(loc, tprint(...))` (guarded by `handling_assertion_failure`) and executes `debug_break()` if that returns true. The runtime default prints "file:line,char: Assertion failed: msg" and a stack trace to stderr, sets `__runtime_support_disable_stack_trace`, and exits non-zero (an assertion in compile-time code makes the compiler exit non-zero). `ENABLE_ASSERT=false` (Basic program parameter) makes `assert` a `#discard` no-op.

### 10.6 Stack traces

With `Build_Options.stack_trace` (default true; `-release` sets false), every non-leaf procedure call pushes a `Stack_Trace_Node { next; info: *Stack_Trace_Procedure_Info { name; location; procedure_address }; hash: u64; call_depth: u32; line_number: u32 }` onto a linked list rooted at `context.stack_trace` (updated at each call site with the current line; hashes differ per call site). C callbacks start with a sentinel node. `Basic.pack_stack_trace()`, `print_stack_trace()`, `log_stack_trace()` and `get_stack_trace_string()` consume it. With LLVM the trace always includes the current procedure.

---

## 11. Modules, files and program structure

### 11.1 Files and `#load`

A program starts from one or more source files given to the compiler (via the default metaprogram). `#load "relative/path.jai";` (data scope) textually includes another file as a **new file scope** under the same module/application scope; the path is relative to the loading file's directory (inside inserted strings, relative to the file containing the `#insert`). Loading the same file twice into one scope creates duplicate declarations (error). `#load` inside `#if` is allowed. Case-insensitive path matches are rejected on Windows. Loaded files see the module scope like any other file. Exported as `Code_Directive_Load { short_name; fully_pathed_filename; loaded_string; load_flags }` and as a `Message_File` to metaprograms.

### 11.2 Modules and `#import`

```
#import "Basic";                          // import module Basic into the current scope (its exported names become visible)
#import "Basic"()(MEMORY_DEBUGGER=true);  // module parameters () and program parameters ()
#import "Hash_Table"(COUNT_COLLISIONS=true);
#import "Codex"(USAGE_MODE=.READ);
#import "Android"()(main);               // positional program parameter (a procedure)
Math :: #import "Math";                   // named import: members via Math.x; does not add to global overload sets
using Sound :: #import "Sound_Player";
#import,file "path/to/file.jai";          // a single file as a module (relative to the importing file)
#import,dir "../Vulkan_Render";           // a directory module (loads module.jai)
#import,string "code";                    // a module from a source string (must be a literal; #string allowed with a trailing ;)
#import,unshared "X";                     // (flag exported as UNSHARED) a private instantiation
using,except(Node) Trees :: #import,file "trees.jai";
#import "Toolchains/Android";             // subdirectory path inside the modules folder
```

- Module lookup: for `#import "Name"`, each directory in `Build_Options.import_path` is searched **in order** for `Name.jai` (single-file module) or `Name/module.jai` (directory module); the first directory containing either wins; both present in the same directory is an ambiguity error. The default `import_path` is `[ "<dir of the first source file>/modules", "<compiler dir>/modules" ]` (a local `modules` folder overrides the compiler's), plus `-import_dir` entries (prepended). Module names may contain `-`, digits and `/` (`Frotz-6_9_105`, `Android/EGL`). Windows case sensitivity is enforced.
- Each module instantiation gets a **module scope** under Preload; a module sees only Preload, its own files, and what it imports. Modules do not see the application scope. Importing a module twice with the same (textual) parameter list yields the same instantiation; different parameter lists create separate instantiations with separate globals. Module scope declarations are per-instantiation.
- `#import` may appear in any data scope or imperative scope (procedure bodies, `#run` blocks, `#modify` blocks, macros, `#if` branches). Its names go into the scope where it appears: an `#import` inside a procedure body is visible in that block, and a file-level one is visible throughout the module — **including under `#scope_file`**, which narrows the declarations a file *makes* but not the names an unnamed `#import` brings in. (Checked against beta 0.2.009: a file whose only `#import "Basic"` sits after `#scope_file` still lets a sibling file call `print`, while a `SECRET :: 42` or a named `Str :: #import "String"` after the same directive stays private to its file.) Imported names are never re-exported to importers of this module. Bare `#import "X"` inside a procedure that would add overloads to an already-sealed set could error historically; named imports avoid this.
- What a module makes its own with `using` *is* visible to importers: `using E :: enum { … };` at export scope puts `E`'s members in the module scope, and `#import`ing that module finds them (POSIX exports the `_SC_*` constants of `using _SC_definitions :: enum s32 { … }` this way). A plain `#import` inside the module contributes nothing to what the module exports.
- `using X;` where `X` was bound by a named import widens the scope with the module's exported names, exactly as `using X :: #import "…"` does; the two halves need not be adjacent, and the `using` may be written before the `#import` that binds the name. `String` is written this way (`Basic :: #import "Basic"; using Basic;`), which is what lets it both call `assert` unqualified and reach `Basic.alloc` by name.
- Failed imports: a module that cannot be found raises a `FAILED_IMPORT` message to the metaprogram, which may `provide_import` a replacement (once); `remap_import` redirects or blocks imports before compilation starts (`"*"` wildcards). `Message_Failed_Import.import_code` is the site.
- Every module is compiled from source every build; there is no caching.
- Exported as `Code_Directive_Import { name; flags; import_type: Provided_Import_Type; module_parameters_call; program_parameters_call }` and as a `Message_Import { module_type: PRELOAD | RUNTIME_SUPPORT | MAIN_PROGRAM | FILE; module_name; fully_pathed_filename }` (one per instantiation; the main program's module_name is `""`).

### 11.3 Module structure and `#module_parameters`

A module's first file (`Name.jai` or `module.jai`) may begin with:

```
#module_parameters (VERBOSE := false, LOAD_FACTOR := 70) (ENABLE_ASSERT := true, MEMORY_DEBUGGER := false) {
    // optional "common code" block: declarations usable in the parameter lists (types, procedures), compiled once and shared across instantiations; cannot see the rest of the module
    Memory_Debugger_Interface :: struct { ... }
};
#module_parameters (DEFINE_SYSTEM_ENTRY_POINT: bool, DEFINE_INITIALIZATION: bool, ENABLE_BACKTRACE_ON_CRASH: bool);   // no defaults: must be supplied
```

- The first list is the **module parameters**: constants inside the module scope (not exported), set per import (`#import "X"(VERBOSE=true)`). Each distinct textual argument list is a separate instantiation; an *empty* list supplies nothing, so `#import "Basic"()(MEMORY_DEBUGGER=true)` is the same instantiation as a bare `#import "Basic"` and only the program parameters differ.
- The second list is the **program parameters**: set once by the main program (`#import "Basic"()(ENABLE_ASSERT=false);`, must precede any other import of that module); all imports (including from other modules) share them; modules cannot set them. Passing parameters to a module without `#module_parameters` is an error. Parameter types may be `$I/interface X` with defaults, procedures, enums, integers, bools.
- The module's files use `#load` for the rest of the module; `#scope_module`/`#scope_file`/`#scope_export` control visibility. `#assert(is_constant(VERBOSE));` is common.
- Exported as `Code_Directive_Module_Parameters { module_parameters: *Code_Procedure_Header; program_parameters; common_code }`.

### 11.4 Preload and Runtime_Support

`Preload` (module_type `PRELOAD`) is imported implicitly by the compiler into every workspace and defines the runtime type system (`Type_Info*`), `Allocator`, `Context`-related types, intrinsics, `OS`, `CPU`, and other compiler-referenced declarations (section 17). `Runtime_Support` (module_type `RUNTIME_SUPPORT`; two `Message_Import`s are sent: as RUNTIME_SUPPORT and as FILE) is imported by the compiler with parameters chosen from `Build_Options.runtime_support_definitions` and defines `Context_Base`, `Temporary_Storage`, the entry point, panic handlers, and default logger/allocator/assertion procedures. `Default_Allocator` is imported by Runtime_Support. Users may replace Runtime_Support via `remap_import`.

### 11.5 The main program and entry point

The main program (module_type `MAIN_PROGRAM`, name `""`) must declare a procedure named by `Build_Options.entry_point_name` (default `main`), with signature `main :: ()` (no parameters; command-line arguments come from `Basic.get_command_line_arguments()` / `__command_line_arguments: [] *u8`). Runtime_Support's `__system_entry_point` (exported as `main` for the C runtime, or the user-provided one) calls `__jai_runtime_init(argc, argv) -> *#Context`, pushes the first thread's context, initializes the crash handler, calls `__instrumentation_first()`, `__instrumentation_second()` (plugin hooks), then `__program_main()` (declared `#entry_point`), then `__jai_runtime_fini`. `main` may itself be declared inside a `#if`, generated by `#insert`, or run at compile time (`#run main();`). For `.DYNAMIC_LIBRARY`/`.STATIC_LIBRARY`/`.OBJECT_FILE` outputs no `main` is required (`runtime_support_definitions` chooses whether init/entry code is included).

### 11.6 Dead code elimination

`Build_Options.dead_code_elimination` (`MODULES_ONLY` default, `ALL`, `NONE`; `-no_dce`): procedures that are never referenced are parsed and header-typechecked but their bodies are not typechecked or compiled (like macros); consequently `#run`, `#import`, `#assert` and errors inside dead procedure bodies do not take effect. `compiler_make_procedure_live(w, header)` forces a body live. With `MODULES_ONLY`, all procedures of the main program are live.

### 11.7 Order independence and dependency resolution

Data-scope declarations are resolved by a scheduler: each declaration waits for the identifiers it needs. Circular dependencies are errors reported with the cycle (including through `#run`, `#if`, `using`, `#import`, `#module_parameters` and the context). Some patterns that are *not* cycles: a struct containing `[] S`/`[..] S`/`*S` of itself; mutually recursive procedures in data scopes; a `#run` referencing a global whose initializer references a constant. Identifier resolution involving name-inserting constructs (`using`, `#import`, `#insert`, compound declarations, `#if` declaring names) waits for those constructs when they might provide the name; two `#if`s in one data scope that each declare something the other reads deadlock — use `#placeholder`. `#run,stallable` marks a `#run` that may block on unresolved dependencies without deadlocking another stallable run (`VULKAN_PATHS :: #run,stallable -> Vulkan_Paths { ... }`). Undeclared identifiers are reported (batched per file, with suggestions such as "declared at file scope elsewhere → use `#scope_export`", and with unfilled placeholders listed).

### 11.8 `#placeholder`

`#placeholder NAME;` in a data scope declares that `NAME` will be defined later by a metaprogram (`add_build_string` into that scope) or by an `#insert`. Lookups of `NAME` wait until it is filled instead of failing; an unfilled placeholder at the end of compilation is an error listing near-miss declarations. Replacing a placeholder from outside its scope, or the same placeholder via two modules, is an error. `#poke_name` fills module-scope placeholders.

### 11.9 `#poke_name`

`#poke_name Module_Name identifier;` injects a declaration `identifier` (visible in the current scope) into the *module scope* of the named import `Module_Name` (a named `#import` binding), so that the module can refer to a user-provided symbol (e.g. a hash function or a `#placeholder` filled from outside). Exported as `Code_Directive_Poke_Name { module_struct; name }`.

### 11.10 Program parameters of interest

Basic: `MEMORY_DEBUGGER`, `ENABLE_ASSERT`, `REPLACEMENT_INTERFACE`, `VISUALIZE_MEMORY_DEBUGGER`, `TEMP_ALLOCATOR_POISON_FREED_MEMORY`. Runtime_Support: `DEFINE_SYSTEM_ENTRY_POINT`, `DEFINE_INITIALIZATION`, `ENABLE_BACKTRACE_ON_CRASH` (module parameters). Android: `DEFINE_ANDROID_MAIN_AND_CALL_THIS`. Thread: `DEBUG`, `CACHE_LINE_SIZE`.

---

## 12. Compile-time execution

### 12.1 `#run`

```
X :: #run compute();                    // expression: the result becomes the constant X
#run print("hi");                       // statement at file scope: executed once during compilation
#run { ...statements... }               // block; trailing ; optional
#run { ... };
#run -> string { ...; return s; }       // block with return type(s); ≡ #run () -> string { ... }()
a, b := #run -> Type1, Type2 { ... };   // multiple return values
#run,stallable build();                 // may block waiting for unresolved declarations
V :: #run,stallable -> Vulkan_Paths { ... };
#run main();                            // run the program's main at compile time
SIZE :: #run enum_highest_value(E) + 1;
MESH_SHADER :: #run sprint(#string END ... END, MAX_JOINTS, MAX_WEIGHTS);
#run { #import "Basic"; helper :: () {}; ... }   // imports and declarations inside a #run block
#if #run get_current_workspace() == 0 { }
offset = #run offset_of(Vertex, "position")     // inside a constant struct literal
```

- A `#run` is a procedure body compiled to bytecode and executed by the compiler's interpreter as soon as its dependencies are typechecked; the order between independent `#run`s is nondeterministic (scheduled across threads). `#run` in a procedure body executes when that body is typechecked (once per polymorph instantiation); in a dead procedure it never runs.
- The result of a `#run` expression is converted to a constant: integers, floats, bools, enums, strings, `Type`s, `Code`, non-foreign procedures, pointers (mapped to global data when possible; heap pointers cannot be mapped and produce a warning/garbage), structs (with the above members; unions/`#place` overlaps are "weird"), fixed and view arrays of those (large arrays that map to a known data segment are not copied back into the AST; threshold `Build_Options.maximum_array_count_before_compile_time_returns_are_not_reflected_in_ast` = 5000), `#type,isa` variants, `#code,null`, and multiple return values. A `#run` whose expression is already constant does not execute.
- The compile-time environment is the full language: heap allocation, file I/O, processes, threads, `#c_call` into loaded dynamic libraries (`#foreign` procedures with a library are callable; library-less `#foreign`/`#elsewhere` are not), callbacks from C into bytecode, `#asm` (compiled to machine code), `debug_break()` (enters the interactive bytecode debugger with `-debugger`, otherwise reports a crash with a user-level stack trace), `#compile_time` (true), `get_current_workspace()` (nonzero). A crash or assertion failure in compile-time code fails the build. Global variables have storage in the compiler process; their final values are discarded (reset to initializers) when the executable is written unless `#no_reset` (4.7). Read-only literal data is not memory-protected at compile time.
- `#run` and `#assert` are not allowed in argument or return lists of procedure/struct declarations, nor in type restriction slots.
- `#run` of a `#compile_time` procedure or of a procedure that calls compile-time-only things is fine; calling a compile-time-only procedure at runtime is an error ("Attempt to call a compile-time function at runtime" / panic via `__panic_due_to_runtime_call_of_compile_time_procedure`).
- Exported as `Code_Directive_Run { procedure: *Code_Procedure_Header; flags: ASSERTION | STALLABLE | SYNTACTICALLY_IMPLICIT | HAS_IMPLICIT_RETURN_TYPES; assertion_string }`; `#assert` is a `#run` with `ASSERTION`.

### 12.2 `#assert`

`#assert cond;`, `#assert cond "message";` (no comma), `#assert(cond);`, `#assert(cond, "message")`? (the parenthesized call form takes the expression only; the message follows as a string), `#assert,stallable cond;`, `#assert !(flags & .REVERSE) "msg";`, `#assert(names_are_equal("a", "b"));` (calls are allowed; evaluated at compile time), `#assert Dimwit.procedure == Flathead.procedure;`. The condition must be a constant expression (a runtime value is an error even in unreachable code); the assertion is evaluated when the enclosing body is live and cannot influence overload resolution. Failure reports the message and the chain of imports/polymorphs and fails the workspace. The wording, measured with the reference compiler, is `Compile-time assertion failed.` with the message appended in quotes when one was written (`Compile-time assertion failed. "one is not two"`); the span underlined is the `#assert` directive itself, not the condition.

### 12.3 `#no_reset`, `#compile_time`, `#run` interaction with globals

See 4.7 and 5.14. The interpreter and the runtime share one view of global data during compilation; `Runtime_Info.global_data_info` describes the segments at runtime (BSS, DATA, RDATA, NO_RESET, USER).

---

## 13. Metaprogramming: `Code`, `#insert`, and the AST

### 13.1 `Code` values

| Form | Meaning |
|---|---|
| `#code expr` | an unevaluated, **untypechecked** expression; identifiers are not resolved until inserted somewhere (so they may not exist at the `#code` site) |
| `#code { stmts }` | a block of statements; `#code x = 3;` statement form without braces is allowed |
| `#code,typed expr` | typecheck the expression in the current scope now; `.type` and `get_root_type` work |
| `#code,null` | the null Code; tests false in `if`; `c.type` on it is an error; `#insert`ing it inserts nothing; a default `$code := #code,null` parameter |
| `code_of(x)` | the definition Code of a declaration / the Code of an expression after constant substitution |
| `#caller_code` | the call-site expression (macro parameter default) |
| implicit | a non-Code argument to a `Code` parameter of a macro (scope = call site) |
| `c.type` | for a constant Code (`$c: Code`, or a Code parameter in a macro): the `Type` of the root expression |
| `Compiler.get_root_type(c) -> (status, Type)` | for non-constant Code (`NOT_TYPED` for untyped `#code`) |
| `compiler_get_nodes(c) -> (root: *Code_Node, all: [] *Code_Node)` | fresh, mutable copy of the AST (compile time only) |
| `compiler_get_code(node, scope_source := #code,null) -> Code` | wrap nodes (optionally copying the scope of another Code) |

`Code` values are compile-time constants; `Code == Code` compares identity for polymorph matching; a `Code` cannot be stored in runtime data. Codes capture the scope where they were written (or their call site when implicitly generated) for later identifier resolution.

### 13.2 `#insert`

```
#insert "a := 7; b := 5;";            // statement-level: parse the string as statements/declarations in this scope; a and b are visible afterwards
#insert s;                             // s: a constant string (e.g. a $-baked parameter or a struct parameter)
#insert code;                          // code: Code (macro parameter, constant)
#insert #run gen(args);                // string or Code produced at compile time
#insert #run () -> string { ... }();   // explicit anonymous procedure
#insert -> string { return "..."; }    // short form (implicit #run)
#insert -> Code { return #code ...; }
x := 1 - #insert a;                    // expression-level: the string must be one expression statement ending with `;` ("factorial(7);"); no declarations
#insert,scope() code;                  // resolve identifiers in the CURRENT (macro) scope rather than the code's origin scope
#insert,scope(target) code;            // resolve in the scope where `target` (a constant Code) lives
#insert(break=break y, continue=continue y, remove=remove y) body;   // remap loop controls inside the inserted body
x.(#insert y)                          // inside a member dereference
Container :: struct (s: string) { #insert s; }   // struct bodies: generated members; `#insert #run gen()` adds default assignments
E :: enum { #insert "A; B; C :: 5;"; }           // enum bodies
```

- `#insert` works in data scopes (file, struct, enum), imperative scopes and expression positions; at toplevel it obeys the active `#scope_*` directive. Inserted strings are lexed with the location of the `#insert`; `#load`/`#library` paths inside resolve relative to the file containing the `#insert`. Extra junk after the last statement of an inserted string is an error. Nested `#insert`s inside inserted strings work.
- Inserted Code is hygienic relative to its origin scope by default (identifiers resolve where the `#code` was written); `,scope()` overrides. Code that a `#run` handed back is the exception: it was built where nothing of the program is in scope, so `#insert #run …` and its `#insert -> Code { … }` short form resolve the names of what comes back at the insertion point.
- An `#insert` written inside a polymorphic body or a polymorphic struct expands once per instantiation, since its text is whatever the constants make it — and so does a `#run` written inside one. The names one expansion declares belong to that instantiation: a sibling specialization neither sees them nor collides with them. Loop controls in inserted code apply to the loop containing the insertion point unless remapped. A `return` inside inserted Code belongs to the procedure containing the insertion (or the macro, if inserted in a macro — a macro's `#insert code` treats `return` as the macro's).
- Nodes added by `#insert` are exported to metaprograms via `Code_Directive_Insert.expansion` and `Typechecked.subexpressions`. `Code_Directive_Insert { expression; scope_redirection; break_replacement; continue_replacement; remove_replacement; expansion; is_internal }`.
- Name-lookup interaction: see 11.7 and 11.8.

### 13.3 Printing types and code

A `Type` printed with `%` yields source-like text usable in generated code (`print("%: %;", name, T)` → `x: *[17] **void;`). `Program_Print.print_expression(*builder, node)` prints AST nodes back to source. `Basic.print_type_to_builder` prints `*Type_Info`.

### 13.4 The message API

The compiler exposes its work to a *metaprogram* through `#import "Compiler"`: workspaces, build options, and a stream of messages (`compiler_wait_for_message`) whose `TYPECHECKED` messages carry the typed AST (`Code_Node` structs). The full API is specified in `docs/compiler.md`. Language-level guarantees: a metaprogram can add source (`add_build_file`, `add_build_string` into a file/module/global scope), replace failed imports, modify procedure bodies (`compiler_modify_procedure`), inject global data (`add_global_data`, `add_data_segment`), set type-info flags, control output and linking, and receive `FILE`, `IMPORT`, `FAILED_IMPORT`, `PHASE`, `TYPECHECKED`, `ERROR`, `DEBUG_DUMP`, `PERFORMANCE_REPORT`, `COMPLETE` messages.

---

## 14. Foreign interface

```
libc     :: #system_library "libc";            // system library: linked by name (search per platform rules; Linux uses /etc/ld.so.conf paths, plain name — you may need -dev packages)
libc     :: #library,system "libc";            // same (preferred spelling; #system_library to be deprecated)
crt      :: #system_library "libm";
kernel32 :: #system_library,no_dll "kernel32";
mylib    :: #library "path/relative/to/this/file/libname";   // user library: the compiler looks for libname.so / libname.a (Linux), .dll/.lib, .dylib
mylib    :: #library,no_dll "x";               // no dynamic library available (static only; also not loadable at compile time)
mylib    :: #library,no_static_library "helper";   // no .a; required on Windows for #elsewhere globals from a DLL
mylib    :: #library,link_always "x";          // never cull this library from the link line even if unreferenced
librt    :: libc;                              // alias
f :: (a: s32, b: *u8) -> s32 #foreign libc;                   // C function; implies #c_call
f :: (a: s32) -> s32 #foreign libc "real_symbol_name";         // renamed
g :: () #foreign;  h :: () #foreign "malloc";                  // no library: resolved at link time (WASM env / cross builds); not callable at compile time
v: s32 #elsewhere libc "environ";                              // extern data
p :: (s: string) -> string #elsewhere Helper;                  // Jai-ABI import from a Jai DLL
q :: (fmt: *u8, args: ..Any) -> s32 #foreign libc "printf";   // C varargs
r :: (x: sigval_t) #c_call;                                    // procedure type with unnamed parameter (as a member type)
t :: (a: s32) -> s32 #foreign libc #deprecated "use x";
```

Rules:

- A `#library`/`#system_library` declaration must be constant (non-constant is an error), may appear in data scopes, inside `#if`, inside procedure bodies, and in `#insert`ed text; its path is relative to the declaring file. A library declaration in runtime code is diagnosed. Libraries referenced only by unused `#foreign`/`#elsewhere` declarations are culled from the link line unless `,link_always`. Compile-time execution loads the *dynamic* library to call foreign procedures; missing dynamic libraries produce an error showing the chain of imports (`,no_dll` opts out). On Linux, `.so.N` suffixes are searched automatically for system libraries; 32-bit libraries are skipped; full filenames like `"libatomic.so.1"` are accepted.
- `#foreign` procedures are `#c_call` (System V AMD64 on x64, AAPCS64 on arm64). Two Jai declarations may bind the same C symbol with different signatures. Parameter names are informational (named arguments to foreign procedures are allowed: `vkWaitForFences(device, fenceCount = 1, ...)`, `memcpy(dest = ..., source = ...)`). Jai types map to C as: `s8..u64` ↔ integers, `float`/`float64` ↔ float/double, `bool` ↔ 1 byte, pointers, `*u8` for `char*` (constant strings convert implicitly), structs by value per the C ABI (empty structs OK), fixed arrays are **not** passable by value (pass `*[N] T` or `*T`; returning fixed arrays from `#c_call` is an error), `string`/`[] T`/`[..] T` pass as structs of their layout, enums as their base integer, `Any` as a 16-byte struct, `#type,isa` integer variants as the base, procedure types as function pointers, `..Any` as C varargs (each argument passed per the C default promotions; passing a fixed array or spreading into C varargs is an error; a Jai procedure (non-foreign) may not be declared `#c_call` with varargs).
- Generated bindings (`Bindings_Generator`) follow conventions used throughout the modules: opaque handles `X_T :: struct {} X :: *X_T;`, bitfields as `__bitfield`, `#align` on members, `#elsewhere` for globals, `#library,system` for system libs, enums with stripped prefixes, doc comments preserved, `_context` for C parameters named `context`, and `#foreign` overloads that wrap the two-call enumeration idiom in Jai procedures of the same name.
- `#program_export` (optionally with a name) exports a Jai procedure or global with C linkage for use from other languages/DLLs; combined with `#c_call` and `push_context { ... }` inside it for a C-callable Jai API. `Remap_Context` adapts a caller's differently-laid-out `Context` (matched by `context_info`) for Jai DLLs.
- `#cpp_method` / `#cpp_return_type_is_non_pod` cover C++ conventions (Windows/Linux differ) for generated C++ bindings; `#cpp_method` procedures take `this` as their first pointer parameter.
- Structs shared with C should be declared `#foreign` (informational) and match layout; `#no_padding` for packed C structs; `[0] T` flexible members; `union` and `#place` for C unions.
- `Build_Options.lazy_foreign_function_lookups`, `use_custom_link_command` (+ `READY_FOR_CUSTOM_LINK_COMMAND` phase and `compiler_custom_link_command_is_complete`), `additional_linker_arguments`, `Build_Options_During_Compile.append_linker_arguments`, `compiler_add_library_search_directory` control linking.

---

## 15. Inline assembly (`#asm`)

`#asm { instr operands; ... }` embeds x86-64 assembly. The reference compiler has no other kind; orangejuice also assembles arm64 blocks, which is an extension and is specified in §15.9 rather than here. Grammar summary (Intel/AMD mnemonics without the `v` prefix; VEX/EVEX encoding chosen by the block's feature set):

```
#asm { mov apple:, 10; }                       // `name:` declares a register operand with inferred class
#asm { banana: gpr; mov.64 banana, 17; }       // declared class: gpr, str (mask/segment?), vec, omr; sizes .8 .16 .32 .64 .128 .256 .512
#asm { popcnt?T result, value; popcnt?BITS r, v; }   // size from a Type or constant integer
#asm AVX, AVX2 { ... }                         // feature tags (names from Machine_X64.x86_Feature_Flag); .x/.y/.z auto sizes; SYSCALL_SYSRET tag for syscall
#asm { t: gpr === a; v: vec === 9; mov w: gpr === 15, 10; x === a; }    // pinning to specific registers (a b c d si di 8..15); high-level vars can be pinned
#asm { mul z, x, y; }                          // implicit-register instructions list every operand (pinned)
#asm { add count, 17; }                        // high-level locals and constants used directly (loaded into registers; taking their address forces memory)
#asm { mov.d [feature_flags_0], d; movups c:, [*b]; add [base + index*4 + 8], 1; }   // memory operands [base + index*scale + disp] with rigid order; base must be a gpr; by-reference struct access needs *; struct values ≤ register size move by value
#asm { pxor.x x:, x; } #asm { movdqu y:, x; } // registers declared in one block are visible in following blocks of the same scope (no scopes are created)
b1 :: #asm { pxor x:, x; } b2 :: #asm { movdqu y:, b1.x; }   // named blocks for cross-block references
#asm { syscall t1:, t2:, call, fd, buf, count; }   // syscall lists clobbered outputs (rcx, r11) then inputs pinned to a, di, si, d, ...
#asm { [ptr]!, v5: &* mask, [ptr + vindex*4] }  // EVEX broadcast, zeroing/merge masking, VSIB
#asm { mov.32 f1:, 133.247; }                  // float immediates for opaque operands
#asm { int 0x41; pause; cpuid a, b:, c, d:; rdtsc high:, result; }
add_regs :: (c: __reg, d: __reg) #expand { #asm { add c, d; } }   // macros taking registers
#if BITS == 8 #asm { ... } else #asm { ... }
if cond #asm { ... }
```

Semantics: no automatic spilling (too many live registers is an error); lifetime-based allocation; the stack pointer may not be manipulated manually; `#asm` blocks work at compile time (assembled to machine code); the LLVM backend respects flag-modifying instructions; procedures containing `#asm` are not inlined; `#asm` operands are opaque to metaprograms (`Code_Asm`). Old letter size suffixes (`.b .w .d .q .x .y .z`) are still accepted. Manual `syscall`s are how Runtime_Support performs `write`, futex and `exit_group` on Linux.

### 15.9 arm64 blocks (an orangejuice extension)

**The reference compiler's `#asm` is x86-64 and nothing else.** A block written for one architecture does not mean anything on the other, and nothing here is compatibility: an arm64 block is orangejuice's own, and a program that wants both writes `#if CPU == .X64`. What is shared is everything that is not the instruction set — a block declares its registers into the scope around it, they are placed by the same lifetime-based allocator with no spilling, a register declared in one block is visible in the next, and a variable of the program stays the back end's to place.

What differs is the shape of an instruction. x86-64 is two-operand and destructive, where `add a, b` means `a += b`; AArch64 is three-operand, where `add d, n, m` means `d = n + m`:

```
#asm { mov a:, 17; sub a, a, 5; add total, total, a; }   // `total` is a variable of the program
#asm { ldr x, [base]; ldr y, [base + 8]; ldr z, [base + index*8]; }   // `[base, #8]` and `[base, index, lsl #3]`
#asm { adds sum, a, b; cset_cs carry; }                  // a condition is written into the mnemonic
#asm { fadd z, x, y; scvtf f, n; fcvtzs back, f; }       // scalar floating point, and across the register files
#asm { vdup.32 v: vec, seed; vadd.32 v, v, v; vaddv.32 s: vec, v; fmov out, s; }   // NEON lanes
#asm { t: gpr === x9; u: vec === v3; }                   // pinned by the register's own name
```

- **Registers.** A general-purpose register is written `w<n>` at 32 bits and below and `x<n>` above, and the block never has to say which: the width comes from the operand's type or the instruction's size tag. The allocator hands out `x0`–`x15` less `x8`, and `v0`–`v7` and `v16`–`v31`; `x16`/`x17` are the linker's veneer scratch, `x18` is reserved on Darwin, and `x29`/`x30` and the stack pointer belong to the procedure around the block.
- **Size tags** mean what they do on x86-64 for a general-purpose instruction. On a NEON instruction a tag is the *lane* width — `vadd.32` is `add v0.4s, v1.4s, v2.4s` — because a NEON register is 128 bits and the arrangement says how it is cut up.
- **Conditions** are spelled into the mnemonic, since a condition is not a name a program could have declared: `cset_eq`, `csel_lt`, and the rest of `eq ne lt le gt ge lo ls hi hs cs cc mi pl`.
- **NEON mnemonics that collide with a scalar one take a `v`** in the Jai spelling and reach the assembler as themselves: `vadd` is written and `add` is emitted, with vector registers.
- **Not expressible**, and reported rather than guessed at: `ld1`/`st1`, whose register list is written in braces; anything the table does not carry. `ldr q0, [x]` loads 128 bits and is an ordinary form.

---

## 16. Runtime semantics and checks

- **Initialization**: every variable and struct member not marked `= ---` is initialized (zero or default). Struct holes are zeroed.
- **Runtime checks** (each controlled by Build_Options and disabled by `set_optimization(.OPTIMIZED)`): array bounds (`__array_bounds_check_fail(index, limit, line, filename)`), cast bounds (`__cast_bounds_check_fail(pre_value, pre_flags, post_value, post_flags, fatal, line, filename)` with flag bits `SIGNED 0x40, 8BIT 0x100, 16BIT 0x200, 32BIT 0x400, 64BIT 0x800`), null pointer on auto-dereference (`__null_pointer_check_fail(arg_index, line, filename)`), arithmetic overflow (`__arithmetic_overflow(left, right, type_code, line, filename)` where `type_code` bit 0x8000 = fatal, 0x4000 = signed, bits 7–8 = operator (1 `+`, 2 `-`, 3 `*`, else `/`), low nibble = size in bytes), runtime calls of compile-time-only procedures (`__panic_due_to_runtime_call_of_compile_time_procedure`). Messages: "Array bounds check failed. (The attempted index is N, but the highest valid index is M). Site is file:line.", "Cast bounds check failed.  Number must be in [lo, hi]; it was V.  Site is file:line.", "Null pointer check failed: ...", "Arithmetic overflow. We tried to compute:\n    a + b\nThe operand type is s64, but the result does not fit into this type.", "Panic." followed by `debug_break`. The same checks are performed during compile-time execution (with the same messages). `#no_abc`/`#no_aoc` blocks, `cast,no_check`, `xx,no_check`, `<<,small` opt out locally.
- **Crashes**: with `backtrace_on_crash = .ON`, signal handlers (SIGSEGV, SIGTRAP, SIGFPE, SIGBUS, SIGILL) print the fault and a backtrace to stderr and `_exit(1)`. `debug_break()` executes `int3`. `exit(code)` in Basic exits without libc.
- **Integer semantics**: two's complement wraparound unless overflow checks are enabled; `/` truncates; shifts are defined for all amounts (unless `,small`); signed right shift is arithmetic unless `,logical`.
- **Strings/arrays**: bounds-checked, no implicit copying, no ownership; literal data is read-only.
- **Threads**: each thread has its own `context` and temporary storage; globals are shared; `Atomics`, `Thread` modules provide primitives; `compare_and_swap` is an intrinsic.
- **Output**: `write_string`/`write_strings`/`write_number` (Runtime_Support, `#no_context`, synchronized across threads and with compiler output at compile time) target stdout or stderr (`to_standard_error`). Errors, assertion failures, bounds failures and stack traces go to stderr.
- **Command line**: `__command_line_arguments: [] *u8` (Preload), `Basic.get_command_line_arguments() -> [] string` (argv[0] included). Compile-time arguments after `-` are in `Build_Options.compile_time_command_line`.
- **Type table**: `Runtime_Info { type_table: [] *Type_Info; global_data_info }` via `Compiler.get_runtime_info()` / `get_type_table()`; types appear only if `type_info`ed (transitively). `Type_Info` pointers are unique per type and stable between compile time and runtime (remapped).

---

## 17. Preload reference

Declarations injected into every program (module `Preload`); layouts are fixed ("@Volatile"):

- `Operating_System_Tag :: enum u32 { NONE; KRAMPOS; WINDOWS; LINUX; ANDROID; IOS; MACOS; NN_SWITCH; PS4; PS5; XBOX; WASM; }` (0..11); `CPU_Tag :: enum u32 { UNINITIALIZED; KRAMPU; CUSTOM; X64; ARM64; }`; globals `OS: Operating_System_Tag` (target OS, constant), `CPU: CPU_Tag`, `IS_CROSS_COMPILING: bool`, `MACHINE_OPTIONS_SIZE` (u8 array size for `Build_Options.machine_options`).
- `Type_Info_Tag :: enum u32 { INTEGER; FLOAT; BOOL; STRING; POINTER; PROCEDURE; VOID; STRUCT; ARRAY; OVERLOAD_SET; ANY; ENUM; POLYMORPHIC_VARIABLE; TYPE; CODE; UNTYPED_LITERAL; UNTYPED_ENUM; VARIANT :: 18; }` (17 unused).
- `Type_Info :: struct { type: Type_Info_Tag; runtime_size: s64; }` (−1 for unfinished/polymorphic); `Type_Info_Integer { using #as info: Type_Info; signed: bool; }`; `Type_Info_Float { info }`; `Type_Info_String { info }`; `Type_Info_Pointer { info; pointer_to: *Type_Info; }`; `Type_Info_Procedure { info; argument_types: [] *Type_Info; return_types: [] *Type_Info; procedure_flags: Flags; }` with `Flags :: enum_flags u32 { IS_ELSEWHERE :: 1; IS_COMPILE_TIME_ONLY :: 2; IS_POLYMORPHIC :: 4; HAS_NO_CONTEXT :: 8; IS_C_CALL :: 0x20; IS_INTRINSIC :: 0x80; IS_SYMMETRIC :: 0x100; IS_CPP_METHOD :: 0x1000_0000; HAS_CPP_NON_POD_RETURN_TYPE :: 0x2000_0000; }`; `Type_Info_Struct { info; name: string; specified_parameters: [] Type_Info_Struct_Member; members: [] Type_Info_Struct_Member; status_flags: Struct_Status_Flags; nontextual_flags; textual_flags; polymorph_source_struct: *Type_Info_Struct; initializer: (*void) #no_context; constant_storage: [] u8; notes: [] string; }`; `Type_Info_Struct_Member { name: string; type: *Type_Info; offset_in_bytes: s64; flags: Flags { CONSTANT 1; IMPORTED 2; USING 4; PROCEDURE_WITH_VOID_POINTER_TYPE_INFO 8; AS 0x10 }; notes: [] string; offset_into_constant_storage: s64 = -1; }`; `Type_Info_Array { info; element_type: *Type_Info; array_type: Array_Type { FIXED; VIEW; RESIZABLE } (u16); array_count: s64 (-1 unless FIXED); }`; `Type_Info_Enum { info; name; internal_type: *Type_Info_Integer; names: [] string; values: [] s64; status_flags: Enum_Status_Flags { INCOMPLETE 1 }; enum_type_flags: Enum_Type_Flags { FLAGS 1; COMPLETE 2; SPECIFIED 4 }; }`; `Type_Info_Variant { info; name; variant_of: *Type_Info; variant_flags: { DISTINCT 1; ISA 2 }; }`; bool/void/Any/Type/Code/polymorphic-variable use plain `Type_Info`.
- `Any_Struct { type: *Type_Info; value_pointer: *void; }` (the type of `Any`), `Newstring { count: s64; data: *u8; }`, `Array_View_64 { count: s64; data: *u8; }`, `Resizable_Array { count: s64; data: *void; allocated: s64; allocator: Allocator; }`, `Procedure_With_Data { proc: *void; data: *void; }`, `Name_Mapper :: #type ([] string);`, `__reg :: #type,distinct u16;`, `Workspace :: s64;`.
- `Allocator_Proc`, `Allocator`, `Allocator_Mode`, `Allocator_Caps` (10.4); `Logger`, `Log_Info`, `Log_Level`, `Log_Flags`, `Log_Section` (10.5); `Stack_Trace_Procedure_Info`, `Stack_Trace_Node` (10.6); `Source_Code_Location { fully_pathed_filename: string; line_number: s64; character_number: s64; }`; `Source_Code_Range { fully_pathed_filename; line_number_start; line_number_end; character_number_start; character_number_end }`; `For_Flags :: enum_flags u8 { POINTER :: 1; REVERSE :: 2; TEMPORARY_V2 :: 4; }`; `Temporary_Storage` (10.4).
- Globals: `__command_line_arguments: [] *u8;`, `__runtime_support_disable_stack_trace := false;`.
- Procedures: `get_current_workspace :: () -> Workspace #compiler` (0 at runtime); intrinsics `memcpy(dest, source, count: s64)`, `memcmp(a, b, count) -> s16 #must`, `memset(dest, value: u8, count)`, `compare_and_swap(pointer: *$T, old: T, new: T) -> (success: bool, old_value: T)` (≤ 8 bytes; also floats); `debug_break()`; `write_number`, `write_nonnegative_number` (exposed via Runtime_Support); `one_time_init`, `preload_compare_and_swap`; `FIRST_ADD_CONTEXT :: #code #add_context #as using base: Context_Base;` (how the Context is seeded).
- Runtime_Support additionally defines `Context_Base`, `Temporary_Storage`, `write_string(s, to_standard_error := false) #compiler #no_context`, `write_strings(..)`, `write_string_unsynchronized`, `runtime_support_default_logger`, `runtime_support_assertion_failed`, `runtime_support_default_allocator_proc`, `c_style_strlen`, `to_string(*u8) #no_context`, `__element_duplicate`, the check-failure procedures (16), `__jai_runtime_init`, `__jai_runtime_fini`, `__system_entry_point`, `__instrumentation_first/second`, `__program_main :: () #entry_point;`, `compile_time_debug_break :: () #compiler #no_context;`, `debug_break` via `#asm`.

---

## 18. Grammar summary

```
file            := { toplevel }
toplevel        := declaration | directive_stmt | note
declaration     := ident_list ':' [type] [ ('=' | ':') expr ] ';'          // with per-name modifiers in compound forms
                 | 'using' [modifiers] declaration
                 | '#no_reset' declaration | '#align' expr declaration | '#program_export' [string] declaration | '#as' declaration
directive_stmt  := '#import' [',file'|',dir'|',string'|',unshared'] string [ '(' args ')' [ '(' args ')' ] ] ';'
                 | '#load' string ';' | '#run' (expr | block) [';'] | '#assert' [',stallable'] expr [string] ';'
                 | '#if' expr (block | stmt | switch_body) [ 'else' ('#if' ... | block | stmt) ]
                 | '#insert' [',scope' '(' [expr] ')'] ['(' remaps ')'] expr ';' | '#placeholder' ident ';' | '#poke_name' ident ident ';'
                 | '#add_context' declaration | '#scope_export' [';'] | '#scope_module' [';'] | '#scope_file' [';'] | '#module_parameters' ...
                 | '#program_export' ... | '#library' ... | '#system_library' ...
type            := ident | '*' type | '[' [expr | '..' | '$' ident] ']' type | proc_type | 'struct' ... | 'union' ... | 'enum' ... | 'enum_flags' ...
                 | '#type' [',distinct'|',isa'] type | '$' ident [ '/' ['interface'] expr ] | '(' type ')' | 'type_of' '(' expr ')' | postfix_expr
proc_type       := ['inline'|'no_inline'] '(' params ')' [ '->' returns ] { header_directive }
proc_literal    := proc_type [ '#modify' block ] block   |  lambda
lambda          := ( ident | '(' ident_list ')' ) '=>' ( expr | block )
params          := [ param { ',' param } [','] ]
param           := ['using' [modifiers]] ['#discard'] ['$'|'$$'] ident [':' ['..'] type] ['=' expr] | ['..'] type
returns         := ret { ',' ret } | '(' ret { ',' ret } ')'
ret             := [ident ':'] type ['=' expr] ['#must'] | ident ':=' expr ['#must']
block           := ['#no_abc'|'#no_aoc'] '{' { stmt } '}'
stmt            := declaration | expr ';' | lvalues assign_op exprs ';' | block | 'if' ... | 'while' ... | 'for' ... | 'defer' stmt
                 | 'return' [args] ';' | 'break' [ident] ';' | 'continue' [ident] ';' | 'remove' [ident] ';'
                 | 'using' ... ';' | 'push_context' [',defer_pop'] [expr] (block | ';') | '#run' ... | '#assert' ... | '#if' ... | '#insert' ... | '#asm' ... | '#bytes' expr ';' | ';'
if_stmt         := 'if' ['#complete'] expr ( '==' '{' { case } '}' | ['then'] stmt ['else' stmt] )
case            := 'case' [expr] ';' { stmt }
for_stmt        := 'for' [':' ident] [ '#v2' ] [ '<' ['=' expr] ] [ '*' ['=' expr] ] [ ['`'] ident [',' ['`'] ident] ':' ] expr [ '..' expr ] stmt
while_stmt      := 'while' ( declaration_head | expr ) stmt
expr            := ifx_expr
ifx_expr        := 'ifx' expr [ ['then'] (expr | block) ] [ 'else' (expr | block) ] | '#ifx' expr ['then'] expr 'else' expr | binary_expr
binary_expr     := unary_expr { binop [',' modifier] unary_expr }        // precedence climbing per 5.1
unary_expr      := ('-'|'!'|'~'|'*'|'<<'|'(.*)'|'..'|'inline'|'no_inline'|'xx' [',no_check']|'cast' [',' mods] '(' type ')' ['.*']|'#run'|...) unary_expr | postfix_expr
postfix_expr    := primary { '.' ident | '.' '*' | '.' '(' type [',' mods] ')' | '[' expr ']' | '(' args ')' [',,' ctx_args] | '.{' fields '}' | '.[' elems ']' }
primary         := literal | ident | '`' ident | '.' ident | '.{' fields '}' | '.[' elems ']' | '(' expr ')' | 'context' | 'null' | proc_literal | type_expr | directive_expr | '#char' string | here_string
args            := [ arg { ',' arg } [','] ]      arg := [ident '='] ['..'] expr
```

## 19. Open questions and known deviations

1. The exact scoring function of overload resolution (numeric distances) is not documented; orangejuice implements the ranking of 7.5 and matches all module usages.
2. `for` range loops: whether assigning to `it` affects iteration for array loops (it does not in the reference; `it_index` does). orangejuice: assignment to `it` in array loops modifies only the copy.
3. Statement-level bare expressions (`result;`) produce no diagnostic in the reference; orangejuice warns.
4. Whether `%%` inside format strings will change meaning (announced); orangejuice implements 0.2.009 behavior with the warning.
5. `#align 9` (non-power-of-two) is generated by the bindings generator in one place; orangejuice rounds up to the next power of two with a warning.
6. `[] u8` ↔ `string` casts: view semantics (3.4).
7. Reverse range loops without `#v2` are treated as errors (the reference warns).
8. `#dynamic_specialize` is unimplemented in orangejuice (error).
9. `#cpp_method`/`#cpp_return_type_is_non_pod` are accepted and follow the Itanium C++ ABI on Linux.
10. `_` shadowing, `?` operator, `.?` operator: `_` always exists; `?` and `.?` are errors.
11. A compound assignment through `operator []=`: the reference at 0.2.009 passes the right-hand side alone, so `w[0] += 10` on a `10` leaves a `10` and `w[2] /= 2` leaves a `2` — measured, and at odds with both this section 6.7 and `how_to/094`'s own prose. orangejuice does what they say and assigns `w[0] = w[0] + 10`.
12. The exact text the reference prints when no candidate matches is not recorded here, only its shape (7.5: "did not match any", printing the argument types). orangejuice writes `The arguments given to 'name' did not match any of its overloads. The arguments were: (T, U).` and reports it at the call site; a measurement against an installed reference would replace it.
