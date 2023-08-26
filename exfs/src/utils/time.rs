use fuser::TimeOrNow;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn time_sec(t: TimeOrNow) -> u64 {
    match t {
        TimeOrNow::SpecificTime(v) => time_sys(v),
        TimeOrNow::Now => time_sys(SystemTime::now()),
    }
}

pub fn time_sys(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).unwrap().as_secs()
}

pub fn system_time_from_time(secs: i64, nsecs: u32) -> SystemTime {
    if secs >= 0 {
        SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nsecs)
    } else {
        SystemTime::UNIX_EPOCH - Duration::new((-secs) as u64, nsecs)
    }
}
