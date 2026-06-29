// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/rea.c

// "read" command — read personal telegrams or announcements.
//
// Usage:
//   read         — read personal telegrams (deletes after display)
//   read w       — read announcements since last time
//   read w <N>   — read announcements from the last N days

use empire_db::telegrams::{TEL_ANNOUNCE, TEL_BULLETIN, TEL_UPDATE};
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let args = args.trim();

    if args.starts_with('w') {
        read_announces(args, ctx).await
    } else {
        read_telegrams(ctx).await
    }
}

async fn read_telegrams(ctx: &CmdCtx<'_>) -> String {
    let tels = match empire_db::telegrams::get_unread(ctx.db, ctx.cnum).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let mut out = String::new();

    if tels.is_empty() {
        out.push_str("1 No telegrams\n");
        out.push_str("0 read\n");
        return out;
    }

    let nations = empire_db::nations::get_all(ctx.db).await.unwrap_or_default();

    for tel in &tels {
        let type_label = match tel.tel_type as i32 {
            TEL_ANNOUNCE => "Announcement",
            TEL_BULLETIN => "Bulletin",
            TEL_UPDATE   => "Update report",
            _            => "Telegram",
        };

        let from_name = if tel.from_cnum == 0 {
            "Server".to_string()
        } else {
            nations.iter()
                .find(|n| n.cnum as i64 == tel.from_cnum)
                .map(|n| n.name.clone())
                .unwrap_or_else(|| format!("#{}", tel.from_cnum))
        };

        out.push_str(&format!("1 --- {type_label} from {from_name} ---\n"));
        for line in tel.body.lines() {
            out.push_str(&format!("1 {line}\n"));
        }
        out.push_str("1 \n");
    }

    // Delete after reading (mirrors 4.4.1 behaviour — mailbox cleared on read)
    if let Err(e) = empire_db::telegrams::mark_read(ctx.db, ctx.cnum).await {
        out.push_str(&format!("10 Warning: could not clear telegrams: {e}\n"));
    }

    out.push_str(&format!("1 {} telegram(s) read.\n", tels.len()));
    out.push_str("0 read\n");
    out
}

async fn read_announces(args: &str, ctx: &CmdCtx<'_>) -> String {
    // Optional: "w <days>" — show last N days; default = since last read
    let since_ts = if let Some(days_str) = args.strip_prefix("w").map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if let Ok(days) = days_str.parse::<i64>() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default().as_secs() as i64;
            now - days * 86400
        } else {
            return "10 Usage: read w [days]\n".to_string();
        }
    } else {
        ctx.nat.last_ann_read
    };

    let anns = match empire_db::telegrams::get_announces(ctx.db, since_ts).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let mut out = String::new();

    if anns.is_empty() {
        out.push_str("1 No new announcements.\n");
        out.push_str("0 read\n");
        // Still reset the counter even if nothing to show
        let _ = empire_db::telegrams::clear_announces(ctx.db, ctx.cnum).await;
        return out;
    }

    let nations = empire_db::nations::get_all(ctx.db).await.unwrap_or_default();

    for ann in &anns {
        let from_name = if ann.from_cnum == 0 {
            "Server".to_string()
        } else {
            nations.iter()
                .find(|n| n.cnum as i64 == ann.from_cnum)
                .map(|n| n.name.clone())
                .unwrap_or_else(|| format!("#{}", ann.from_cnum))
        };

        out.push_str(&format!("1 --- Announcement from {from_name} ---\n"));
        for line in ann.body.lines() {
            out.push_str(&format!("1 {line}\n"));
        }
        out.push_str("1 \n");
    }

    if let Err(e) = empire_db::telegrams::clear_announces(ctx.db, ctx.cnum).await {
        out.push_str(&format!("10 Warning: could not clear announcement counter: {e}\n"));
    }

    out.push_str(&format!("1 {} announcement(s).\n", anns.len()));
    out.push_str("0 read\n");
    out
}
