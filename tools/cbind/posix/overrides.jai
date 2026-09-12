//
// Declarations the module ships in place of what the generator produces.
//
// The generator emits what the headers say; where that is not what the module
// wants to expose, the replacement lives here and `generate.sh` drops the
// generated declaration of the same name in favour of it.
//

// The headers make this a plain `typedef int`, which would leave
// `clock_gettime(.REALTIME, *now)` with nothing to name. The values are the
// ones `<bits/time.h>` gives the `CLOCK_*` macros.
clockid_t :: enum u32 {
    REALTIME           :: 0;
    MONOTONIC          :: 1;
    PROCESS_CPUTIME_ID :: 2;
    THREAD_CPUTIME_ID  :: 3;
    MONOTONIC_RAW      :: 4;
    REALTIME_COARSE    :: 5;
    MONOTONIC_COARSE   :: 6;
    BOOTTIME           :: 7;
    REALTIME_ALARM     :: 8;
    BOOTTIME_ALARM     :: 9;
    TAI                :: 11;

    CLOCK_REALTIME           :: REALTIME;
    CLOCK_MONOTONIC          :: MONOTONIC;
    CLOCK_PROCESS_CPUTIME_ID :: PROCESS_CPUTIME_ID;
    CLOCK_THREAD_CPUTIME_ID  :: THREAD_CPUTIME_ID;
    CLOCK_MONOTONIC_RAW      :: MONOTONIC_RAW;
    CLOCK_REALTIME_COARSE    :: REALTIME_COARSE;
    CLOCK_MONOTONIC_COARSE   :: MONOTONIC_COARSE;
    CLOCK_BOOTTIME           :: BOOTTIME;
    CLOCK_REALTIME_ALARM     :: REALTIME_ALARM;
    CLOCK_BOOTTIME_ALARM     :: BOOTTIME_ALARM;
    CLOCK_TAI                :: TAI;
}
