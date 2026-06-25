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
// Ported from: src/lib/commands/shutdo.c

// "shutdown" command — schedule or abort server shutdown.
// Usage: shutdown <minutes>   (negative to abort)
// Deity only.

use super::ctx::CmdCtx;
use tracing::info;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    if !ctx.is_deity {
        return "10 Permission denied: deity only\n".to_string();
    }

    let minutes_str = args.trim();
    let minutes: i64 = match minutes_str.parse() {
        Ok(n) => n,
        Err(_) => return format!("10 Invalid argument '{}'; expected integer minutes\n", minutes_str),
    };

    if minutes < 0 {
        // Abort pending shutdown
        let mut handle_guard = ctx.state.shutdown_handle.lock().unwrap();
        if let Some(handle) = handle_guard.take() {
            handle.abort();
            info!("Shutdown aborted by deity {}", ctx.cnum);
            return "1 Shutdown aborted\n0 shutdown\n".to_string();
        } else {
            return "1 No shutdown pending\n0 shutdown\n".to_string();
        }
    }

    // Abort any existing pending shutdown first
    {
        let mut handle_guard = ctx.state.shutdown_handle.lock().unwrap();
        if let Some(old) = handle_guard.take() {
            old.abort();
        }
    }

    let delay_secs = minutes as u64 * 60;
    info!("Shutdown scheduled in {} minutes by deity {}", minutes, ctx.cnum);

    let task_handle = tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
        info!("Shutdown timer expired — exiting");
        std::process::exit(0);
    });

    {
        let mut handle_guard = ctx.state.shutdown_handle.lock().unwrap();
        *handle_guard = Some(task_handle.abort_handle());
    }

    if minutes == 0 {
        format!("1 Shutting down immediately\n0 shutdown\n")
    } else {
        format!("1 Shutdown scheduled in {} minute(s)\n0 shutdown\n", minutes)
    }
}
