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
// Ported from: src/lib/commands/add.c

// "add" command — create or reset a nation slot.
// Usage: add <cnum|?> <name> <representative> <status>
//   status: v=visitor, p=player, g=god, d=delete
// Deity only.

use empire_db::nations;
use empire_types::nation::{Nation, NatFlags, NatStatus};
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    if !ctx.is_deity {
        return "10 Permission denied: deity only\n".to_string();
    }

    let parts: Vec<&str> = args.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return "10 Usage: add <cnum|?> <name> <representative> <v|p|g|d>\n".to_string();
    }

    let cnum_str     = parts[0].trim();
    let name_str     = parts[1].trim();
    let rep_str      = parts[2].trim();
    let status_char  = parts[3].trim().chars().next().unwrap_or(' ');

    // Validate name and representative lengths
    if name_str.len() > 20 {
        return "10 Name too long (max 20 chars)\n".to_string();
    }
    if rep_str.len() > 20 {
        return "10 Representative too long (max 20 chars)\n".to_string();
    }

    // Parse target status
    let new_status = match status_char {
        'v' | 'V' => NatStatus::Visitor,
        'p' | 'P' => NatStatus::Active,
        'g' | 'G' => NatStatus::Deity,
        'd' | 'D' => NatStatus::Unused,
        _ => return format!("10 Unknown status char '{}'; use v/p/g/d\n", status_char),
    };

    // Resolve target cnum
    let target_cnum: u8 = if cnum_str == "?" {
        // Find first unused slot (1..=99)
        let all = match nations::get_all(ctx.db).await {
            Ok(v) => v,
            Err(e) => return format!("10 Database error: {e}\n"),
        };
        let used: std::collections::HashSet<u8> = all.iter().map(|n| n.cnum).collect();
        match (1u8..=99).find(|c| !used.contains(c)) {
            Some(c) => c,
            None => return "10 No free country slots available\n".to_string(),
        }
    } else {
        match cnum_str.parse::<u8>() {
            Ok(c) => c,
            Err(_) => return format!("10 Invalid country number '{}'\n", cnum_str),
        }
    };

    // Validate: cannot add or modify country 0 (deity slot)
    if target_cnum == 0 {
        return "10 Cannot modify country 0\n".to_string();
    }
    if target_cnum > 99 {
        return format!("10 Country number {} out of range (1-99)\n", target_cnum);
    }

    // Load existing nation or build a blank one
    let existing = match nations::get_by_cnum(ctx.db, target_cnum).await {
        Ok(opt) => opt,
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    let start_cash = ctx.config.game.start_cash;

    let mut nat = existing.unwrap_or_else(|| Nation {
        uid: target_cnum as i32,
        cnum: target_cnum,
        status: NatStatus::Unused,
        flags: NatFlags::empty(),
        name: String::new(),
        representative: String::new(),
        host_addr: String::new(),
        user_id: String::new(),
        xcap: 0, ycap: 0, xorg: 0, yorg: 0,
        money: 0, reserve: 0,
        tech: 0.0, research: 0.0, education: 0.0, happiness: 0.0,
        login_count: 0, tele_cnt: 0,
        passwd_hash: String::new(),
        last_login: 0, last_logout: 0,
    });

    // Reset the nation fields
    nat.status       = new_status;
    nat.name         = name_str.to_string();
    nat.representative = rep_str.to_string();
    nat.flags        = NatFlags::empty();
    nat.money        = if new_status == NatStatus::Unused { 0 } else { start_cash };
    nat.tech         = 0.0;
    nat.research     = 0.0;
    nat.education    = 0.0;
    nat.happiness    = 0.0;
    nat.reserve      = 0;
    nat.xcap         = 0;
    nat.ycap         = 0;
    nat.xorg         = 0;
    nat.yorg         = 0;
    nat.passwd_hash  = String::new();  // force password reset

    if let Err(e) = nations::put(ctx.db, &nat).await {
        return format!("10 Database error saving nation: {e}\n");
    }

    let action = if new_status == NatStatus::Unused { "deleted" } else { "created" };
    format!(
        "1 Country {}, {}, {}.\n0 add\n",
        target_cnum, name_str, action
    )
}
