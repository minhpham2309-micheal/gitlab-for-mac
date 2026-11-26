use std::time::{SystemTime, UNIX_EPOCH};

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
];

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400) as u64;
    let mp = if m > 2 { u64::from(m - 3) } else { u64::from(m + 9) };
    let doy = (153 * mp + 2) / 5 + u64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

pub fn parse_iso_utc_secs(iso: &str) -> Option<i64> {
    let s = iso.trim();
    if s.len() < 10 { return None; }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let m: u32 = s.get(5..7)?.parse().ok()?;
    let d: u32 = s.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) { return None; }
    let mut secs = days_from_civil(y, m, d) * 86_400;
    if s.len() >= 19 && matches!(s.as_bytes()[10], b'T' | b' ') {
        let hh: i64 = s.get(11..13)?.parse().ok()?;
        let mm: i64 = s.get(14..16)?.parse().ok()?;
        let ss: i64 = s.get(17..19)?.parse().ok()?;
        secs += hh * 3600 + mm * 60 + ss;
        // Adjust for explicit +/-HH:MM timezone offset when present.
        let rest = &s[19..];
        if let Some(sign_pos) = rest.find(['+', '-']) {
            let tz = &rest[sign_pos..];
            if tz.len() >= 6 && tz.as_bytes()[3] == b':'
                && let (Ok(th), Ok(tm)) = (tz[1..3].parse::<i64>(), tz[4..6].parse::<i64>()) {
                    let off = th * 3600 + tm * 60;
                    if tz.starts_with('+') { secs -= off; } else { secs += off; }
                }
        }
    }
    Some(secs)
}

pub fn format_relative_from(iso: &str, now_secs: i64) -> String {
    let Some(ts) = parse_iso_utc_secs(iso) else {
        return iso.get(..10).unwrap_or(iso).to_string();
    };
    let diff = (now_secs - ts).max(0);
    let unit = |n: i64, singular: &str| {
        if n == 1 { format!("1 {singular} ago") } else { format!("{n} {singular}s ago") }
    };
    if diff < 60 { "just now".into() }
    else if diff < 3_600 { unit(diff / 60, "minute") }
    else if diff < 86_400 { unit(diff / 3_600, "hour") }
    else if diff < 7 * 86_400 { unit(diff / 86_400, "day") }
    else if diff < 30 * 86_400 { unit(diff / (7 * 86_400), "week") }
    else if diff < 365 * 86_400 { unit(diff / (30 * 86_400), "month") }
    else { unit(diff / (365 * 86_400), "year") }
}

pub fn format_relative(iso: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_relative_from(iso, now)
}

pub fn format_long_date(iso: &str) -> String {
    let s = iso.trim();
    let fallback = || s.chars().take(10).collect::<String>();
    let (Some(ys), Some(ms), Some(ds)) = (s.get(0..4), s.get(5..7), s.get(8..10)) else {
        return fallback();
    };
    let (Ok(y), Ok(m), Ok(d)) = (ys.parse::<u32>(), ms.parse::<usize>(), ds.parse::<u32>()) else {
        return fallback();
    };
    match MONTHS.get(m.wrapping_sub(1)) {
        Some(month) => format!("{month} {d}, {y}"),
        None => fallback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_relative_and_long_dates_accurately() {
        let base = parse_iso_utc_secs("2026-04-10T12:00:00Z").unwrap();
        assert_eq!(format_relative_from("2026-04-10T11:00:00Z", base), "1 hour ago");
        assert_eq!(format_relative_from("2026-04-05T12:00:00Z", base), "5 days ago");
        assert_eq!(format_relative_from("2026-04-02T12:00:00Z", base), "1 week ago");
        assert_eq!(format_long_date("2026-04-10T08:15:00.000Z"), "April 10, 2026");
        // Must never panic on non-ASCII strings crossing byte offsets 4/7/10.
        assert_eq!(format_long_date("ngày-tháng-năm"), "ngày-tháng");
    }
}
