/// Formats a timestamp into a human-readable relative time string.
///
/// # Arguments
///
/// * `seconds` - Seconds since UNIX epoch (UTC)
/// * `offset` - Timezone offset in seconds
///
/// # Returns
///
/// A String representing the relative time (e.g., "4 years ago")
pub fn format_relative_time(seconds: i64, offset: i32, now: std::time::SystemTime) -> String {
    let now = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let adjusted_seconds = seconds + offset as i64;
    let adjusted_now = now + offset as i64;

    let diff = if adjusted_now > adjusted_seconds {
        adjusted_now - adjusted_seconds
    } else {
        // If the time is somehow in the future, treat as "0 seconds ago".
        0
    };

    const SECOND: i64 = 1;
    const MINUTE: i64 = 60 * SECOND;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    const MONTH: i64 = 30 * DAY;
    const YEAR: i64 = 365 * DAY;

    let (value, unit) = if diff >= YEAR {
        let years = diff / YEAR;
        (years, if years == 1 { "year" } else { "years" })
    } else if diff >= MONTH {
        let months = diff / MONTH;
        (months, if months == 1 { "month" } else { "months" })
    } else if diff >= DAY {
        let days = diff / DAY;
        (days, if days == 1 { "day" } else { "days" })
    } else if diff >= HOUR {
        let hours = diff / HOUR;
        (hours, if hours == 1 { "hour" } else { "hours" })
    } else if diff >= MINUTE {
        let minutes = diff / MINUTE;
        (minutes, if minutes == 1 { "minute" } else { "minutes" })
    } else {
        let seconds = diff / SECOND;
        (seconds, if seconds == 1 { "second" } else { "seconds" })
    };

    format!("{} {} ago", value, unit)
}
