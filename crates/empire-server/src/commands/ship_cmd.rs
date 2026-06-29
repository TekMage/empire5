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
// Ported from: src/lib/commands/shi.c

// "ship" command — list owned ships in human-readable table form.
// Usage: ship [uid-spec]
//   ship *           — all owned ships
//   ship 0           — ship uid 0
//   ship 0-5         — ships 0 through 5

use empire_db::ships;
use empire_types::commodity::Item;
use empire_types::ship_chr::ShipChr;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let spec = args.trim();

    let all = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mine: Vec<_> = all.into_iter()
        .filter(|s| s.own == ctx.cnum || ctx.is_deity)
        .filter(|s| matches_ship(s.uid, spec))
        .collect();

    let mut out = String::new();
    out.push_str("1 shp#     ship type       x,y   fl   eff civ mil  uw  fd pn he xl ln mob tech\n");

    for s in &mine {
        let type_name = ShipChr::for_type(s.ship_type as usize)
            .map(|c| c.name)
            .unwrap_or("unknown");
        let rx = ctx.x_rel(s.x);
        let ry = ctx.y_rel(s.y);
        let fl = if s.fleet == ' ' { '~' } else { s.fleet };

        let civ  = s.items.get(Item::Civil);
        let mil  = s.items.get(Item::Milit);
        let uw   = s.items.get(Item::Uw);
        let food = s.items.get(Item::Food);

        // Placeholder zero counts for planes/helicopters/xlight/land
        // (full cargo tracking added when those unit types are populated)
        out.push_str(&format!(
            "1 {:4}  {:16} {:4},{:<4} {}  {:3}% {:3} {:3} {:3} {:3}  0  0  0  0 {:3} {:4}\n",
            s.uid, type_name, rx, ry, fl, s.effic,
            civ, mil, uw, food,
            s.mobil, s.tech,
        ));
    }

    let n = mine.len();
    out.push_str(&format!("1 {n} ship{}\n", if n == 1 { "" } else { "s" }));
    out.push_str("0 ship\n");
    out
}

fn matches_ship(uid: i32, spec: &str) -> bool {
    if spec.is_empty() || spec == "*" { return true; }
    if let Ok(n) = spec.parse::<i32>() { return uid == n; }
    if let Some((lo, hi)) = spec.split_once('-') {
        if let (Ok(lo), Ok(hi)) = (lo.trim().parse::<i32>(), hi.trim().parse::<i32>()) {
            return uid >= lo && uid <= hi;
        }
    }
    true
}
