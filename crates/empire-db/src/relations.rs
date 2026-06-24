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
// Ported from: include/nat.h (struct relatstr, enum relations), src/lib/subs/nat.c
// Known contributors to the original:
//    Dave Pare, 1989

// Diplomatic relations between nations.
// Stored in `relations` table: (cnum, target) → relate.
// Default for any missing pair: NEUTRAL.

use crate::{Db, DbResult};
use empire_types::coords::NatId;

/// Diplomatic relation from one nation toward another.
/// Matches C enum `relations` in include/nat.h (AT_WAR=0, HOSTILE=1, ..., ALLIED=4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Relation {
    AtWar    = 0,
    Hostile  = 1,
    Neutral  = 2,
    Friendly = 3,
    Allied   = 4,
}

impl Relation {
    pub fn name(self) -> &'static str {
        match self {
            Relation::AtWar    => "At War",
            Relation::Hostile  => "Hostile",
            Relation::Neutral  => "Neutral",
            Relation::Friendly => "Friendly",
            Relation::Allied   => "Allied",
        }
    }

    pub fn from_char(c: char) -> Option<Relation> {
        match c.to_ascii_lowercase() {
            'w' => Some(Relation::AtWar),
            'h' => Some(Relation::Hostile),
            'n' => Some(Relation::Neutral),
            'f' => Some(Relation::Friendly),
            'a' => Some(Relation::Allied),
            _   => None,
        }
    }
}

fn from_i64(v: i64) -> Relation {
    match v {
        0 => Relation::AtWar,
        1 => Relation::Hostile,
        3 => Relation::Friendly,
        4 => Relation::Allied,
        _ => Relation::Neutral,
    }
}

/// Return the relation cnum has toward target (NEUTRAL if not set).
pub async fn get(db: &Db, cnum: NatId, target: NatId) -> DbResult<Relation> {
    let v: Option<i64> = sqlx::query_scalar(
        "SELECT relate FROM relations WHERE cnum=? AND target=?"
    )
    .bind(cnum as i64)
    .bind(target as i64)
    .fetch_optional(db.pool()).await?;
    Ok(v.map(from_i64).unwrap_or(Relation::Neutral))
}

/// Set cnum's relation toward target.
pub async fn set(db: &Db, cnum: NatId, target: NatId, rel: Relation) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO relations (cnum, target, relate) VALUES (?, ?, ?)
         ON CONFLICT(cnum, target) DO UPDATE SET relate=excluded.relate"
    )
    .bind(cnum as i64)
    .bind(target as i64)
    .bind(rel as u8 as i64)
    .execute(db.pool()).await?;
    Ok(())
}

/// Return all explicitly set relations for cnum, sorted by target.
pub async fn get_all_for(db: &Db, cnum: NatId) -> DbResult<Vec<(NatId, Relation)>> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT target, relate FROM relations WHERE cnum=? ORDER BY target"
    )
    .bind(cnum as i64)
    .fetch_all(db.pool()).await?;
    Ok(rows.into_iter().map(|(t, r)| (t as NatId, from_i64(r))).collect())
}
