// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/chan.c

// "change" command — change country name or representative name.
//
// Usage: change country <new-name>
//        change representative <new-name>
//
// Changing the country name costs 10% of current cash for active nations.
// Changing the representative name is always free.
// Names may not be blank and are trimmed to 19 characters.

use empire_db::nations;
use empire_types::nation::NatStatus;
use super::ctx::CmdCtx;

const MAX_NAME_LEN: usize = 19;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return "10 Usage: change country|representative <new-name>\n".to_string();
    }

    let which = parts[0].trim().to_lowercase();
    let new_name = parts[1].trim();

    if new_name.is_empty() {
        return "10 Name cannot be blank.\n".to_string();
    }
    let new_name = &new_name[..new_name.len().min(MAX_NAME_LEN)];

    let mut nat = ctx.nat.clone();

    match which.as_str() {
        "country" | "c" | "n" => {
            // For active nations, charge 10% of current money.
            let charge = if nat.status == NatStatus::Active && !ctx.is_deity && nat.money > 0 {
                nat.money / 10
            } else {
                0
            };

            // Ensure the name isn't already taken.
            match nations::natbyname(ctx.db, new_name).await {
                Ok(Some(existing)) if existing.cnum != ctx.cnum => {
                    return format!("1 Country name '{}' is already in use.\n0 change\n", new_name);
                }
                Err(e) => return format!("10 DB error: {e}\n"),
                _ => {}
            }

            if charge > 0 {
                nat.money -= charge;
            }
            nat.name = new_name.to_string();
            if let Err(e) = nations::put(ctx.db, &nat).await {
                return format!("10 DB error: {e}\n");
            }
            let msg = if charge > 0 {
                format!("1 Country name changed to '{}' (cost: ${charge}).\n0 change\n", nat.name)
            } else {
                format!("1 Country name changed to '{}'.\n0 change\n", nat.name)
            };
            msg
        }
        "representative" | "rep" | "r" | "p" => {
            nat.representative = new_name.to_string();
            if let Err(e) = nations::put(ctx.db, &nat).await {
                return format!("10 DB error: {e}\n");
            }
            format!("1 Representative name changed to '{}'.\n0 change\n", nat.representative)
        }
        _ => "10 Usage: change country|representative <new-name>\n".to_string(),
    }
}
