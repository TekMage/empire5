// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/forc.c

// "force" command — deity-only, triggers an immediate update tick.
//
// Usage: force

use std::sync::atomic::Ordering;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let _ = args;

    if !ctx.is_deity {
        return "10 This command is restricted to deities.\n".to_string();
    }
    if !ctx.config.update.allow_force {
        return "1 Force updates are disabled (allow_force = false in empire.toml).\n0 force\n".to_string();
    }
    if !ctx.state.updates_enabled.load(Ordering::Relaxed) {
        return "1 Updates are currently disabled.\n0 force\n".to_string();
    }

    ctx.state.force_update.notify_one();
    "1 Update triggered.\n0 force\n".to_string()
}
