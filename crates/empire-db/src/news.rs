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
// Ported from: src/lib/subs/nreport.c, include/news.h

// DB accessors for the news table.
// nreport() analogue: add_news() coalesces events within a 5-minute window.

use sqlx::FromRow;
use crate::{Db, DbResult};

#[derive(Debug, Clone, FromRow)]
pub struct NewsItem {
    pub id:      i64,
    pub actor:   i64,
    pub verb:    i64,
    pub victim:  i64,
    pub times:   i64,
    pub when_ts: i64,
}

/// File a news event, coalescing into an existing entry if one exists for
/// the same (actor, verb, victim) within the last 5 minutes.
/// Mirrors nreport() / ncache() in nreport.c.
pub async fn add_news(db: &Db, actor: u8, verb: u8, victim: u8, times: i32) -> DbResult<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let window = now - 300; // 5 minutes

    let updated = sqlx::query(
        "UPDATE news SET times = times + ? \
         WHERE actor = ? AND verb = ? AND victim = ? AND when_ts >= ?",
    )
    .bind(times as i64)
    .bind(actor as i64)
    .bind(verb as i64)
    .bind(victim as i64)
    .bind(window)
    .execute(db.pool()).await?
    .rows_affected();

    if updated == 0 {
        sqlx::query(
            "INSERT INTO news (actor, verb, victim, times, when_ts) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(actor as i64)
        .bind(verb as i64)
        .bind(victim as i64)
        .bind(times as i64)
        .bind(now)
        .execute(db.pool()).await?;
    }

    Ok(())
}

/// Return all news items at or after `since_ts`, ordered by time.
pub async fn get_since(db: &Db, since_ts: i64) -> DbResult<Vec<NewsItem>> {
    Ok(sqlx::query_as::<_, NewsItem>(
        "SELECT * FROM news WHERE when_ts >= ? ORDER BY when_ts ASC",
    )
    .bind(since_ts)
    .fetch_all(db.pool()).await?)
}

/// Delete news items older than `cutoff_ts` (called from update engine).
pub async fn delete_older_than(db: &Db, cutoff_ts: i64) -> DbResult<()> {
    sqlx::query("DELETE FROM news WHERE when_ts < ?")
        .bind(cutoff_ts)
        .execute(db.pool()).await?;
    Ok(())
}
