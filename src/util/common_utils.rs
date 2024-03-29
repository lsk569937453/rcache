use time::get_time;

pub fn ustime() -> i64 {
    let tv = get_time();
    tv.sec * 1000000 + (tv.nsec / 1000) as i64
}

/// Current timestamp in milliseconds
pub fn mstime() -> i64 {
    ustime() / 1000
}
