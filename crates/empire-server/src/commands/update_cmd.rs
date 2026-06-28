// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/upda.c

// "update" command — show update schedule information.
//
// Usage: update

use std::sync::atomic::Ordering;
use chrono::{DateTime, Local, TimeZone};
use super::ctx::CmdCtx;

/// Format a Unix timestamp as the classic Empire time string:
///   "Wed Jun 25 20:00:00"
/// ptkei regex: r"(?P<day>\S\S\S) (?P<month>\S\S\S) +(?P<date>\d+) ..."
/// Date is right-justified in 2 chars with a leading space for single digits.
fn fmt_empire_time(ts: u64) -> String {
    let dt: DateTime<Local> = Local.timestamp_opt(ts as i64, 0)
        .single()
        .unwrap_or_else(|| Local::now());
    // Classic Empire format: "Wed Jun  6 20:00:00" (single-digit date gets extra space)
    format!("{}", dt.format("%a %b %e %H:%M:%S"))
}

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let _ = args;

    let mut out = String::new();

    let enabled = ctx.state.updates_enabled.load(Ordering::Relaxed);
    if !enabled {
        out.push_str("1 UPDATES ARE DISABLED!\n");
    }

    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Emit "The current time is   <time>." — ptkei ParseUpdate expects 3 spaces after "is"
    out.push_str(&format!("1 The current time is   {}.\n", fmt_empire_time(now_ts)));

    let next_ts = ctx.state.next_update_at.load(Ordering::Relaxed);
    if next_ts > 0 {
        // Emit "The next update is at <time>." — ptkei ParseUpdate expects this exact string
        out.push_str(&format!("1 The next update is at {}.\n", fmt_empire_time(next_ts)));

        let secs_until = if next_ts > now_ts { next_ts - now_ts } else { 0 };
        let mins = secs_until / 60;
        let secs = secs_until % 60;
        if secs_until == 0 {
            out.push_str("1 An update is running or is imminent.\n");
        } else if mins > 0 {
            out.push_str(&format!("1 Next update in {}m {}s.\n", mins, secs));
        } else {
            out.push_str(&format!("1 Next update in {}s.\n", secs));
        }
    } else {
        out.push_str("1 Next update time is not yet known.\n");
    }

    let interval = ctx.config.update.update_interval_secs;
    let schedule_file = &ctx.config.server.schedule_file;
    if schedule_file.exists() {
        out.push_str(&format!("1 Updates follow schedule file: {}\n", schedule_file.display()));
    } else {
        out.push_str(&format!("1 Updates run every {} seconds ({:.1} minutes).\n",
            interval, interval as f64 / 60.0));
    }

    out.push_str(&format!("1 ETU per update: {}.\n", ctx.config.game.etu_per_update));

    if ctx.config.update.allow_force && ctx.is_deity {
        out.push_str("1 Force updates are enabled (deity: use 'force' to trigger now).\n");
    }

    out.push_str("0 update\n");
    out
}
