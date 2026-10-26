//
// Debug: a program asking for a debugger.
//
//     Debug :: #import "Debug";
//     Debug.init();                // name the crash and walk the stack
//     Debug.attach_to_debugger();  // true when one is watching
//     Debug.breakpoint();          // stop here, if one is
//
// `breakpoint` stops only when a debugger is attached, so a program that
// leaves its breakpoints in still runs to the end on its own. `debug_break`
// from Basic is the unconditional one: without a debugger the trap it raises
// ends the program.
//
// Whether a debugger is attached is read from `/proc/self/status` on Linux,
// where a traced process names its tracer. Elsewhere nothing says so cheaply,
// and the answer is no.
//

#import "Basic";
#import "String";
#import "File";
Crash_Handler :: #import "Runtime_Support_Crash_Handler";

// Installs the crash handler: a fault names itself and prints the call stack
// rather than dying silently. The entry point usually has already, and doing
// it twice is harmless.
init :: () {
    Crash_Handler.init();
}

// Whether a debugger is watching this process. Nothing is launched: a
// program run on its own stays on its own.
attach_to_debugger :: () -> bool {
    return debugger_is_attached();
}

debugger_is_attached :: () -> bool {
    #if OS == .LINUX {
        status, ok := read_entire_file("/proc/self/status", log_errors = false);
        if !ok  return false;
        defer free(status);
        found, _, after := split_from_left(status, "TracerPid:");
        if !found  return false;
        after = trim_left(after, " \t");
        return after.count > 0 && after[0] != #char "0";
    } else {
        return false;
    }
}

// Stops in the debugger, when there is one.
breakpoint :: () {
    if debugger_is_attached()  debug_break();
}
