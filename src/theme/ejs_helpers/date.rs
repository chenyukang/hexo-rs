//! Date formatting helpers

use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, TimeZone};

/// Parse a date string and format it according to the given format
pub fn format_date(date_str: &str, format: &str) -> String {
    parse_and_format_date(date_str, format)
}

/// Format date for XML (ISO 8601)
pub fn format_date_xml(date_str: &str) -> String {
    parse_and_format_date_xml(date_str)
}

/// Generate a time tag
pub fn time_tag(date_str: &str, format: &str) -> String {
    let datetime = parse_and_format_date_xml(date_str);
    let display = parse_and_format_date(date_str, format);
    format!(r#"<time datetime="{}">{}</time>"#, datetime, display)
}

/// Extract year from date
pub fn year(date_str: &str) -> i32 {
    if let Ok(dt) = parse_datetime(date_str) {
        return dt.year();
    }
    0
}

/// Extract month from date
pub fn month(date_str: &str) -> u32 {
    if let Ok(dt) = parse_datetime(date_str) {
        return dt.month();
    }
    0
}

/// Extract day from date
pub fn day(date_str: &str) -> u32 {
    if let Ok(dt) = parse_datetime(date_str) {
        return dt.day();
    }
    0
}

// --- Internal helper functions ---

fn parse_datetime(date_str: &str) -> Result<DateTime<FixedOffset>, ()> {
    let date_str = date_str.trim();

    // Try parsing as ISO 8601 with timezone
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return Ok(dt);
    }

    // Try "YYYY-MM-DD HH:MM:SS" format
    if let Ok(ndt) = NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S") {
        let local = Local
            .from_local_datetime(&ndt)
            .single()
            .unwrap_or_else(Local::now);
        return Ok(local.fixed_offset());
    }

    // Try "YYYY-MM-DD" format
    if let Ok(nd) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        let ndt = nd.and_hms_opt(0, 0, 0).unwrap();
        let local = Local
            .from_local_datetime(&ndt)
            .single()
            .unwrap_or_else(Local::now);
        return Ok(local.fixed_offset());
    }

    Err(())
}

fn parse_and_format_date(date_str: &str, format: &str) -> String {
    if let Ok(dt) = parse_datetime(date_str) {
        let chrono_format = moment_to_chrono_format(format);
        return dt.format(&chrono_format).to_string();
    }
    date_str.to_string()
}

fn parse_and_format_date_xml(date_str: &str) -> String {
    if let Ok(dt) = parse_datetime(date_str) {
        return dt.to_rfc3339();
    }
    date_str.to_string()
}

/// Convert moment.js format to chrono format
fn moment_to_chrono_format(format: &str) -> String {
    // Order matters! Longer patterns must come before shorter ones
    // to avoid partial replacements (e.g., MMMM before MMM before MM before M)
    let replacements = [
        ("YYYY", "%Y"),
        ("YY", "%y"),
        ("MMMM", "%B"),
        ("MMM", "%b"),
        ("MM", "%m"),
        ("DDDD", "%j"),
        ("DD", "%d"),
        ("HH", "%H"),
        ("hh", "%I"),
        ("mm", "%M"),
        ("ss", "%S"),
        ("dddd", "%A"),
        ("ddd", "%a"),
        ("ZZ", "%z"),
        ("SSS", "%3f"),
    ];

    let mut result = format.to_string();
    for (from, to) in replacements {
        result = result.replace(from, to);
    }
    result
}

use chrono::Datelike;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_date() {
        let result = format_date("2024-01-15", "YYYY-MM-DD");
        assert_eq!(result, "2024-01-15");
    }

    #[test]
    fn test_year() {
        assert_eq!(year("2024-01-15"), 2024);
        assert_eq!(year("2024-01-15 10:30:00"), 2024);
    }

    #[test]
    fn test_month() {
        assert_eq!(month("2024-03-15"), 3);
    }
}
