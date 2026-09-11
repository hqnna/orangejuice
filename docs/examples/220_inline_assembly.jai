//
// `#asm` (**L§15**). A register is a declaration like any other, and the
// compiler places it; what a program writes is which register a value has to
// be in — `=== a` pins one — not which one it gets.
//

#import "Basic";

// `bit_scan_forward`, by hand: the index of the lowest set bit.
lowest_set_bit :: (value: u64) -> u64 {
    result: u64;
    #asm {
        bsf.q result, value;
    }
    return result;
}

// Adding with the carry flag read back, which is what a wide-integer routine
// needs and what no expression in the language produces.
add_with_carry :: (a: u64, b: u64) -> (sum: u64, carried: bool) {
    sum := a;
    carry: u8;
    #asm {
        add.q sum, b;
        setc carry;
    }
    return sum, carry != 0;
}

// A syscall: the number in `rax` and the arguments in the registers Linux
// fixes. This is `write(2)`, reaching the kernel with no libc in between.
//
// `SYSCALL_SYSRET` is the feature flag that says this block issues one, which
// is what tells the compiler `rcx` and `r11` do not survive it.
write_directly :: (fd: s32, data: *u8, count: s64) -> s64 {
    result: s64;
    #asm SYSCALL_SYSRET {
        mov.q rcx: gpr === c,  0;
        mov.q r11: gpr === 11, 0;
        mov.q rax: gpr === a,  1;
        mov.d rdi: gpr === di, fd;
        mov.q rsi: gpr === si, data;
        mov.q rdx: gpr === d,  count;
        syscall rcx, r11, rax, rdi, rsi, rdx;
        mov.q result, rax;
    }
    return result;
}

main :: () {
    a := lowest_set_bit(8);
    b := lowest_set_bit(1);
    c := lowest_set_bit(1024);
    print("% % %\n", a, b, c);

    sum, carried := add_with_carry(1, 2);
    print("% %\n", sum, carried);
    wrapped, overflowed := add_with_carry(0xffff_ffff_ffff_ffff, 1);
    print("% %\n", wrapped, overflowed);

    message := "straight to the kernel\n";
    written := write_directly(1, message.data, message.count);
    print("wrote % bytes\n", written);
}
