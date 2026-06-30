// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/name.c

// "name" command — give a ship a custom name (up to 24 characters).
//
// Usage: name SHIP# NAME
//        name 3 "HMS Victory"
//        name 3 ~            (clears the name)

use empire_db::ships;
use super::ctx::CmdCtx;

const MAX_NAME_LEN: usize = 24;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].trim().is_empty() {
        return "10 Usage: name SHIP# NAME  (use ~ to clear)\n".to_string();
    }

    let uid: i32 = match parts[0].trim().parse() {
        Ok(n) => n,
        Err(_) => return format!("10 Invalid ship number '{}'\n", parts[0]),
    };

    let new_name = parts[1].trim().trim_matches('"');

    let mut all = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let ship = match all.iter_mut().find(|s| s.uid == uid) {
        Some(s) => s,
        None => return format!("10 Ship #{uid} not found\n"),
    };

    if ship.own != ctx.cnum && !ctx.is_deity {
        return format!("10 Ship #{uid} is not yours\n");
    }

    if new_name == "~" {
        ship.name.clear();
    } else {
        let trimmed: String = new_name.chars().take(MAX_NAME_LEN).collect();
        ship.name = trimmed;
    }

    if let Err(e) = ships::put(ctx.db, ship).await {
        return format!("10 database error: {e}\n");
    }

    let display = if ship.name.is_empty() {
        "(unnamed)".to_string()
    } else {
        ship.name.clone()
    };
    format!("1 Ship #{uid} named \"{display}\"\n0 name\n")
}
