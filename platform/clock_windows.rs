pub fn local_day() -> i64 {
    #[repr(C)]
    struct SystemTime { year: u16, month: u16, weekday: u16, day: u16, hour: u16, minute: u16, second: u16, milliseconds: u16 }
    #[repr(C)]
    struct FileTime { low: u32, high: u32 }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
        fn SystemTimeToFileTime(time: *const SystemTime, file: *mut FileTime) -> i32;
    }
    // Encode the local calendar date, not the UTC timestamp, as an epoch day.
    unsafe {
        let mut time: SystemTime = std::mem::zeroed();
        GetLocalTime(&mut time);
        time.hour = 0; time.minute = 0; time.second = 0; time.milliseconds = 0;
        let mut file: FileTime = std::mem::zeroed();
        assert_ne!(SystemTimeToFileTime(&time, &mut file), 0);
        (((file.high as u64) << 32 | file.low as u64) / 864_000_000_000) as i64 - 134_774
    }
}
