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
// Ported from: src/lib/commands/enli.c

// "enlist" command — convert civilians to military via enlistment centers.
//
// Usage: enlist <sector-spec> <amount>
//   enlist 0,0 100     — enlist up to 100 military at the capital
//   enlist * 50        — enlist up to 50 per matching sector
//
// Rules:
//   - Sector must be owned by the player
//   - Sector loyalty must be ≤70 (loyal enough to enlist)
//   - Per sector: max = min(civ/2, wanted, 500, 999-current_mil)
//   - Total across all sectors limited to 10000 before "Rioting in induction center"
//   - Civs reduced by amount enlisted

use empire_db::sectors;
use empire_types::commodity::Item;
use super::ctx::CmdCtx;
use super::sector_sel::SectSpec;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: enlist <sector-spec> <amount>\n".to_string();
    }

    let sect_spec = parts[0];
    let wanted: i16 = match parts[1].parse() {
        Ok(n) => n,
        Err(_) => return format!("10 Invalid amount '{}'\n", parts[1]),
    };
    if wanted <= 0 {
        return "10 Amount must be positive\n".to_string();
    }

    let filter = match SectSpec::parse(sect_spec, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
    };

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let mut total_enlisted = 0i32;

    for mut s in all_sectors {
        if s.own != ctx.cnum && !ctx.is_deity { continue; }
        if s.own == 0 { continue; }
        if !filter.matches(&s, ctx.world_x, ctx.world_y) { continue; }

        if total_enlisted >= 10_000 {
            out.push_str("1 Rioting in induction center!\n");
            break;
        }

        let xy = ctx.format_xy(s.x, s.y);
        let loyal = s.loyal;

        // Loyalty check: sector must be loyal enough (≤70 means loyal)
        if loyal > 70 && !ctx.is_deity {
            out.push_str(&format!("1 {xy}: civilians are disloyal (loyalty {}), won't enlist\n", loyal));
            continue;
        }

        let civs = s.items.get(Item::Civil);
        if civs <= 0 {
            out.push_str(&format!("1 {xy}: no civilians to enlist\n"));
            continue;
        }

        let cur_mil = s.items.get(Item::Milit);

        // Cap: 50% of civs, wanted, 500/sector, max 999 mil
        let max_from_civ = civs / 2;
        let max_mil_cap  = (999 - cur_mil).max(0);
        let new_mil = wanted
            .min(max_from_civ)
            .min(500)
            .min(max_mil_cap);

        if new_mil <= 0 {
            out.push_str(&format!("1 {xy}: cannot enlist any military here\n"));
            continue;
        }

        s.items.set(Item::Civil, civs - new_mil);
        s.items.set(Item::Milit, cur_mil + new_mil);
        total_enlisted += new_mil as i32;

        if let Err(e) = sectors::put(ctx.db, &s).await {
            out.push_str(&format!("1 {xy}: sector save error: {e}\n"));
            continue;
        }

        out.push_str(&format!("1 {xy}: enlisted {new_mil} military ({civs}→{} civs)\n",
            civs - new_mil));
    }


    if out.is_empty() {
        out.push_str(&format!("1 {sect_spec}: No matching sectors\n"));
    }
    out.push_str("0 enlist\n");
    out
}
