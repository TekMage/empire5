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
// Usage: ship [uid-spec][?realm=N][&type=X]
//   ship *                 — all owned ships
//   ship 0                 — ship uid 0
//   ship 0-5               — ships 0 through 5
//   ship a                 — every ship in fleet 'a'
//   ship ~                 — ships with no fleet assigned
//   ship *?realm=2         — all ships currently within realm 2's
//                            bounding box (see 'info realm')
//   ship *?type=can        — all ships of type 'can' (nuc carrier) --
//                            matches sname or a case-insensitive
//                            substring of the full name, e.g. "carrier"
//   ship *?realm=2&type=can — both filters combined
//
// uid-spec uses the same grammar as navigate/fire/torpedo (see shpsub::
// ship_spec_matches) -- #N there means "uid N", so realm filtering
// uses a separate ?realm=N suffix instead of overloading '#'.

use empire_db::ships;
use empire_types::commodity::Item;
use empire_types::ship_chr::ShipChr;
use super::ctx::CmdCtx;
use super::sector_sel::{in_range_wrap, parse_unit_filters, resolve_realm_filter};
use crate::subs::shpsub::ship_spec_matches;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let (base_spec, filters) = parse_unit_filters(args.trim());

    let realm = match resolve_realm_filter(&filters, ctx).await {
        Ok(r) => r,
        Err(e) => return format!("10 {e}\n"),
    };
    let type_filter = filters.iter().find(|(k, _)| *k == "type").map(|(_, v)| *v);

    let all = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mine: Vec<_> = all.into_iter()
        .filter(|s| s.own == ctx.cnum || ctx.is_deity)
        .filter(|s| ship_spec_matches(base_spec, s))
        .filter(|s| match &realm {
            Some(r) => in_range_wrap(s.x, r.xl, r.xh, ctx.world_x as i16)
                && in_range_wrap(s.y, r.yl, r.yh, ctx.world_y as i16),
            None => true,
        })
        .filter(|s| match type_filter {
            Some(t) => ShipChr::for_type(s.ship_type as usize)
                .map(|c| c.sname.eq_ignore_ascii_case(t)
                    || c.name.to_lowercase().contains(&t.to_lowercase()))
                .unwrap_or(false),
            None => true,
        })
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
