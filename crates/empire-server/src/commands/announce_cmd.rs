// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/tele.c (announce branch)

// "announce" command — broadcast a message to all active nations.
//
// Usage: announce <message>
//
// All active players can announce; stored once and shown via "read w".

use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let body = args.trim();
    if body.is_empty() {
        return "10 Usage: announce <message>\n".to_string();
    }

    let dated = {
        use chrono::{Local, TimeZone};
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs();
        let dt = Local.timestamp_opt(now as i64, 0).single().unwrap_or_else(Local::now);
        format!("{}", dt.format("%a %b %e %T %Y"))
    };

    let full_body = format!(
        "{dated} {from_name} (#{from_cnum}) announces:\n{body}\n",
        from_name = ctx.nat.name,
        from_cnum = ctx.cnum,
    );

    match empire_db::telegrams::announce(ctx.db, ctx.cnum, &full_body).await {
        Ok(_)  => format!("1 Announcement sent to all nations.\n0 announce\n"),
        Err(e) => format!("10 Failed to send announcement: {e}\n"),
    }
}
