// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: include/tel.h, src/lib/common/mailbox.c, src/lib/subs/wu.c

// DB accessors for the telegrams table.
//
// Telegram types (tel_type):
//   0 = TEL_NORM     — normal player-to-player
//   1 = TEL_ANNOUNCE — broadcast announcement (to_cnum = 0, shared)
//   2 = TEL_BULLETIN — deity bulletin
//   3 = TEL_UPDATE   — automatic update report from the server

use sqlx::FromRow;
use crate::{Db, DbResult};

pub const TEL_NORM:     i32 = 0;
pub const TEL_ANNOUNCE: i32 = 1;
pub const TEL_BULLETIN: i32 = 2;
pub const TEL_UPDATE:   i32 = 3;

/// One telegram row from the database.
#[derive(Debug, Clone, FromRow)]
pub struct Telegram {
    pub uid:       i64,
    pub to_cnum:   i64,
    pub from_cnum: i64,
    pub sent_at:   i64,
    pub body:      String,
    pub tel_type:  i64,
}

// ── Send ──────────────────────────────────────────────────────────────────────

/// Send a telegram to a specific nation.  Increments their `tele_cnt`.
pub async fn send(
    db:        &Db,
    to_cnum:   u8,
    from_cnum: u8,
    tel_type:  i32,
    body:      &str,
) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO telegrams (to_cnum, from_cnum, tel_type, body, sent_at) \
         VALUES (?, ?, ?, ?, strftime('%s','now'))",
    )
    .bind(to_cnum as i64)
    .bind(from_cnum as i64)
    .bind(tel_type as i64)
    .bind(body)
    .execute(db.pool()).await?;

    // Increment recipient's unread counter (skip for announcements which use to_cnum=0)
    sqlx::query(
        "UPDATE nations SET tele_cnt = tele_cnt + 1 WHERE cnum = ?",
    )
    .bind(to_cnum as i64)
    .execute(db.pool()).await?;

    Ok(())
}

/// Broadcast an announcement to all active nations.
/// Inserts ONE shared row (to_cnum=0) and increments ann_cnt for every player.
pub async fn announce(db: &Db, from_cnum: u8, body: &str) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO telegrams (to_cnum, from_cnum, tel_type, body, sent_at) \
         VALUES (0, ?, ?, ?, strftime('%s','now'))",
    )
    .bind(from_cnum as i64)
    .bind(TEL_ANNOUNCE as i64)
    .bind(body)
    .execute(db.pool()).await?;

    // Increment ann_cnt for every active nation (status >= 4 = Active)
    sqlx::query(
        "UPDATE nations SET ann_cnt = ann_cnt + 1 WHERE status >= 4",
    )
    .execute(db.pool()).await?;

    Ok(())
}

// ── Read ──────────────────────────────────────────────────────────────────────

/// Return all unread telegrams addressed to `cnum` (types 0, 2, 3).
pub async fn get_unread(db: &Db, cnum: u8) -> DbResult<Vec<Telegram>> {
    Ok(sqlx::query_as::<_, Telegram>(
        "SELECT * FROM telegrams WHERE to_cnum = ? ORDER BY sent_at ASC",
    )
    .bind(cnum as i64)
    .fetch_all(db.pool()).await?)
}

/// Return announcements (to_cnum=0) sent after `since_ts` (unix timestamp).
pub async fn get_announces(db: &Db, since_ts: i64) -> DbResult<Vec<Telegram>> {
    Ok(sqlx::query_as::<_, Telegram>(
        "SELECT * FROM telegrams WHERE to_cnum = 0 AND tel_type = ? AND sent_at > ? \
         ORDER BY sent_at ASC",
    )
    .bind(TEL_ANNOUNCE as i64)
    .bind(since_ts)
    .fetch_all(db.pool()).await?)
}

// ── Clear ─────────────────────────────────────────────────────────────────────

/// Delete all personal telegrams for `cnum` and reset their tele_cnt.
pub async fn mark_read(db: &Db, cnum: u8) -> DbResult<()> {
    sqlx::query("DELETE FROM telegrams WHERE to_cnum = ?")
        .bind(cnum as i64)
        .execute(db.pool()).await?;

    sqlx::query("UPDATE nations SET tele_cnt = 0 WHERE cnum = ?")
        .bind(cnum as i64)
        .execute(db.pool()).await?;

    Ok(())
}

/// Update last_ann_read to now and reset ann_cnt for `cnum`.
pub async fn clear_announces(db: &Db, cnum: u8) -> DbResult<()> {
    sqlx::query(
        "UPDATE nations SET ann_cnt = 0, last_ann_read = strftime('%s','now') WHERE cnum = ?",
    )
    .bind(cnum as i64)
    .execute(db.pool()).await?;
    Ok(())
}
