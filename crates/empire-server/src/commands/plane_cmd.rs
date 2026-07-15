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
// Ported from: src/lib/commands/plan.c

// "plane" command — list owned planes in human-readable table form.
// Usage: plane [uid-spec][?realm=N][&type=X]
//   plane *                — all owned planes
//   plane 0                — plane uid 0
//   plane 0-5              — planes 0 through 5
//   plane c                — every plane in wing 'c'
//   plane ~                — planes with no wing assigned
//   plane *?realm=2        — all planes currently within realm 2's
//                            bounding box (see 'info realm')
//   plane *?type=f35       — all planes of type f35 (matches sname or
//                            a case-insensitive substring of the full
//                            name, e.g. "raptor" matches "F-22 Raptor")
//   plane *?realm=2&type=f35 — both filters combined
//
// uid-spec uses the same grammar as fly/bomb/recon (see plnsub::
// plane_spec_matches) -- #N there means "uid N", so realm filtering
// uses a separate ?realm=N suffix instead of overloading '#'.

use empire_db::planes;
use empire_types::plane_chr::PlaneChr;
use super::ctx::CmdCtx;
use super::sector_sel::{in_range_wrap, parse_unit_filters, resolve_realm_filter};
use crate::subs::plnsub::plane_spec_matches;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let (base_spec, filters) = parse_unit_filters(args.trim());

    let realm = match resolve_realm_filter(&filters, ctx).await {
        Ok(r) => r,
        Err(e) => return format!("10 {e}\n"),
    };
    let type_filter = filters.iter().find(|(k, _)| *k == "type").map(|(_, v)| *v);

    let all = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mine: Vec<_> = all.into_iter()
        .filter(|p| p.own == ctx.cnum || ctx.is_deity)
        .filter(|p| plane_spec_matches(base_spec, p))
        .filter(|p| match &realm {
            Some(r) => in_range_wrap(p.x, r.xl, r.xh, ctx.world_x as i16)
                && in_range_wrap(p.y, r.yl, r.yh, ctx.world_y as i16),
            None => true,
        })
        .filter(|p| match type_filter {
            Some(t) => PlaneChr::for_type(p.plane_type as usize)
                .map(|c| c.sname.eq_ignore_ascii_case(t)
                    || c.name.to_lowercase().contains(&t.to_lowercase()))
                .unwrap_or(false),
            None => true,
        })
        .collect();

    let mut out = String::new();
    out.push_str("1 pln#     plane type       x,y   wg   eff mob tech mission range\n");

    for p in &mine {
        let type_name = PlaneChr::for_type(p.plane_type as usize)
            .map(|c| c.name)
            .unwrap_or("unknown");
        let rx = ctx.x_rel(p.x);
        let ry = ctx.y_rel(p.y);
        let wg = if p.wing == ' ' { '~' } else { p.wing };

        out.push_str(&format!(
            "1 {:4}  {:16} {:4},{:<4} {}  {:3}% {:3} {:4} {:7} {:5}\n",
            p.uid, type_name, rx, ry, wg,
            p.effic, p.mobil, p.tech, p.mission, p.range,
        ));
    }

    let n = mine.len();
    out.push_str(&format!("1 {n} plane{}\n", if n == 1 { "" } else { "s" }));
    out.push_str("0 plane\n");
    out
}
