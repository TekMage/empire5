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
// Ported from: include/ship.h
// Known contributors to the original:
//    Dave Pare
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995

// DB accessors for the ships table.
// ref: struct shpstr / ship.h

use sqlx::FromRow;
use empire_types::ship::{Ship, RetreatFlags};
use empire_types::commodity::Inventory;
use empire_types::coords::{Coord, NatId};
use crate::{Db, DbError, DbResult};

fn items_from_json(s: &str) -> Inventory {
    let vals: Vec<i16> = serde_json::from_str(s).unwrap_or_default();
    let mut inv = Inventory::zero();
    for (i, v) in vals.iter().enumerate().take(14) { inv.0[i] = *v; }
    inv
}
fn items_to_json(inv: &Inventory) -> String {
    serde_json::to_string(&inv.0.as_slice()).unwrap_or_else(|_| "[]".to_string())
}

#[derive(FromRow)]
struct ShipRow {
    uid: i64, own: i64, x: i64, y: i64,
    ship_type: i64, effic: i64, mobil: i64, off: i64, tech: i64,
    fleet: String,
    opx: i64, opy: i64, mission: i64, mission_radius: i64,
    items: String,
    pstage: i64, ptime: i64, access: i64,
    name: String,
    orig_x: i64, orig_y: i64, orig_own: i64,
    retreat_flags: i64, retreat_path: String,
}

impl From<ShipRow> for Ship {
    fn from(r: ShipRow) -> Self {
        Ship {
            uid: r.uid as i32, own: r.own as NatId,
            x: r.x as Coord, y: r.y as Coord,
            ship_type: r.ship_type as i8, effic: r.effic as i8,
            mobil: r.mobil as i8, off: r.off != 0, tech: r.tech as i16,
            fleet: r.fleet.chars().next().unwrap_or(' '),
            opx: r.opx as Coord, opy: r.opy as Coord,
            mission: r.mission as i16, mission_radius: r.mission_radius as i16,
            items: items_from_json(&r.items),
            pstage: r.pstage as i16, ptime: r.ptime as i16,
            access: r.access as i16,
            name: r.name,
            orig_x: r.orig_x as Coord, orig_y: r.orig_y as Coord,
            orig_own: r.orig_own as NatId,
            retreat_flags: RetreatFlags::from_bits_truncate(r.retreat_flags as u32),
            retreat_path: r.retreat_path,
        }
    }
}

pub async fn get(db: &Db, uid: i32) -> DbResult<Option<Ship>> {
    Ok(sqlx::query_as::<_, ShipRow>("SELECT * FROM ships WHERE uid=?")
        .bind(uid).fetch_optional(db.pool()).await?.map(Ship::from))
}
pub async fn require(db: &Db, uid: i32) -> DbResult<Ship> {
    get(db, uid).await?.ok_or_else(|| DbError::NotFound(format!("ship {uid}")))
}
pub async fn get_all(db: &Db) -> DbResult<Vec<Ship>> {
    Ok(sqlx::query_as::<_, ShipRow>("SELECT * FROM ships ORDER BY uid")
        .fetch_all(db.pool()).await?.into_iter().map(Ship::from).collect())
}
pub async fn get_by_owner(db: &Db, own: NatId) -> DbResult<Vec<Ship>> {
    Ok(sqlx::query_as::<_, ShipRow>("SELECT * FROM ships WHERE own=? ORDER BY uid")
        .bind(own as i64)
        .fetch_all(db.pool()).await?.into_iter().map(Ship::from).collect())
}
pub async fn get_by_fleet(db: &Db, own: NatId, fleet: char) -> DbResult<Vec<Ship>> {
    Ok(sqlx::query_as::<_, ShipRow>(
        "SELECT * FROM ships WHERE own=? AND fleet=? ORDER BY uid",
    )
    .bind(own as i64).bind(fleet.to_string())
    .fetch_all(db.pool()).await?.into_iter().map(Ship::from).collect())
}

pub async fn get_at_xy(db: &Db, x: Coord, y: Coord) -> DbResult<Vec<Ship>> {
    Ok(sqlx::query_as::<_, ShipRow>(
        "SELECT * FROM ships WHERE x=? AND y=? ORDER BY uid",
    )
    .bind(x as i64).bind(y as i64)
    .fetch_all(db.pool()).await?.into_iter().map(Ship::from).collect())
}

pub async fn put(db: &Db, s: &Ship) -> DbResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO ships \
         (uid,own,x,y,ship_type,effic,mobil,off,tech,fleet,opx,opy,\
          mission,mission_radius,items,pstage,ptime,access,name,\
          orig_x,orig_y,orig_own,retreat_flags,retreat_path,updated_at) \
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,strftime('%s','now'))",
    )
    .bind(s.uid).bind(s.own as i64).bind(s.x as i64).bind(s.y as i64)
    .bind(s.ship_type as i64).bind(s.effic as i64).bind(s.mobil as i64)
    .bind(s.off as i64).bind(s.tech as i64).bind(s.fleet.to_string())
    .bind(s.opx as i64).bind(s.opy as i64)
    .bind(s.mission as i64).bind(s.mission_radius as i64)
    .bind(items_to_json(&s.items))
    .bind(s.pstage as i64).bind(s.ptime as i64).bind(s.access as i64)
    .bind(&s.name)
    .bind(s.orig_x as i64).bind(s.orig_y as i64).bind(s.orig_own as i64)
    .bind(s.retreat_flags.bits() as i64).bind(&s.retreat_path)
    .execute(db.pool()).await?;
    Ok(())
}

pub async fn delete(db: &Db, uid: i32) -> DbResult<()> {
    sqlx::query("DELETE FROM ships WHERE uid=?")
        .bind(uid).execute(db.pool()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;

    fn make_ship(uid: i32, own: u8) -> Ship {
        Ship {
            uid, own, x: 0, y: 0, ship_type: 0, effic: 100, mobil: 60,
            off: false, tech: 100, fleet: ' ',
            opx: 0, opy: 0, mission: 0, mission_radius: 0,
            items: Inventory::zero(), pstage: 0, ptime: 0, access: 0,
            name: "SS Test".into(),
            orig_x: 0, orig_y: 0, orig_own: own,
            retreat_flags: RetreatFlags::empty(), retreat_path: String::new(),
        }
    }

    #[tokio::test]
    async fn ship_round_trip() {
        let db = test_db().await;
        let s = make_ship(0, 1);
        put(&db, &s).await.unwrap();
        let got = get(&db, 0).await.unwrap().unwrap();
        assert_eq!(got.name, "SS Test");
        assert_eq!(got.own, 1);
    }

    #[tokio::test]
    async fn get_by_owner_filters() {
        let db = test_db().await;
        put(&db, &make_ship(0, 1)).await.unwrap();
        put(&db, &make_ship(1, 2)).await.unwrap();
        assert_eq!(get_by_owner(&db, 1).await.unwrap().len(), 1);
        assert_eq!(get_by_owner(&db, 2).await.unwrap().len(), 1);
    }
}
