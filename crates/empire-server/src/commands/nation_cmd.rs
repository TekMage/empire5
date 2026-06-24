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
// Ported from: src/lib/commands/nati.c

// "nation" command — display a nation's status report.
// Usage: nation [country-name-or-number]  (deities can query others)

use empire_db::{nations, sectors};
use empire_types::nation::Nation;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let subject_cnum = if args.trim().is_empty() {
        ctx.cnum
    } else {
        // Parse as number or name prefix
        match resolve_nation(args.trim(), ctx).await {
            Ok(c) => c,
            Err(e) => return format!("10 {e}\n"),
        }
    };

    // Deities can see any nation; players only their own
    if !ctx.is_deity && subject_cnum != ctx.cnum {
        return "10 Only deities can request a nation report for another country.\n".to_string();
    }

    let nat = match nations::get_by_cnum(ctx.db, subject_cnum).await {
        Ok(Some(n)) => n,
        Ok(None)    => return format!("10 Nation #{subject_cnum} not found.\n"),
        Err(e)      => return format!("10 database error: {e}\n"),
    };

    format_nation_report(&nat, ctx).await
}

async fn resolve_nation(arg: &str, ctx: &CmdCtx<'_>) -> Result<u8, String> {
    if let Ok(n) = arg.parse::<u8>() {
        return Ok(n);
    }
    let all = nations::get_all(ctx.db).await
        .map_err(|e| format!("database error: {e}"))?;
    let arg_lc = arg.to_lowercase();
    all.into_iter()
        .find(|n| n.name.to_lowercase().starts_with(&arg_lc))
        .map(|n| n.cnum)
        .ok_or_else(|| format!("No such country: {arg}"))
}

async fn format_nation_report(nat: &Nation, ctx: &CmdCtx<'_>) -> String {
    let mut out = String::new();

    out.push_str(&format!("1\n"));
    out.push_str(&format!("1 (#{}) {} Nation Report\n", nat.cnum, nat.name));
    out.push_str(&format!("1 Nation status: {:?}\n", nat.status));

    // Capital sector info
    let cap_info = match sectors::get_at(ctx.db, nat.xcap, nat.ycap).await {
        Ok(Some(s)) => {
            let xy = ctx.format_xy(nat.xcap, nat.ycap);
            format!(
                "{}% eff capital at {} has {} civilian(s) & {} military",
                s.effic,
                xy,
                s.items.get(empire_types::commodity::Item::Civil),
                s.items.get(empire_types::commodity::Item::Milit),
            )
        }
        _ => format!("No capital sector at {},{}", nat.xcap, nat.ycap),
    };
    out.push_str(&format!("1 {cap_info}\n"));

    out.push_str(&format!(
        "1  The treasury has ${:.2}     Military reserves: {}\n",
        nat.money as f64, nat.reserve
    ));
    out.push_str(&format!(
        "1 Education......{:8.2}       Happiness.....{:8.2}\n",
        nat.education, nat.happiness
    ));
    out.push_str(&format!(
        "1 Technology.....{:8.2}       Research......{:8.2}\n",
        nat.tech, nat.research
    ));

    // Population limit: rough estimate from research level
    // C formula: max_population(research, SCT_MINE, 0) — deferred to Phase 6
    let max_pop = (nat.research as i64 * 10 + 50_000).min(1_000_000);
    out.push_str(&format!("1 Max population estimate: {max_pop}\n"));
    out.push_str("1\n");

    out.push_str("0 nation\n");
    out
}
