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
// Ported from: src/lib/commands/enable.c, src/lib/commands/disable.c

// "enable" / "disable" commands — control whether the update loop fires.
// Deity only.

use std::sync::atomic::Ordering;
use super::ctx::CmdCtx;

/// `enable` — allow the update engine to run ticks.
pub async fn run_enable(ctx: &CmdCtx<'_>) -> String {
    if !ctx.is_deity {
        return "10 Permission denied: deity only\n".to_string();
    }
    ctx.state.updates_enabled.store(true, Ordering::Relaxed);
    "1 Updates are enabled\n0 enable\n".to_string()
}

/// `disable` — prevent the update engine from running ticks.
pub async fn run_disable(ctx: &CmdCtx<'_>) -> String {
    if !ctx.is_deity {
        return "10 Permission denied: deity only\n".to_string();
    }
    ctx.state.updates_enabled.store(false, Ordering::Relaxed);
    "1 Updates are disabled\n0 disable\n".to_string()
}
