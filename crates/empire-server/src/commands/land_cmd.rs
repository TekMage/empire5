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
// Ported from: src/lib/commands/lan.c

// "land" command — list owned land units in human-readable table form.
// Usage: land [uid-spec]
//   land *          — all owned land units
//   land 0          — land unit uid 0
//   land 0-5        — land units 0 through 5

use empire_db::land_units;
use empire_types::commodity::Item;
use empire_types::land_chr::LandChr;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let spec = args.trim();

    let all = match land_units::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mine: Vec<_> = all.into_iter()
        .filter(|u| u.own == ctx.cnum || ctx.is_deity)
        .filter(|u| matches_uid(u.uid, spec))
        .collect();

    let mut out = String::new();
    out.push_str("1 lnd#     land unit type   x,y   ar   eff civ mil  uw  fd mob tech\n");

    for u in &mine {
        let type_name = LandChr::for_type(u.land_type as usize)
            .map(|c| c.name)
            .unwrap_or("unknown");
        let rx = ctx.x_rel(u.x);
        let ry = ctx.y_rel(u.y);
        let ar = if u.army == ' ' { '~' } else { u.army };

        let civ  = u.items.get(Item::Civil);
        let mil  = u.items.get(Item::Milit);
        let uw   = u.items.get(Item::Uw);
        let food = u.items.get(Item::Food);

        out.push_str(&format!(
            "1 {:4}  {:16} {:4},{:<4} {}  {:3}% {:3} {:3} {:3} {:3} {:3} {:4}\n",
            u.uid, type_name, rx, ry, ar, u.effic,
            civ, mil, uw, food,
            u.mobil, u.tech,
        ));
    }

    let n = mine.len();
    out.push_str(&format!("1 {n} unit{}\n", if n == 1 { "" } else { "s" }));
    out.push_str("0 land\n");
    out
}

fn matches_uid(uid: i32, spec: &str) -> bool {
    if spec.is_empty() || spec == "*" { return true; }
    if let Ok(n) = spec.parse::<i32>() { return uid == n; }
    if let Some((lo, hi)) = spec.split_once('-') {
        if let (Ok(lo), Ok(hi)) = (lo.trim().parse::<i32>(), hi.trim().parse::<i32>()) {
            return uid >= lo && uid <= hi;
        }
    }
    true
}
