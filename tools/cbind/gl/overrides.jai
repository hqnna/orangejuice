//
// Khronos spells its fixed-width types through `<stdint.h>`, and C's
// `signed char` reaches the generator as a plain byte; both are named
// directly here, so GL exports no `int64_t` of its own beside POSIX's and its
// signed bytes are signed.
//

khronos_int8_t :: s8;
khronos_int32_t :: s32;
khronos_int64_t :: s64;
khronos_uint64_t :: u64;
GLbyte :: s8;
