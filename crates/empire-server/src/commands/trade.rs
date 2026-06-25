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
// Ported from: src/lib/commands/trad.c and src/lib/commands/mark.c

// "trade" command — display the full commodity market overview.
//
// Shows all active lots (not yet bought), including the player's own.
// Usage: trade

use empire_db::{nations, trades};
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let _ = args; // no arguments used

    if !ctx.config.game.opt_market {
        return "10 The market is disabled\n".to_string();
    }

    let lots = match trades::get_active(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    let mut out = String::new();
    out.push_str("1 Commodity market listing:\n");
    out.push_str("1  #  Seller  Item                         Amt    Price/unit    Total      Sector\n");
    out.push_str("1  -  ------  ---------------------------  -----  ----------  ---------  --------\n");

    if lots.is_empty() {
        out.push_str("1 The market is empty.\n");
        out.push_str("0 trade\n");
        return out;
    }

    for lot in &lots {
        let seller_name = match nations::get_by_cnum(ctx.db, lot.seller).await {
            Ok(Some(n)) => n.name,
            _ => format!("#{}", lot.seller),
        };
        let total = lot.amount as f64 * lot.price;
        let own_marker = if lot.seller == ctx.cnum { " *" } else { "  " };
        out.push_str(&format!(
            "1{own_marker}{:2}  {:<6}  {:<27}  {:5}    {:8.2}  {:9.2}  {},{}\n",
            lot.uid,
            &seller_name[..seller_name.len().min(6)],
            lot.item.name(),
            lot.amount,
            lot.price,
            total,
            lot.from_x,
            lot.from_y,
        ));
    }

    out.push_str(&format!("1 {} lot(s) available.\n0 trade\n", lots.len()));
    out
}
