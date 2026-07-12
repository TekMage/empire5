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
// Usage: plane [uid-spec]
//   plane *          — all owned planes
//   plane 0          — plane uid 0
//   plane 0-5        — planes 0 through 5

use empire_db::planes;
use empire_types::plane_chr::PlaneChr;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let spec = args.trim();

    let all = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mine: Vec<_> = all.into_iter()
        .filter(|p| p.own == ctx.cnum || ctx.is_deity)
        .filter(|p| matches_uid(p.uid, spec))
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
