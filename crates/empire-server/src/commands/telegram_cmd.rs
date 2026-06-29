// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/tele.c

// "telegram" command — send a message to one or more nations.
//
// Usage: telegram <nation> <message>
//        telegram <nation>,<nation>,... <message>
//
// Nation can be a country number or name; '*' sends to all active nations.

use empire_db::telegrams::TEL_NORM;
use empire_types::nation::NatStatus;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    // Split "to_spec message" — first word is the recipient spec
    let (to_spec, body) = match args.split_once(' ') {
        Some((t, b)) => (t.trim(), b.trim()),
        None => return "10 Usage: telegram <nation> <message>\n".to_string(),
    };

    if body.is_empty() {
        return "10 Usage: telegram <nation> <message>\n".to_string();
    }

    // Resolve recipient(s)
    let nations = match empire_db::nations::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let mut recipients: Vec<u8> = Vec::new();

    if to_spec == "*" {
        // Broadcast to all active (deity only, or any player to all?)
        // Classic Empire allows any player to telegram *, so we allow it here too.
        for n in &nations {
            if n.status >= NatStatus::Active && n.cnum != ctx.cnum {
                recipients.push(n.cnum);
            }
        }
    } else {
        // Support comma-separated list: "1,3,5"
        for spec in to_spec.split(',') {
            let spec = spec.trim();
            if let Ok(num) = spec.parse::<u8>() {
                if nations.iter().any(|n| n.cnum == num && n.status >= NatStatus::Active) {
                    recipients.push(num);
                } else {
                    return format!("10 Country {} not found or not active\n", num);
                }
            } else {
                // Try by name
                match nations.iter().find(|n| n.name == spec && n.status >= NatStatus::Active) {
                    Some(n) => recipients.push(n.cnum),
                    None => return format!("10 Country '{}' not found\n", spec),
                }
            }
        }
    }

    if recipients.is_empty() {
        return "10 No recipients found\n".to_string();
    }

    // Lookup sender name for the message header
    let from_name = &ctx.nat.name;

    // Format body with sender attribution
    let dated = {
        use chrono::{Local, TimeZone};
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs();
        let dt = Local.timestamp_opt(now as i64, 0).single().unwrap_or_else(Local::now);
        format!("{}", dt.format("%a %b %e %T %Y"))
    };
    let full_body = format!(
        "Date: {dated}\nFrom: {from_name} (#{from_cnum})\n\n{body}\n",
        from_cnum = ctx.cnum,
    );

    let mut out = String::new();
    for to_cnum in &recipients {
        match empire_db::telegrams::send(ctx.db, *to_cnum, ctx.cnum, TEL_NORM, &full_body).await {
            Ok(_) => {
                let name = nations.iter().find(|n| n.cnum == *to_cnum)
                    .map(|n| n.name.as_str()).unwrap_or("?");
                out.push_str(&format!("1 Telegram sent to {} (#{to_cnum})\n", name));
            }
            Err(e) => out.push_str(&format!("1 Failed to send to #{to_cnum}: {e}\n")),
        }
    }

    out.push_str("0 telegram\n");
    out
}
