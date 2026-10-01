use chrono::{DateTime, Local, Utc};

/// Formats a byte size with one decimal place for KB and MB (e.g. "2.0 KB", "1.5 MB").
pub fn human_bytes(bytes: i64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// Formats a byte size for UI display with rounded KB and one decimal MB (e.g. "2 KB", "1.5 MB").
pub fn human_size(bytes: i64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// Formats a Unix timestamp into a human-readable local date and time string.
pub fn format_full_time(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%a %b %-d %H:%M:%S %Y")
                .to_string()
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// Formats a Unix timestamp into a short relative string (e.g. "now", "5 min", "2 hr", "3 day").
pub fn relative_time(timestamp: i64) -> String {
    match relative_age(timestamp) {
        RelativeAge::Now => "now".to_string(),
        RelativeAge::Minutes(minutes) => format!("{minutes} min"),
        RelativeAge::Hours(hours) => format!("{hours} hr"),
        RelativeAge::Days(days) => format!("{days} day"),
    }
}

/// The age [`relative_time`] shows; equal ages render the same label.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RelativeAge {
    Now,
    Minutes(i64),
    Hours(i64),
    Days(i64),
}

pub fn relative_age(timestamp: i64) -> RelativeAge {
    let seconds = (Utc::now().timestamp() - timestamp).max(0);
    if seconds < 60 {
        RelativeAge::Now
    } else if seconds < 3_600 {
        RelativeAge::Minutes(seconds / 60)
    } else if seconds < 86_400 {
        RelativeAge::Hours(seconds / 3_600)
    } else {
        RelativeAge::Days(seconds / 86_400)
    }
}

/// Masks a secret string, revealing only the final 4 characters (e.g. "********tail").
pub fn masked_secret(value: &str) -> String {
    let visible_tail = value.chars().rev().take(4).collect::<Vec<_>>();
    let tail = visible_tail.into_iter().rev().collect::<String>();
    if tail.is_empty() {
        "********".to_string()
    } else {
        format!("********{tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::{RelativeAge, human_bytes, human_size, masked_secret, relative_age, relative_time};

    #[test]
    fn formats_human_bytes() {
        assert_eq!(human_bytes(12), "12 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(1_572_864), "1.5 MB");
    }

    #[test]
    fn formats_display_sizes() {
        assert_eq!(human_size(12), "12 B");
        assert_eq!(human_size(2048), "2 KB");
        assert_eq!(human_size(1_572_864), "1.5 MB");
    }

    #[test]
    fn masks_secrets() {
        assert_eq!(masked_secret(""), "********");
        assert_eq!(masked_secret("abc"), "********abc");
        assert_eq!(masked_secret("secret-token"), "********oken");
    }

    #[test]
    fn relative_age_matches_the_label() {
        let now = chrono::Utc::now().timestamp();
        assert_eq!(relative_age(now - 30), RelativeAge::Now);
        assert_eq!(relative_age(now - 150), RelativeAge::Minutes(2));
        assert_eq!(relative_time(now - 150), "2 min");
        assert_eq!(relative_age(now - 2 * 86_400), RelativeAge::Days(2));
    }
}
