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
// Ported from: src/lib/commands/rela.c

// "relations" command — show diplomatic relations with all known nations.
// Usage: relations [country]   (deity can query from any nation's perspective)

use empire_db::{nations, relations};
use empire_types::nation::NatStatus;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    // Determine whose perspective we're showing
    let as_cnum: u8 = if args.trim().is_empty() {
        ctx.cnum
    } else {
        match resolve_nation(args.trim(), ctx).await {
            Ok(c) => c,
            Err(e) => return format!("10 {e}\n"),
        }
    };

    let all_nations = match nations::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let subject_name = all_nations.iter()
        .find(|n| n.cnum == as_cnum)
        .map(|n| n.name.as_str())
        .unwrap_or("Unknown");

    let mut out = String::new();
    out.push_str(&format!(
        "1 \t{} Diplomatic Relations Report\n",
        subject_name
    ));
    let col_label = if ctx.cnum == as_cnum { "yours" } else { "his" };
    out.push_str(&format!(
        "1   Formal Relations         {:5}      theirs\n",
        col_label
    ));

    for nat in &all_nations {
        let cn = nat.cnum;
        if cn == as_cnum { continue; }
        if nat.status < NatStatus::Sanct { continue; }

        let ours   = relations::get(ctx.db, as_cnum, cn).await
            .unwrap_or(relations::Relation::Neutral);
        let theirs = relations::get(ctx.db, cn, as_cnum).await
            .unwrap_or(relations::Relation::Neutral);

        out.push_str(&format!(
            "1 {:3}) {:<20}  {:<10} {}\n",
            cn,
            &nat.name[..nat.name.len().min(20)],
            ours.name(),
            theirs.name(),
        ));
    }

    out.push_str("0 relations\n");
    out
}

async fn resolve_nation(arg: &str, ctx: &CmdCtx<'_>) -> Result<u8, String> {
    if let Ok(n) = arg.parse::<u8>() { return Ok(n); }
    let all = nations::get_all(ctx.db).await
        .map_err(|e| format!("database error: {e}"))?;
    let arg_lc = arg.to_lowercase();
    all.into_iter()
        .find(|n| n.name.to_lowercase().starts_with(&arg_lc))
        .map(|n| n.cnum)
        .ok_or_else(|| format!("No such country: {arg}"))
}
