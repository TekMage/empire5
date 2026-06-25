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
// DB accessors for the trade_items table.
// ref: struct comstr in commodity.h

use sqlx::FromRow;
use empire_types::trade::TradeItem;
use empire_types::commodity::Item;
use crate::{Db, DbResult};

// ── Raw DB row ────────────────────────────────────────────────────────────────

#[derive(FromRow)]
struct TradeRow {
    uid:     i64,
    seller:  i64,
    item:    i64,
    amount:  i64,
    price:   f64,
    from_x:  i64,
    from_y:  i64,
    created: i64,
    bought:  i64,
    buyer:   i64,
}

// ── Conversions ───────────────────────────────────────────────────────────────

fn row_to_trade(r: TradeRow) -> Option<TradeItem> {
    let item = Item::try_from_i32(r.item as i32)?;
    Some(TradeItem {
        uid:     r.uid as i32,
        seller:  r.seller as u8,
        item,
        amount:  r.amount as i32,
        price:   r.price,
        from_x:  r.from_x as i16,
        from_y:  r.from_y as i16,
        created: r.created,
        bought:  r.bought != 0,
        buyer:   r.buyer as u8,
    })
}

// ── Reads ─────────────────────────────────────────────────────────────────────

/// Return every trade_items row, in uid order.
pub async fn get_all(db: &Db) -> DbResult<Vec<TradeItem>> {
    let rows = sqlx::query_as::<_, TradeRow>(
        "SELECT uid,seller,item,amount,price,from_x,from_y,created,bought,buyer \
         FROM trade_items ORDER BY uid",
    )
    .fetch_all(db.pool())
    .await?;

    Ok(rows.into_iter().filter_map(row_to_trade).collect())
}

/// Return the trade lot with the given uid, or `None` if absent.
pub async fn get_by_uid(db: &Db, uid: i32) -> DbResult<Option<TradeItem>> {
    let row = sqlx::query_as::<_, TradeRow>(
        "SELECT uid,seller,item,amount,price,from_x,from_y,created,bought,buyer \
         FROM trade_items WHERE uid = ?",
    )
    .bind(uid)
    .fetch_optional(db.pool())
    .await?;

    Ok(row.and_then(row_to_trade))
}

/// Return all lots that have not yet been purchased (`bought = 0`).
pub async fn get_active(db: &Db) -> DbResult<Vec<TradeItem>> {
    let rows = sqlx::query_as::<_, TradeRow>(
        "SELECT uid,seller,item,amount,price,from_x,from_y,created,bought,buyer \
         FROM trade_items WHERE bought = 0 ORDER BY uid",
    )
    .fetch_all(db.pool())
    .await?;

    Ok(rows.into_iter().filter_map(row_to_trade).collect())
}

// ── Writes ────────────────────────────────────────────────────────────────────

/// Insert or replace a trade lot.
pub async fn put(db: &Db, t: &TradeItem) -> DbResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO trade_items \
         (uid,seller,item,amount,price,from_x,from_y,created,bought,buyer) \
         VALUES (?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(t.uid)
    .bind(t.seller as i64)
    .bind(t.item as i32 as i64)
    .bind(t.amount as i64)
    .bind(t.price)
    .bind(t.from_x as i64)
    .bind(t.from_y as i64)
    .bind(t.created)
    .bind(t.bought as i64)
    .bind(t.buyer as i64)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Delete a trade lot by uid.
pub async fn delete(db: &Db, uid: i32) -> DbResult<()> {
    sqlx::query("DELETE FROM trade_items WHERE uid = ?")
        .bind(uid)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Return the next available uid (max existing + 1, minimum 1).
pub async fn next_uid(db: &Db) -> DbResult<i32> {
    let row: (Option<i64>,) =
        sqlx::query_as("SELECT MAX(uid) FROM trade_items")
            .fetch_one(db.pool())
            .await?;
    Ok(row.0.unwrap_or(0) as i32 + 1)
}
