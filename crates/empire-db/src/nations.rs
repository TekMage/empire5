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
// ref: struct natstr / nat.h, natbyname/natpass from src/lib/player/nat.c,
//      ef_read/ef_write pattern from src/lib/common/file.c

use sqlx::FromRow;
use empire_types::nation::{Nation, NatFlags, NatStatus, Realm};
use empire_types::coords::Coord;
use crate::{Db, DbError, DbResult};

// ── Raw DB row ────────────────────────────────────────────────────────────────

#[derive(FromRow)]
struct NationRow {
    uid: i64, cnum: i64, status: i64, flags: i64,
    name: String, representative: String, host_addr: String, user_id: String,
    xcap: i64, ycap: i64, xorg: i64, yorg: i64,
    money: i64, reserve: i64,
    tech: f64, research: f64, education: f64, happiness: f64,
    login_count: i64, tele_cnt: i64, ann_cnt: i64, last_ann_read: i64,
    passwd_hash: String, last_login: i64, last_logout: i64,
    news_time: i64,
}

#[derive(FromRow)]
struct RealmRow {
    uid: i64, cnum: i64, realm: i64,
    xl: i64, xh: i64, yl: i64, yh: i64,
}

// ── Conversions ───────────────────────────────────────────────────────────────

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
            ann_cnt: r.ann_cnt as i32,
            last_ann_read: r.last_ann_read,
            passwd_hash: r.passwd_hash,
            last_login: r.last_login,
            last_logout: r.last_logout,
            news_time: r.news_time,
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

// ── Reads ─────────────────────────────────────────────────────────────────────

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

/// Find a nation by its country name (case-sensitive, ignores Unused slots).
/// Mirrors natbyname() in src/lib/player/nat.c.
pub async fn natbyname(db: &Db, name: &str) -> DbResult<Option<Nation>> {
    Ok(sqlx::query_as::<_, NationRow>(
        "SELECT * FROM nations WHERE name = ? AND status != 0 LIMIT 1",
    )
    .bind(name)
    .fetch_optional(db.pool()).await?.map(Nation::from))
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

// ── Writes ────────────────────────────────────────────────────────────────────

pub async fn put(db: &Db, n: &Nation) -> DbResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO nations \
         (uid,cnum,status,flags,name,representative,host_addr,user_id,\
          xcap,ycap,xorg,yorg,money,reserve,tech,research,education,\
          happiness,login_count,tele_cnt,ann_cnt,last_ann_read,passwd_hash,last_login,last_logout,\
          news_time,updated_at) \
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,strftime('%s','now'))",
    )
    .bind(n.uid).bind(n.cnum as i64).bind(n.status as i64)
    .bind(n.flags.bits() as i64)
    .bind(&n.name).bind(&n.representative).bind(&n.host_addr).bind(&n.user_id)
    .bind(n.xcap as i64).bind(n.ycap as i64)
    .bind(n.xorg as i64).bind(n.yorg as i64)
    .bind(n.money as i64).bind(n.reserve as i64)
    .bind(n.tech).bind(n.research).bind(n.education).bind(n.happiness)
    .bind(n.login_count as i64).bind(n.tele_cnt as i64)
    .bind(n.ann_cnt as i64).bind(n.last_ann_read)
    .bind(&n.passwd_hash).bind(n.last_login).bind(n.last_logout)
    .bind(n.news_time)
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

// ── Authentication ────────────────────────────────────────────────────────────

/// Verify a login password for the given country number.
///
/// Mirrors natpass() in src/lib/player/nat.c:
/// - Visitor nations (STAT_VIS) always pass regardless of password.
/// - If no hash is stored yet, accept an empty password (allows fresh installs).
/// - Otherwise bcrypt-verify the supplied plaintext against the stored hash.
pub async fn verify_passwd(db: &Db, cnum: u8, password: &str) -> DbResult<bool> {
    let n = match get_by_cnum(db, cnum).await? {
        Some(n) => n,
        None => return Ok(false),
    };
    if n.status == NatStatus::Visitor {
        return Ok(true);
    }
    // Empty hash means no password has been set yet — accept any password
    // (mirrors original Empire 4.x nat.c behaviour: empty nat_cdes accepts all).
    if n.passwd_hash.is_empty() {
        return Ok(true);
    }
    Ok(bcrypt::verify(password, &n.passwd_hash).unwrap_or(false))
}

/// Hash and store a new password for the given country.
pub async fn set_passwd(db: &Db, cnum: u8, password: &str) -> DbResult<()> {
    let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| DbError::NotFound(format!("bcrypt error: {e}")))?;
    sqlx::query("UPDATE nations SET passwd_hash=? WHERE cnum=?")
        .bind(&hash).bind(cnum as i64)
        .execute(db.pool()).await?;
    Ok(())
}

/// Record a successful login: update host_addr, user_id, last_login, login_count.
pub async fn record_login(db: &Db, cnum: u8, host_addr: &str, user_id: &str, now: i64)
    -> DbResult<()>
{
    sqlx::query(
        "UPDATE nations SET host_addr=?, user_id=?, last_login=?, \
         login_count=login_count+1 WHERE cnum=?",
    )
    .bind(host_addr).bind(user_id).bind(now).bind(cnum as i64)
    .execute(db.pool()).await?;
    Ok(())
}

/// Record logout timestamp.
pub async fn record_logout(db: &Db, cnum: u8, now: i64) -> DbResult<()> {
    sqlx::query("UPDATE nations SET last_logout=? WHERE cnum=?")
        .bind(now).bind(cnum as i64)
        .execute(db.pool()).await?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;

    fn make_nation(uid: i32, cnum: u8, status: NatStatus, name: &str) -> Nation {
        Nation {
            uid, cnum, status,
            flags: NatFlags::empty(),
            name: name.into(), representative: "".into(),
            host_addr: "".into(), user_id: "".into(),
            xcap: 0, ycap: 0, xorg: 0, yorg: 0,
            money: 0, reserve: 0,
            tech: 0.0, research: 0.0, education: 0.0, happiness: 0.0,
            login_count: 0, tele_cnt: 0, ann_cnt: 0, last_ann_read: 0,
            passwd_hash: "".into(), last_login: 0, last_logout: 0, news_time: 0,
        }
    }

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
            login_count: 1, tele_cnt: 0, ann_cnt: 0, last_ann_read: 0,
            passwd_hash: "".into(), last_login: 0, last_logout: 0, news_time: 0,
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
        put(&db, &make_nation(1, 1, NatStatus::Active, "A")).await.unwrap();
        put(&db, &make_nation(2, 2, NatStatus::Unused, "B")).await.unwrap();
        let active = get_active(&db).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].cnum, 1);
    }

    #[tokio::test]
    async fn natbyname_finds_existing() {
        let db = test_db().await;
        put(&db, &make_nation(1, 1, NatStatus::Active, "Wolfpack")).await.unwrap();
        let found = natbyname(&db, "Wolfpack").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().cnum, 1);
    }

    #[tokio::test]
    async fn natbyname_misses_unused() {
        let db = test_db().await;
        put(&db, &make_nation(1, 1, NatStatus::Unused, "Ghost")).await.unwrap();
        assert!(natbyname(&db, "Ghost").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn verify_passwd_empty_hash_accepts_any_password() {
        // Empty hash means "no password set" — any password (including blank) is accepted.
        let db = test_db().await;
        put(&db, &make_nation(1, 1, NatStatus::Active, "X")).await.unwrap();
        assert!(verify_passwd(&db, 1, "").await.unwrap());
        assert!(verify_passwd(&db, 1, "anything").await.unwrap());
        assert!(verify_passwd(&db, 1, "tekmage").await.unwrap());
    }

    #[tokio::test]
    async fn set_and_verify_passwd() {
        let db = test_db().await;
        put(&db, &make_nation(1, 1, NatStatus::Active, "X")).await.unwrap();
        set_passwd(&db, 1, "secret").await.unwrap();
        assert!(verify_passwd(&db, 1, "secret").await.unwrap());
        assert!(!verify_passwd(&db, 1, "wrong").await.unwrap());
    }

    #[tokio::test]
    async fn visitor_bypasses_passwd() {
        let db = test_db().await;
        put(&db, &make_nation(1, 1, NatStatus::Visitor, "Visitor")).await.unwrap();
        assert!(verify_passwd(&db, 1, "anything").await.unwrap());
        assert!(verify_passwd(&db, 1, "").await.unwrap());
    }
}
