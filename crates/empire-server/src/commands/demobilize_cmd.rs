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
// Ported from: src/lib/commands/demo.c

// "demobilize" command — convert sector military into civilians, optionally
// also crediting the national reserve.
//
// Usage: demobilize <sector-spec> <amount> [reserve|civ]
//   demobilize * 100          — demob up to 100 mil/sector to civ + reserve
//   demobilize 0,0 50 civ     — demob 50 mil to civilians only, no reserve
//   demobilize * -10          — keep 10 mil per sector, demob the rest
//
// Rules (from demo.c):
//   - Sector must be ≥60% efficient and owned (old_own == own)
//   - Costs $5 per military demobilized
//   - Demobilized mil ALWAYS become civilians in the sector (civ += delta,
//     mil -= delta) — that's what "demobilize" means. This is unconditional
//     in the reference; it does not depend on the reserve/civ mode.
//   - By default (or "reserve"): demobilized troops ALSO get added to
//     nat_reserve, on top of becoming civilians — the reserve is a separate
//     national callback pool, not an alternative destination.
//   - With "civ": civilians only, no reserve credit.
//   - Negative amount: keep |amount| mil, demob the rest
//   - Civ count is capped at ITEM_MAX (9999) per sector in all modes.

use empire_db::{sectors, nations};
use empire_types::commodity::Item;
use super::ctx::CmdCtx;
use super::sector_sel::SectSpec;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: demobilize <sector-spec> <amount> [reserve|civ]\n".to_string();
    }

    let sect_spec = parts[0];
    let amount: i16 = match parts[1].parse() {
        Ok(n) => n,
        Err(_) => return format!("10 Invalid amount '{}'\n", parts[1]),
    };

    // Third arg: "civ" to convert to civilians; default is reserve
    let to_reserve = parts.get(2).map(|s| *s != "civ").unwrap_or(true);

    let filter = match SectSpec::parse(sect_spec, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
    };

    let mut nat = match nations::get_by_cnum(ctx.db, ctx.cnum).await {
        Ok(Some(n)) => n,
        Ok(None) => return "10 Internal error: nation not found\n".to_string(),
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let mut total_demob = 0i32;
    let mut total_cost = 0.0f64;

    for mut s in all_sectors {
        if s.own != ctx.cnum && !ctx.is_deity { continue; }
        if s.own == 0 { continue; }
        if !filter.matches(&s, ctx.world_x, ctx.world_y) { continue; }

        if s.effic < 60 && !ctx.is_deity { continue; }

        // Sector must still be under original ownership (not freshly conquered)
        if s.old_own != s.own && !ctx.is_deity { continue; }

        let mil = s.items.get(Item::Milit);
        if mil == 0 { continue; }

        let civ = s.items.get(Item::Civil);

        // Negative amount means "keep |amount|, demob the rest"
        let delta: i16 = if amount < 0 {
            let keep = (-amount).min(mil);
            mil - keep
        } else {
            amount.min(mil)
        };
        if delta <= 0 { continue; }

        // Demobilized mil always become civilians — cap by ITEM_MAX (9999).
        const ITEM_MAX: i16 = 9999;
        let delta = delta.min(ITEM_MAX - civ);
        if delta <= 0 { continue; }

        // Cost check: $5 per demobilized mil
        let cost = delta as f64 * 5.0;
        if nat.money as f64 - total_cost - cost < 0.0 {
            let xy = ctx.format_xy(s.x, s.y);
            out.push_str(&format!("1 Can't afford to demobilize {} military in {xy}\n", delta));
            break;
        }

        total_cost += cost;
        s.items.set(Item::Milit, mil - delta);
        s.items.set(Item::Civil, civ + delta);

        let xy = ctx.format_xy(s.x, s.y);
        out.push_str(&format!(
            "1 {} demobilized in {xy} ({} mil left)\n",
            delta, mil - delta
        ));

        if let Err(e) = sectors::put(ctx.db, &s).await {
            out.push_str(&format!("1 {xy}: sector save error: {e}\n"));
            continue;
        }

        if to_reserve {
            nat.reserve += delta as i32;
        }
        total_demob += delta as i32;
    }

    if total_demob == 0 {
        out.push_str("1 No eligible sectors/military for demobilization\n");
    } else {
        nat.money -= total_cost as i32;
        out.push_str(&format!(
            "1 {} military converted to {} new civilians\n",
            total_demob, total_demob
        ));
        if to_reserve {
            out.push_str(&format!(
                "1 Military reserve now {}\n",
                nat.reserve
            ));
        }
        if let Err(e) = nations::put(ctx.db, &nat).await {
            out.push_str(&format!("1 Warning: could not save nation update: {e}\n"));
        }
    }

    out.push_str("0 demobilize\n");
    out
}
