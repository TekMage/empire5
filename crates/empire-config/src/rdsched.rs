// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// Empire is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/common/rdsched.c
// Known contributors to the original:
//    Markus Armbruster, 2007-2011

//! Read the Empire update schedule file.
//!
//! Format (one directive per line; `#` starts a comment):
//! ```text
//! # absolute times (local timezone)
//! 2007-01-05 14:00
//! 22 Jan 2007 14:00
//!
//! # anchor-relative (day of week, optionally + time)
//! next Fri 14:00
//! next Fri
//!
//! # periodic
//! every 6 hours
//! every 30 minutes until 2007-01-05 20:00
//!
//! # remove a specific time
//! skip 2007-01-05 14:00
//! ```

use chrono::{DateTime, Datelike, Duration, Local, NaiveDateTime, NaiveTime, TimeZone, Timelike, Weekday};
use std::path::Path;

/// Read update schedule from `path`.
///
/// Returns a sorted list of at most `max` local update times that fall after
/// `after`.  `anchor` is the reference point for `next Weekday` directives;
/// it is updated as each absolute time is parsed (matching the C behaviour).
///
/// Returns `Err` only on I/O failure.  Parse errors on individual lines are
/// logged as warnings and skipped.
pub fn read_schedule(
    path: &Path,
    after: DateTime<Local>,
    anchor: DateTime<Local>,
    max: usize,
) -> std::io::Result<Vec<DateTime<Local>>> {
    let text = std::fs::read_to_string(path)?;
    let mut sched: Vec<DateTime<Local>> = Vec::new();
    let mut anchor = anchor;

    for (lno, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Err(e) = parse_line(line, &mut sched, &mut anchor, after, max) {
            tracing::warn!("schedule:{}: {}", lno + 1, e);
        }
    }

    sched.truncate(max);
    Ok(sched)
}

// ── Line dispatcher ───────────────────────────────────────────────────────────

fn parse_line(
    line: &str,
    sched: &mut Vec<DateTime<Local>>,
    anchor: &mut DateTime<Local>,
    after: DateTime<Local>,
    max: usize,
) -> Result<(), String> {
    let lo = line.to_ascii_lowercase();

    if let Some(rest) = lo.strip_prefix("skip") {
        let t = parse_time_str(rest.trim(), anchor)?;
        delete_update(t, sched);
        return Ok(());
    }

    if let Some(rest) = lo.strip_prefix("every") {
        return parse_every(rest.trim(), line, sched, anchor, after, max);
    }

    // Absolute or anchor-relative time
    let t = parse_time_str(line, anchor)?;
    *anchor = t;
    insert_update(t, sched, max, after);
    Ok(())
}

// ── "every N hours/minutes [until TIME]" ─────────────────────────────────────

fn parse_every(
    lo_rest: &str,
    original_line: &str,
    sched: &mut Vec<DateTime<Local>>,
    anchor: &mut DateTime<Local>,
    after: DateTime<Local>,
    max: usize,
) -> Result<(), String> {
    let (delta_secs, tail) = parse_interval(lo_rest)?;
    let delta = Duration::seconds(delta_secs);

    let until = if tail.to_ascii_lowercase().trim_start().starts_with("until") {
        // parse_time_str needs original casing; find "until" keyword in original line.
        let original_after_until = find_after_keyword_ci(original_line, "until")?;
        Some(parse_time_str(original_after_until.trim(), anchor)?)
    } else if tail.trim().is_empty() {
        None
    } else {
        return Err(format!("trailing junk after 'every' clause: '{}'", tail.trim()));
    };

    let mut t = *anchor;
    loop {
        t = t + delta;
        if let Some(u) = until {
            if t > u {
                break;
            }
        }
        let idx = insert_update(t, sched, max, after);
        // Stop when the schedule is full and the new time is beyond the end.
        // Mirrors C: break when insert_update returns n-1.
        if idx >= max {
            break;
        }
    }
    Ok(())
}

/// Return the substring of `line` that follows the first (case-insensitive)
/// occurrence of `keyword`.
fn find_after_keyword_ci<'a>(line: &'a str, keyword: &str) -> Result<&'a str, String> {
    let lo = line.to_ascii_lowercase();
    let pos = lo.find(keyword)
        .ok_or_else(|| format!("'{keyword}' not found in '{line}'"))?;
    Ok(&line[pos + keyword.len()..])
}

/// Parse "N hours..." or "N minutes..." from the start of `s`.
/// Returns (seconds, rest_of_string).
fn parse_interval(s: &str) -> Result<(i64, &str), String> {
    let s = s.trim_start();

    // Extract leading digits
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 {
        return Err("expected number after 'every'".to_string());
    }
    let n: i64 = s[..end].parse().map_err(|e| format!("bad number: {e}"))?;
    let rest = s[end..].trim_start();

    if let Some(tail) = rest.strip_prefix("hours") {
        Ok((n * 3600, tail))
    } else if let Some(tail) = rest.strip_prefix("minutes") {
        Ok((n * 60, tail))
    } else {
        Err(format!("expected 'hours' or 'minutes', found: '{rest}'"))
    }
}

// ── Time string parsing ───────────────────────────────────────────────────────

/// Parse a time string in any supported format, returning a local `DateTime`.
///
/// Supported formats:
/// - `YYYY-MM-DD HH:MM` — ISO 8601
/// - `DD Mon YYYY HH:MM` — RFC 2822-style
/// - `Mon DD HH:MM YYYY` — ctime-style (e.g. `Jan  5 14:00 2007`)
/// - `next Weekday [HH:MM]` — anchor-relative
pub fn parse_time_str(s: &str, anchor: &DateTime<Local>) -> Result<DateTime<Local>, String> {
    let s = s.trim();

    // "next Weekday [HH:MM]"
    if s.to_ascii_lowercase().starts_with("next") {
        let rest = s[4..].trim();
        return parse_next(rest, anchor);
    }

    // ISO 8601: "YYYY-MM-DD HH:MM"
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        return local_from_naive(ndt);
    }

    // RFC 2822-like: "22 Jan 2007 15:35"
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%d %b %Y %H:%M") {
        return local_from_naive(ndt);
    }

    // ctime-like: "Jan 22 15:35 2007" or "Jan  5 15:35 2007" (zero-padded day)
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%b %d %H:%M %Y") {
        return local_from_naive(ndt);
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%b %e %H:%M %Y") {
        return local_from_naive(ndt);
    }

    Err(format!("unrecognised time: '{s}'"))
}

fn local_from_naive(ndt: NaiveDateTime) -> Result<DateTime<Local>, String> {
    Local.from_local_datetime(&ndt)
        .single()
        .ok_or_else(|| format!("ambiguous or invalid local time: {ndt}"))
}

/// Parse `"Weekday [HH:MM]"` relative to `anchor`.
/// "next Fri 14:00" — next Friday at 14:00
/// "next Fri"       — next Friday at anchor's current time
fn parse_next(s: &str, anchor: &DateTime<Local>) -> Result<DateTime<Local>, String> {
    // Split off weekday token
    let (wd_str, rest) = s.split_once(|c: char| c.is_whitespace())
        .map(|(a, b)| (a, b.trim()))
        .unwrap_or((s, ""));

    let target = parse_weekday(wd_str)?;

    let (h, m) = if rest.is_empty() {
        (anchor.hour(), anchor.minute())
    } else {
        let nt = NaiveTime::parse_from_str(rest, "%H:%M")
            .map_err(|e| format!("bad time '{rest}': {e}"))?;
        (nt.hour(), nt.minute())
    };

    // Days to advance: always at least 1 (C's "next" means strictly after anchor day)
    let anchor_wd = anchor.weekday().num_days_from_monday();
    let target_wd = target.num_days_from_monday();
    let days = {
        let d = (target_wd + 7 - anchor_wd) % 7;
        if d == 0 { 7i64 } else { d as i64 }
    };

    let date = anchor.date_naive() + Duration::days(days);
    let ndt = date
        .and_hms_opt(h, m, 0)
        .ok_or_else(|| format!("invalid time {h:02}:{m:02}"))?;
    local_from_naive(ndt)
}

fn parse_weekday(s: &str) -> Result<Weekday, String> {
    match s.to_ascii_lowercase().as_str() {
        "mon" | "monday"    => Ok(Weekday::Mon),
        "tue" | "tuesday"   => Ok(Weekday::Tue),
        "wed" | "wednesday" => Ok(Weekday::Wed),
        "thu" | "thursday"  => Ok(Weekday::Thu),
        "fri" | "friday"    => Ok(Weekday::Fri),
        "sat" | "saturday"  => Ok(Weekday::Sat),
        "sun" | "sunday"    => Ok(Weekday::Sun),
        other               => Err(format!("unknown weekday: '{other}'")),
    }
}

// ── Schedule manipulation ─────────────────────────────────────────────────────

/// Insert `t` into `sched` (sorted ascending) if `t > after` and `sched.len() < max`.
/// When the schedule is already at `max`, insert only if `t` is earlier than the
/// current last entry (displacing it), matching C's memmove + truncate semantics.
///
/// Returns the insertion index, or `usize::MAX` when `t` is in the past,
/// or `max` when the schedule is full and `t` is beyond all existing entries.
fn insert_update(
    t: DateTime<Local>,
    sched: &mut Vec<DateTime<Local>>,
    max: usize,
    after: DateTime<Local>,
) -> usize {
    if t <= after {
        return usize::MAX;
    }
    let pos = sched.partition_point(|&x| x < t);
    // Duplicate — already scheduled at this time
    if pos < sched.len() && sched[pos] == t {
        return pos;
    }
    if pos >= max {
        return max; // beyond capacity
    }
    sched.insert(pos, t);
    if sched.len() > max {
        sched.pop();
    }
    pos
}

/// Remove the entry for `t` from `sched` if present.
fn delete_update(t: DateTime<Local>, sched: &mut Vec<DateTime<Local>>) {
    if let Ok(pos) = sched.binary_search(&t) {
        sched.remove(pos);
    }
}

fn strip_comment(s: &str) -> &str {
    if let Some(pos) = s.find('#') {
        &s[..pos]
    } else {
        s
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn dt(y: i32, mo: u32, d: u32, h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, mo, d, h, m, 0).unwrap()
    }

    #[test]
    fn parse_iso() {
        let anchor = dt(2007, 1, 1, 0, 0);
        let t = parse_time_str("2007-01-05 14:00", &anchor).unwrap();
        assert_eq!(t, dt(2007, 1, 5, 14, 0));
    }

    #[test]
    fn parse_rfc() {
        let anchor = dt(2007, 1, 1, 0, 0);
        let t = parse_time_str("22 Jan 2007 15:35", &anchor).unwrap();
        assert_eq!(t, dt(2007, 1, 22, 15, 35));
    }

    #[test]
    fn parse_ctime() {
        let anchor = dt(2007, 1, 1, 0, 0);
        let t = parse_time_str("Jan 22 15:35 2007", &anchor).unwrap();
        assert_eq!(t, dt(2007, 1, 22, 15, 35));
    }

    #[test]
    fn every_hours() {
        // "every 6 hours" starting from a Friday 12:00, look ahead 48h
        let anchor = dt(2007, 1, 5, 12, 0); // Fri
        let after  = dt(2007, 1, 5, 12, 0);
        let mut sched = Vec::new();
        parse_every("6 hours", "every 6 hours", &mut sched, &mut anchor.clone(), after, 8).unwrap();
        assert_eq!(sched[0], dt(2007, 1, 5, 18, 0));
        assert_eq!(sched[1], dt(2007, 1, 6,  0, 0));
        assert_eq!(sched.len(), 8);
    }

    #[test]
    fn every_with_until() {
        let anchor = dt(2007, 1, 5, 12, 0);
        let after  = dt(2007, 1, 5, 12, 0);
        let mut sched = Vec::new();
        let line = "every 6 hours until 2007-01-06 00:00";
        parse_every("6 hours until 2007-01-06 00:00", line, &mut sched, &mut anchor.clone(), after, 16).unwrap();
        // 18:00 and 00:00 (next day)
        assert_eq!(sched.len(), 2);
        assert_eq!(sched[0], dt(2007, 1, 5, 18, 0));
        assert_eq!(sched[1], dt(2007, 1, 6,  0, 0));
    }

    #[test]
    fn next_weekday() {
        // anchor is a Friday; "next Mon 09:00" should land on Monday
        let anchor = dt(2007, 1, 5, 0, 0); // Fri Jan 5 2007
        let t = parse_time_str("next Mon 09:00", &anchor).unwrap();
        assert_eq!(t, dt(2007, 1, 8, 9, 0)); // Mon Jan 8 2007
    }

    #[test]
    fn next_same_weekday_wraps() {
        // anchor IS a Friday; "next Fri" must skip to the next Friday (+7 days)
        let anchor = dt(2007, 1, 5, 0, 0); // Fri
        let t = parse_time_str("next Fri 14:00", &anchor).unwrap();
        assert_eq!(t, dt(2007, 1, 12, 14, 0)); // Fri Jan 12 2007
    }

    #[test]
    fn skip_removes_entry() {
        let anchor = dt(2007, 1, 5, 0, 0);
        let after  = dt(2007, 1, 4, 0, 0);
        let t1 = dt(2007, 1, 5, 10, 0);
        let t2 = dt(2007, 1, 5, 16, 0);
        let mut sched = vec![t1, t2];
        delete_update(t1, &mut sched);
        assert_eq!(sched, vec![t2]);
        let _ = (anchor, after); // suppress unused
    }
}
