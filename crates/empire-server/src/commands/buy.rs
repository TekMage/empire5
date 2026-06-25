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
// Ported from: src/lib/commands/buy.c and src/lib/commands/mark.c

// "buy" command — browse or purchase commodity lots.
//
// Usage:
//   buy           — list all active market lots
//   buy LOT#      — purchase the specified lot

use empire_db::{nations, sectors, trades};
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    if !ctx.config.game.opt_market {
        return "10 The market is disabled\n".to_string();
    }

    let arg = args.trim();
    if arg.is_empty() {
        list_market(ctx).await
    } else {
        match arg.parse::<i32>() {
            Ok(lot_uid) => purchase_lot(lot_uid, ctx).await,
            Err(_) => "10 Usage: buy [LOT#]\n".to_string(),
        }
    }
}

/// List all active market lots.
async fn list_market(ctx: &CmdCtx<'_>) -> String {
    let lots = match trades::get_active(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    if lots.is_empty() {
        return "1 No commodity lots currently on the market.\n0 buy\n".to_string();
    }

    let mut out = String::new();
    out.push_str("1  #  Seller  Item                         Amt    Price/unit    Total      From\n");
    out.push_str("1  -  ------  ---------------------------  -----  ----------  ---------  -----\n");

    for lot in &lots {
        let seller_name = nation_name(ctx, lot.seller).await;
        let total = lot.amount as f64 * lot.price;
        out.push_str(&format!(
            "1 {:2}  {:<6}  {:<27}  {:5}    {:8.2}  {:9.2}  {},{}\n",
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

    out.push_str(&format!("1 {} lot(s) available.\n0 buy\n", lots.len()));
    out
}

/// Purchase a specific lot by uid.
async fn purchase_lot(uid: i32, ctx: &CmdCtx<'_>) -> String {
    // Load the lot
    let mut lot = match trades::get_by_uid(ctx.db, uid).await {
        Ok(Some(l)) => l,
        Ok(None) => return format!("10 Lot #{uid} does not exist.\n"),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    if lot.bought {
        return format!("10 Lot #{uid} has already been purchased.\n");
    }

    if lot.seller == ctx.cnum && !ctx.is_deity {
        return "10 You can't buy your own lot.\n".to_string();
    }

    let cost = lot.amount as f64 * lot.price;

    // Load buyer nation
    let mut buyer_nat = match nations::get_by_cnum(ctx.db, ctx.cnum).await {
        Ok(Some(n)) => n,
        Ok(None) => return "10 Internal error: your nation not found\n".to_string(),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    if (buyer_nat.money as f64) < cost {
        return format!(
            "10 You can't afford lot #{uid}: costs ${cost:.2}, you have ${}\n",
            buyer_nat.money
        );
    }

    // Load seller nation
    let mut seller_nat = match nations::get_by_cnum(ctx.db, lot.seller).await {
        Ok(Some(n)) => n,
        Ok(None) => return "10 Internal error: seller nation not found\n".to_string(),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    // Find delivery sector: buyer's capital, falling back to any owned sector
    let delivery_sector = match find_delivery_sector(ctx).await {
        Ok(Some(s)) => s,
        Ok(None) => return "10 You have no sectors to receive delivery.\n".to_string(),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    let (del_x, del_y) = (delivery_sector.x, delivery_sector.y);
    let mut del_sector = delivery_sector;

    // Check sector capacity
    let current = del_sector.items.get(lot.item) as i32;
    let new_total = current + lot.amount;
    if new_total > i16::MAX as i32 {
        return format!(
            "10 Your sector at {del_x},{del_y} cannot hold {} more {}: currently holds {current}\n",
            lot.amount,
            lot.item.name()
        );
    }

    // Transfer money
    buyer_nat.money -= cost as i32;
    seller_nat.money += cost as i32;

    // Deliver goods
    del_sector.items.set(lot.item, new_total as i16);

    // Mark lot as sold
    lot.bought = true;
    lot.buyer = ctx.cnum;

    // Persist everything
    let r1 = nations::put(ctx.db, &buyer_nat).await;
    let r2 = nations::put(ctx.db, &seller_nat).await;
    let r3 = sectors::put(ctx.db, &del_sector).await;
    let r4 = trades::put(ctx.db, &lot).await;

    for r in [r1, r2, r3, r4] {
        if let Err(e) = r {
            return format!("10 Database error during purchase: {e}\n");
        }
    }

    let rel_del = ctx.format_xy(del_x, del_y);
    format!(
        "1 Bought lot #{uid}: {} {} for ${cost:.2}\n\
         1 Delivered to your sector at {rel_del}.\n\
         0 buy\n",
        lot.amount,
        lot.item.name()
    )
}

/// Find the best delivery sector for the buyer.
/// Prefers the capital; falls back to any owned sector.
async fn find_delivery_sector(
    ctx: &CmdCtx<'_>,
) -> Result<Option<empire_types::sector::Sector>, empire_db::DbError> {
    // Try capital first
    let cap = sectors::get_at(ctx.db, ctx.nat.xcap, ctx.nat.ycap).await?;
    if let Some(s) = cap {
        if s.own == ctx.cnum {
            return Ok(Some(s));
        }
    }
    // Fall back to any owned sector
    let owned = sectors::get_by_owner(ctx.db, ctx.cnum).await?;
    Ok(owned.into_iter().next())
}

/// Resolve a nation name for display (truncated to fit table).
async fn nation_name(ctx: &CmdCtx<'_>, cnum: u8) -> String {
    match nations::get_by_cnum(ctx.db, cnum).await {
        Ok(Some(n)) => n.name,
        _ => format!("#{cnum}"),
    }
}
