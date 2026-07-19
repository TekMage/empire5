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
// Ported from: src/lib/commands/boar.c

// "board" command — board and capture an enemy (or undefended) ship.
//
// Usage: board <victim-ship#> <boarder-ship#> [mil]
//
// Both ships must be in the same sea sector. If the victim has no
// military aboard, it's captured automatically with no fight (matches
// the real game's rule that zero defender strength means automatic
// success — see att_resolve_board). Otherwise it's resolved as a single
// combat round, same style as 'attack'.
//
// KNOWN GAPS (v1): real 4.4.1 also supports launching a boarding party
// from a sector's own militia or land units (not just another ship), with
// a mobility-gated "approach" phase where nearby hostile coastal defense
// can fire on the boarder first — not ported, consistent with how
// 'attack'/'assault' already scope out similar approach-phase mechanics.
// Victory transfers a single token mil aboard the captured ship (mirrors
// the reference's per-attacking-object "1 mil moves" rule, simplified
// since this port only ever has one boarding object).

use empire_db::{ships, nations, relations};
use empire_types::commodity::Item;
use empire_types::ship::RetreatFlags;

use rand::SeedableRng;
use rand::rngs::StdRng;

use super::ctx::CmdCtx;
use crate::subs::attsub::{att_resolve_board, at_war};

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: board <victim-ship#> <boarder-ship#> [mil]\n".to_string();
    }

    let victim_uid: i32 = match parts[0].trim_start_matches('#').parse() {
        Ok(n) => n,
        Err(_) => return format!("10 Invalid victim ship '{}'\n", parts[0]),
    };
    let boarder_uid: i32 = match parts[1].trim_start_matches('#').parse() {
        Ok(n) => n,
        Err(_) => return format!("10 Invalid boarder ship '{}'\n", parts[1]),
    };
    let mil_req: Option<i16> = parts.get(2).and_then(|s| s.parse().ok());

    if victim_uid == boarder_uid {
        return "10 A ship can't board itself.\n".to_string();
    }

    let mut victim = match ships::get(ctx.db, victim_uid).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 Ship #{victim_uid} doesn't exist\n"),
        Err(e) => return format!("10 database error: {e}\n"),
    };
    let mut boarder = match ships::get(ctx.db, boarder_uid).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 Ship #{boarder_uid} doesn't exist\n"),
        Err(e) => return format!("10 database error: {e}\n"),
    };

    if boarder.own != ctx.cnum && !ctx.is_deity {
        return format!("10 You don't own ship #{boarder_uid}\n");
    }
    if victim.own == ctx.cnum {
        return "10 You already own that ship.\n".to_string();
    }
    if victim.x != boarder.x || victim.y != boarder.y {
        return format!(
            "10 Ship #{victim_uid} is not in the same sector as #{boarder_uid}\n"
        );
    }

    // Relation check — must be at war (deities exempt), unless the victim
    // is unowned.
    if !ctx.is_deity && victim.own != 0 {
        let rel = match relations::get(ctx.db, ctx.cnum, victim.own).await {
            Ok(r) => r,
            Err(e) => return format!("10 database error: {e}\n"),
        };
        if !at_war(ctx.nat.status, rel) {
            let def_name = match nations::get_by_cnum(ctx.db, victim.own).await {
                Ok(Some(n)) => n.name,
                _ => format!("nation #{}", victim.own),
            };
            return format!(
                "10 You are not at war with {def_name}. Declare war first.\n"
            );
        }
    }

    let boarder_mil = boarder.items.get(Item::Milit);
    let mil = mil_req.unwrap_or(boarder_mil).min(boarder_mil).max(0);
    if mil <= 0 {
        return format!("10 Ship #{boarder_uid} has no military to board with\n");
    }

    let def_mil = victim.items.get(Item::Milit);

    let tech_att = 1.0 + ctx.nat.tech / 100.0;
    let tech_def = if victim.own != 0 {
        match nations::get_by_cnum(ctx.db, victim.own).await {
            Ok(Some(n)) => 1.0 + n.tech / 100.0,
            _ => 1.0,
        }
    } else {
        1.0
    };

    let result = {
        let mut rng = StdRng::from_entropy();
        att_resolve_board(
            mil as i32, boarder.effic, tech_att,
            def_mil as i32, victim.effic, tech_def,
            &mut rng,
        )
    };

    let mut out = String::new();
    out.push_str(&format!(
        "1 Boarding ship #{victim_uid} from #{boarder_uid} with {mil} military\n"
    ));
    for line in &result.log {
        out.push_str(&format!("1 {line}\n"));
    }

    if result.attacker_wins {
        let old_own = victim.own;
        let att_cas = (result.att_casualties as i16).min(mil);
        let survivors = (mil - att_cas).max(0);
        boarder.items.add(Item::Milit, -att_cas);

        // Token prize crew moves aboard the captured ship.
        let prize_crew = survivors.min(1);
        boarder.items.add(Item::Milit, -prize_crew);
        victim.items.set(Item::Milit, prize_crew);
        victim.own = ctx.cnum;
        victim.mission = 0;
        victim.retreat_flags = RetreatFlags::empty();
        victim.retreat_path.clear();

        out.push_str(&format!(
            "1 We have boarded and captured ship #{victim_uid}, sir! (was nation #{old_own})\n"
        ));

        if let Err(e) = ships::put(ctx.db, &boarder).await {
            out.push_str(&format!("1 Boarder save error: {e}\n"));
        }
        if let Err(e) = ships::put(ctx.db, &victim).await {
            out.push_str(&format!("1 Victim save error: {e}\n"));
        }
    } else {
        let att_cas = (result.att_casualties as i16).min(mil);
        let def_cas = (result.def_casualties as i16).min(def_mil);
        boarder.items.add(Item::Milit, -att_cas);
        victim.items.add(Item::Milit, -def_cas);

        out.push_str("1 You have been repelled\n");

        if let Err(e) = ships::put(ctx.db, &boarder).await {
            out.push_str(&format!("1 Boarder save error: {e}\n"));
        }
        if let Err(e) = ships::put(ctx.db, &victim).await {
            out.push_str(&format!("1 Victim save error: {e}\n"));
        }
    }

    out.push_str("0 board\n");
    out
}
