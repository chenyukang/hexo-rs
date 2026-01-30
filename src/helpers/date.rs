//! Date helper functions

use chrono::{DateTime, Local, TimeZone};

/// Format a date using Moment.js-compatible format string
///
/// # Examples
/// ```ignore
/// format_date(&date, "YYYY-MM-DD") // -> "2024-01-15"
/// ```
pub fn format_date<Tz: TimeZone>(date: &DateTime<Tz>, format: &str) -> String
where
    Tz::Offset: std::fmt::Display,
{
    // Convert Moment.js format to chrono format
    let chrono_format = moment_to_chrono_format(format);
    date.format(&chrono_format).to_string()
}

/// Format a date in ISO 8601 / XML format
pub fn date_xml<Tz: TimeZone>(date: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    date.format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string()
}

/// Format just the time portion
pub fn time<Tz: TimeZone>(date: &DateTime<Tz>, format: &str) -> String
where
    Tz::Offset: std::fmt::Display,
{
    format_date(date, format)
}

/// Format date in full format (like "January 1, 2024")
pub fn full_date<Tz: TimeZone>(date: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    date.format("%B %d, %Y").to_string()
}

/// Get relative time (like "2 hours ago")
pub fn relative_date(date: &DateTime<Local>) -> String {
    let now = Local::now();
    let duration = now.signed_duration_since(*date);

    if duration.num_seconds() < 0 {
        return "in the future".to_string();
    }

    let seconds = duration.num_seconds();
    let minutes = duration.num_minutes();
    let hours = duration.num_hours();
    let days = duration.num_days();

    if seconds < 60 {
        "a few seconds ago".to_string()
    } else if minutes == 1 {
        "a minute ago".to_string()
    } else if minutes < 60 {
        format!("{} minutes ago", minutes)
    } else if hours == 1 {
        "an hour ago".to_string()
    } else if hours < 24 {
        format!("{} hours ago", hours)
    } else if days == 1 {
        "yesterday".to_string()
    } else if days < 30 {
        format!("{} days ago", days)
    } else if days < 365 {
        let months = days / 30;
        if months == 1 {
            "a month ago".to_string()
        } else {
            format!("{} months ago", months)
        }
    } else {
        let years = days / 365;
        if years == 1 {
            "a year ago".to_string()
        } else {
            format!("{} years ago", years)
        }
    }
}

/// Generate a <time> HTML element
pub fn time_tag<Tz: TimeZone>(date: &DateTime<Tz>, format: Option<&str>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let datetime = date.format("%Y-%m-%dT%H:%M:%S%:z").to_string();
    let display = format_date(date, format.unwrap_or("YYYY-MM-DD"));
    format!(r#"<time datetime="{}">{}</time>"#, datetime, display)
}

/// Convert Moment.js format to chrono format
fn moment_to_chrono_format(format: &str) -> String {
    // Process from longest to shortest patterns within each category
    // Use unique placeholders that won't conflict
    let replacements = [
        // Year (process first as they're uppercase)
        ("YYYY", "%Y"),
        ("YY", "%y"),
        // Month (uppercase M)
        ("MMMM", "%B"), // Full month name
        ("MMM", "%b"),  // Abbreviated month name
        ("MM", "%m"),   // Two-digit month
        // Day of month (uppercase D) - process before lowercase
        ("DDDD", "%j"), // Day of year
        ("DD", "%d"),   // Two-digit day
        // Hour 24h (uppercase H)
        ("HH", "%H"),
        // Hour 12h (lowercase h)
        ("hh", "%I"),
        // Minute (lowercase m after we've processed MM)
        ("mm", "%M"),
        // Second (lowercase s)
        ("ss", "%S"),
        // Day of week (lowercase d) - process last to avoid conflicts
        ("dddd", "%A"), // Full weekday name
        ("ddd", "%a"),  // Abbreviated weekday name
        // Timezone
        ("ZZ", "%z"),
        // Milliseconds
        ("SSS", "%3f"),
        // AM/PM - careful with A as it appears in many places
    ];

    let mut result = format.to_string();

    for (from, to) in replacements {
        result = result.replace(from, to);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_format_date() {
        let date = Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        assert_eq!(format_date(&date, "YYYY-MM-DD"), "2024-01-15");
        assert_eq!(format_date(&date, "YYYY/MM/DD"), "2024/01/15");
    }

    #[test]
    fn test_full_date() {
        let date = Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        assert_eq!(full_date(&date), "January 15, 2024");
    }

    #[test]
    fn test_moment_to_chrono() {
        assert_eq!(moment_to_chrono_format("YYYY-MM-DD"), "%Y-%m-%d");
        assert_eq!(moment_to_chrono_format("HH:mm:ss"), "%H:%M:%S");
    }
}
