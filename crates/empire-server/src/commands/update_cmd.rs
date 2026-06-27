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
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let _ = args;

    let mut out = String::new();

    let enabled = ctx.state.updates_enabled.load(Ordering::Relaxed);
    if !enabled {
        out.push_str("1 UPDATES ARE DISABLED!\n");
    }

    let next_ts = ctx.state.next_update_at.load(Ordering::Relaxed);
    if next_ts > 0 {
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
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
