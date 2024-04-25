use time::OffsetDateTime;

pub fn ustime() -> i128 {
    let now = OffsetDateTime::now_utc();
    now.unix_timestamp_nanos()
}

/// Current timestamp in milliseconds
pub fn mstime() -> i128 {
    ustime() / 1000
}
