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
// Ported from: include/nat.h
// Known contributors to the original:
//    Thomas Ruschak
//    Ken Stevens, 1995
//    Steve McClure, 1998-2000

// DB accessors for the nations table.
// ref: struct natstr / nat.h, ef_read/ef_write pattern from file.c

use sqlx::FromRow;
use empire_types::nation::{Nation, NatFlags, NatStatus, Realm};
use empire_types::coords::Coord;
use crate::{Db, DbError, DbResult};

// ── Raw DB row ───────────────────────────────────────────────────────────────

#[derive(FromRow)]
struct NationRow {
    uid: i64, cnum: i64, status: i64, flags: i64,
    name: String, representative: String, host_addr: String, user_id: String,
    xcap: i64, ycap: i64, xorg: i64, yorg: i64,
    money: i64, reserve: i64,
    tech: f64, research: f64, education: f64, happiness: f64,
    login_count: i64, tele_cnt: i64,
}

#[derive(FromRow)]
struct RealmRow {
    uid: i64, cnum: i64, realm: i64,
    xl: i64, xh: i64, yl: i64, yh: i64,
}

// ── Conversions ──────────────────────────────────────────────────────────────

impl From<NationRow> for Nation {
    fn from(r: NationRow) -> Self {
        Nation {
            uid: r.uid as i32,
            cnum: r.cnum as u8,
            status: nat_status(r.status),
            flags: NatFlags::from_bits_truncate(r.flags as u32),
            name: r.name,
            representative: r.representative,
            host_addr: r.host_addr,
            user_id: r.user_id,
            xcap: r.xcap as Coord, ycap: r.ycap as Coord,
            xorg: r.xorg as Coord, yorg: r.yorg as Coord,
            money: r.money as i32, reserve: r.reserve as i32,
            tech: r.tech, research: r.research,
            education: r.education, happiness: r.happiness,
            login_count: r.login_count as i32,
            tele_cnt: r.tele_cnt as i32,
        }
    }
}

impl From<RealmRow> for Realm {
    fn from(r: RealmRow) -> Self {
        Realm {
            uid: r.uid as i32, cnum: r.cnum as u8, realm: r.realm as u16,
            xl: r.xl as Coord, xh: r.xh as Coord,
            yl: r.yl as Coord, yh: r.yh as Coord,
        }
    }
}

fn nat_status(v: i64) -> NatStatus {
    match v {
        1 => NatStatus::New,    2 => NatStatus::Visitor,
        3 => NatStatus::Sanct,  4 => NatStatus::Active,
        5 => NatStatus::Deity,  _ => NatStatus::Unused,
    }
}

// ── Reads ────────────────────────────────────────────────────────────────────

pub async fn get(db: &Db, uid: i32) -> DbResult<Option<Nation>> {
    Ok(sqlx::query_as::<_, NationRow>("SELECT * FROM nations WHERE uid = ?")
        .bind(uid).fetch_optional(db.pool()).await?.map(Nation::from))
}

pub async fn get_by_cnum(db: &Db, cnum: u8) -> DbResult<Option<Nation>> {
    get(db, cnum as i32).await
}

pub async fn require(db: &Db, cnum: u8) -> DbResult<Nation> {
    get_by_cnum(db, cnum).await?
        .ok_or_else(|| DbError::NotFound(format!("nation {cnum}")))
}

pub async fn get_all(db: &Db) -> DbResult<Vec<Nation>> {
    Ok(sqlx::query_as::<_, NationRow>("SELECT * FROM nations ORDER BY uid")
        .fetch_all(db.pool()).await?.into_iter().map(Nation::from).collect())
}

pub async fn get_active(db: &Db) -> DbResult<Vec<Nation>> {
    Ok(sqlx::query_as::<_, NationRow>(
        "SELECT * FROM nations WHERE status >= ? ORDER BY uid",
    )
    .bind(NatStatus::Active as i64)
    .fetch_all(db.pool()).await?.into_iter().map(Nation::from).collect())
}

pub async fn get_realms(db: &Db, cnum: u8) -> DbResult<Vec<Realm>> {
    Ok(sqlx::query_as::<_, RealmRow>(
        "SELECT * FROM realms WHERE cnum = ? ORDER BY realm",
    )
    .bind(cnum as i64)
    .fetch_all(db.pool()).await?.into_iter().map(Realm::from).collect())
}

pub async fn count_active(db: &Db) -> DbResult<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM nations WHERE status >= ?",
    )
    .bind(NatStatus::Active as i64)
    .fetch_one(db.pool()).await?;
    Ok(row.0)
}

// ── Writes ───────────────────────────────────────────────────────────────────

pub async fn put(db: &Db, n: &Nation) -> DbResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO nations \
         (uid,cnum,status,flags,name,representative,host_addr,user_id,\
          xcap,ycap,xorg,yorg,money,reserve,tech,research,education,\
          happiness,login_count,tele_cnt,updated_at) \
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,strftime('%s','now'))",
    )
    .bind(n.uid).bind(n.cnum as i64).bind(n.status as i64)
    .bind(n.flags.bits() as i64)
    .bind(&n.name).bind(&n.representative).bind(&n.host_addr).bind(&n.user_id)
    .bind(n.xcap as i64).bind(n.ycap as i64)
    .bind(n.xorg as i64).bind(n.yorg as i64)
    .bind(n.money as i64).bind(n.reserve as i64)
    .bind(n.tech).bind(n.research).bind(n.education).bind(n.happiness)
    .bind(n.login_count as i64).bind(n.tele_cnt as i64)
    .execute(db.pool()).await?;
    Ok(())
}

pub async fn put_realm(db: &Db, r: &Realm) -> DbResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO realms (uid,cnum,realm,xl,xh,yl,yh) VALUES(?,?,?,?,?,?,?)",
    )
    .bind(r.uid).bind(r.cnum as i64).bind(r.realm as i64)
    .bind(r.xl as i64).bind(r.xh as i64).bind(r.yl as i64).bind(r.yh as i64)
    .execute(db.pool()).await?;
    Ok(())
}

/// Reset a nation slot to Unused without removing the row.
pub async fn clear(db: &Db, cnum: u8) -> DbResult<()> {
    sqlx::query("UPDATE nations SET status=0 WHERE cnum=?")
        .bind(cnum as i64).execute(db.pool()).await?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;

    #[tokio::test]
    async fn put_and_get_round_trips() {
        let db = test_db().await;
        let n = Nation {
            uid: 1, cnum: 1, status: NatStatus::Active,
            flags: NatFlags::empty(),
            name: "Testland".into(), representative: "Bob".into(),
            host_addr: "127.0.0.1".into(), user_id: "bob".into(),
            xcap: 4, ycap: 2, xorg: 0, yorg: 0,
            money: 20_000, reserve: 0,
            tech: 10.5, research: 3.2, education: 1.0, happiness: 50.0,
            login_count: 1, tele_cnt: 0,
        };
        put(&db, &n).await.unwrap();
        let got = get(&db, 1).await.unwrap().unwrap();
        assert_eq!(got.name, "Testland");
        assert_eq!(got.money, 20_000);
        assert_eq!(got.tech, 10.5);
    }

    #[tokio::test]
    async fn missing_returns_none() {
        let db = test_db().await;
        assert!(get(&db, 42).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn get_active_filters() {
        let db = test_db().await;
        let mut n = Nation { uid: 1, cnum: 1, status: NatStatus::Active, money: 1,
            flags: NatFlags::empty(), name: "A".into(), representative: "".into(),
            host_addr: "".into(), user_id: "".into(),
            xcap: 0, ycap: 0, xorg: 0, yorg: 0, reserve: 0,
            tech: 0.0, research: 0.0, education: 0.0, happiness: 0.0,
            login_count: 0, tele_cnt: 0 };
        put(&db, &n).await.unwrap();
        n.uid = 2; n.cnum = 2; n.status = NatStatus::Unused;
        put(&db, &n).await.unwrap();
        let active = get_active(&db).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].cnum, 1);
    }
}
