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
// Usage: land [uid-spec][?realm=N][&type=X]
//   land *                — all owned land units
//   land 0                — land unit uid 0
//   land 0-5              — land units 0 through 5
//   land a                — every unit in army 'a'
//   land ~                — units with no army assigned
//   land *?realm=2        — all units currently within realm 2's
//                           bounding box (see 'info realm')
//   land *?type=cav       — all units of type 'cav' (cavalry) --
//                           matches sname or a case-insensitive
//                           substring of the full name
//   land *?realm=2&type=cav — both filters combined
//
// uid-spec uses the same grammar as march/attack/fire (see lndsub::
// land_spec_matches) -- #N there means "uid N", so realm filtering
// uses a separate ?realm=N suffix instead of overloading '#'.

use empire_db::land_units;
use empire_types::commodity::Item;
use empire_types::land_chr::LandChr;
use super::ctx::CmdCtx;
use super::sector_sel::{in_range_wrap, parse_unit_filters, resolve_realm_filter};
use crate::subs::lndsub::land_spec_matches;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let (base_spec, filters) = parse_unit_filters(args.trim());

    let realm = match resolve_realm_filter(&filters, ctx).await {
        Ok(r) => r,
        Err(e) => return format!("10 {e}\n"),
    };
    let type_filter = filters.iter().find(|(k, _)| *k == "type").map(|(_, v)| *v);

    let all = match land_units::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mine: Vec<_> = all.into_iter()
        .filter(|u| u.own == ctx.cnum || ctx.is_deity)
        .filter(|u| land_spec_matches(base_spec, u))
        .filter(|u| match &realm {
            Some(r) => in_range_wrap(u.x, r.xl, r.xh, ctx.world_x as i16)
                && in_range_wrap(u.y, r.yl, r.yh, ctx.world_y as i16),
            None => true,
        })
        .filter(|u| match type_filter {
            Some(t) => LandChr::for_type(u.land_type as usize)
                .map(|c| c.sname.eq_ignore_ascii_case(t)
                    || c.name.to_lowercase().contains(&t.to_lowercase()))
                .unwrap_or(false),
            None => true,
        })
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
