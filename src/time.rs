pub fn timestamp() -> u32 {
    use std::time::SystemTime;
    let now = SystemTime::now();
    let now = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let now = now.as_secs();
    now as u32
}
pub fn format_timestamp(time: u32) -> String {
	use chrono::prelude::*;
	let dt = NaiveDateTime::from_timestamp_opt(time as i64, 0).unwrap();
	// 东八区
	let dt: DateTime<FixedOffset> = DateTime::from_utc(dt, FixedOffset::east_opt(8 * 3600).unwrap());
	dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[test]
fn test_timestamp() {
    assert!(timestamp() > 16912_84709 && timestamp() < 20000_00000);
    // 时间戳在 2023-08-06 09:18:29 ~ 2033-05-18 11:33:20 之间
    // 单位：秒
}

#[test]
fn test_format_timestamp() {
	assert_eq!(
		format_timestamp(16912_84709),
		"2023-08-06 09:18:29".to_string()
	);
	assert_eq!(
		format_timestamp(20000_00000),
		"2033-05-18 11:33:20".to_string()
	);
}