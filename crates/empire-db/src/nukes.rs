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
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: include/nuke.h
// Known contributors to the original:
//    Dave Pare, 1986
//    Markus Armbruster, 2004-2020

// DB accessors for the nukes table.
// ref: struct nukstr / nuke.h

use sqlx::FromRow;
use empire_types::nuke::Nuke;
use empire_types::coords::{Coord, NatId};
use crate::{Db, DbError, DbResult};

#[derive(FromRow)]
struct NukeRow {
    uid: i64, own: i64, x: i64, y: i64,
    nuke_type: i64, effic: i64, tech: i64,
    stockpile: String, plane: i64,
}

impl From<NukeRow> for Nuke {
    fn from(r: NukeRow) -> Self {
        Nuke {
            uid: r.uid as i32, own: r.own as NatId,
            x: r.x as Coord, y: r.y as Coord,
            nuke_type: r.nuke_type as i8, effic: r.effic as i8,
            tech: r.tech as i16,
            stockpile: r.stockpile.chars().next().unwrap_or(' '),
            plane: r.plane as i32,
        }
    }
}

pub async fn get(db: &Db, uid: i32) -> DbResult<Option<Nuke>> {
    Ok(sqlx::query_as::<_, NukeRow>("SELECT * FROM nukes WHERE uid=?")
        .bind(uid).fetch_optional(db.pool()).await?.map(Nuke::from))
}
pub async fn require(db: &Db, uid: i32) -> DbResult<Nuke> {
    get(db, uid).await?.ok_or_else(|| DbError::NotFound(format!("nuke {uid}")))
}
pub async fn get_all(db: &Db) -> DbResult<Vec<Nuke>> {
    Ok(sqlx::query_as::<_, NukeRow>("SELECT * FROM nukes ORDER BY uid")
        .fetch_all(db.pool()).await?.into_iter().map(Nuke::from).collect())
}
pub async fn get_by_owner(db: &Db, own: NatId) -> DbResult<Vec<Nuke>> {
    Ok(sqlx::query_as::<_, NukeRow>("SELECT * FROM nukes WHERE own=? ORDER BY uid")
        .bind(own as i64)
        .fetch_all(db.pool()).await?.into_iter().map(Nuke::from).collect())
}
pub async fn get_on_plane(db: &Db, plane_uid: i32) -> DbResult<Option<Nuke>> {
    Ok(sqlx::query_as::<_, NukeRow>("SELECT * FROM nukes WHERE plane=?")
        .bind(plane_uid as i64)
        .fetch_optional(db.pool()).await?.map(Nuke::from))
}

pub async fn put(db: &Db, n: &Nuke) -> DbResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO nukes \
         (uid,own,x,y,nuke_type,effic,tech,stockpile,plane,updated_at) \
         VALUES(?,?,?,?,?,?,?,?,?,strftime('%s','now'))",
    )
    .bind(n.uid).bind(n.own as i64).bind(n.x as i64).bind(n.y as i64)
    .bind(n.nuke_type as i64).bind(n.effic as i64).bind(n.tech as i64)
    .bind(n.stockpile.to_string()).bind(n.plane as i64)
    .execute(db.pool()).await?;
    Ok(())
}

pub async fn delete(db: &Db, uid: i32) -> DbResult<()> {
    sqlx::query("DELETE FROM nukes WHERE uid=?")
        .bind(uid).execute(db.pool()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;

    fn make_nuke(uid: i32, own: u8) -> Nuke {
        Nuke { uid, own, x: 0, y: 0, nuke_type: 0, effic: 100, tech: 200,
               stockpile: 'a', plane: -1 }
    }

    #[tokio::test]
    async fn nuke_round_trip() {
        let db = test_db().await;
        let n = make_nuke(0, 1);
        put(&db, &n).await.unwrap();
        let got = get(&db, 0).await.unwrap().unwrap();
        assert_eq!(got.stockpile, 'a');
        assert_eq!(got.tech, 200);
    }

    #[tokio::test]
    async fn get_on_plane_works() {
        let db = test_db().await;
        let mut n = make_nuke(0, 1);
        n.plane = 7;
        put(&db, &n).await.unwrap();
        assert!(get_on_plane(&db, 7).await.unwrap().is_some());
        assert!(get_on_plane(&db, 9).await.unwrap().is_none());
    }
}
