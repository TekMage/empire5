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
// Ported from: src/lib/commands/sell.c

// "sell" command — list a commodity lot on the market.
//
// Usage: sell ITEM AMOUNT PRICE SECT
//   ITEM   — commodity mnemonic (c,m,s,g,p,i,d,b,f,o,l,h,u,r)
//   AMOUNT — quantity to sell (positive integer)
//   PRICE  — price per unit in dollars (positive float)
//   SECT   — source sector as "x,y" (absolute coordinates)

use empire_db::{sectors, trades};
use empire_types::commodity::Item;
use empire_types::trade::TradeItem;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    // Market gate
    if !ctx.config.game.opt_market {
        return "10 The market is disabled\n".to_string();
    }

    // Parse arguments: ITEM AMOUNT PRICE SECT
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 4 {
        return "10 Usage: sell ITEM AMOUNT PRICE SECT\n".to_string();
    }

    // Parse commodity mnemonic
    let item_char = parts[0].chars().next().unwrap_or('\0');
    let item = match Item::from_mnemonic(item_char) {
        Some(i) => i,
        None => return format!("10 Unknown commodity '{}'. Use c,m,s,g,p,i,d,b,f,o,l,h,u,r\n", parts[0]),
    };

    // Parse amount
    let amount: i32 = match parts[1].parse() {
        Ok(n) if n > 0 => n,
        _ => return format!("10 Invalid amount '{}' — must be a positive integer\n", parts[1]),
    };

    // Parse price
    let price: f64 = match parts[2].parse() {
        Ok(p) if p > 0.0 => p,
        _ => return format!("10 Invalid price '{}' — must be a positive number\n", parts[2]),
    };
    // Cap price to prevent overflow (mirrors C: price > 1000.0 → price = 1000.0)
    let price = price.min(1000.0);

    // Parse sector coordinates "x,y"
    let (sx, sy) = match parse_xy(parts[3]) {
        Some(pair) => pair,
        None => return format!("10 Bad sector '{}' — expected x,y\n", parts[3]),
    };

    // Convert relative coords to absolute
    let abs_x = ctx.x_abs(sx);
    let abs_y = ctx.y_abs(sy);

    // Load the sector
    let mut sector = match sectors::get_at(ctx.db, abs_x, abs_y).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 No sector at {},{}\n", sx, sy),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    // Ownership check
    if sector.own != ctx.cnum && !ctx.is_deity {
        return "10 You don't own that sector.\n".to_string();
    }

    // Check available stock
    let available = sector.items.get(item) as i32;
    if available <= 0 {
        return format!("10 You have no {} there to sell.\n", item.name());
    }

    let to_sell = amount.min(available);

    // Deduct from sector inventory
    sector.items.set(item, (available - to_sell) as i16);
    if let Err(e) = sectors::put(ctx.db, &sector).await {
        return format!("10 Failed to update sector: {e}\n");
    }

    // Create the trade lot
    let uid = match trades::next_uid(ctx.db).await {
        Ok(u) => u,
        Err(e) => return format!("10 Failed to allocate lot ID: {e}\n"),
    };

    let now = chrono::Utc::now().timestamp();
    let lot = TradeItem {
        uid,
        seller: ctx.cnum,
        item,
        amount: to_sell,
        price,
        from_x: abs_x as i16,
        from_y: abs_y as i16,
        created: now,
        bought: false,
        buyer: 0,
    };

    if let Err(e) = trades::put(ctx.db, &lot).await {
        // Rollback the sector deduction on failure
        sector.items.set(item, available as i16);
        let _ = sectors::put(ctx.db, &sector).await;
        return format!("10 Failed to create market listing: {e}\n");
    }

    let total = to_sell as f64 * price;
    format!(
        "1 Sold listing #{uid}: {to_sell} {} at {price:.2}/unit (total {total:.2})\n\
         0 sell\n",
        item.name()
    )
}

/// Parse "x,y" into a pair of i16 coordinates.
fn parse_xy(s: &str) -> Option<(i16, i16)> {
    let mut it = s.splitn(2, ',');
    let x: i16 = it.next()?.trim().parse().ok()?;
    let y: i16 = it.next()?.trim().parse().ok()?;
    Some((x, y))
}
