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
// Ported from: include/land.h
// Known contributors to the original:
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995
//    Steve McClure, 1998

// DB accessors for the land_units table.
// ref: struct lndstr / land.h

use sqlx::FromRow;
use empire_types::land::LandUnit;
use empire_types::ship::RetreatFlags;
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
struct LandRow {
    uid: i64, own: i64, x: i64, y: i64,
    land_type: i64, effic: i64, mobil: i64, off: i64, tech: i64,
    army: String,
    opx: i64, opy: i64, mission: i64, mission_radius: i64,
    ship: i64, harden: i64, retreat: i64,
    retreat_flags: i64, retreat_path: String,
    scar: i64, items: String,
    pstage: i64, ptime: i64, carried_by_land: i64, access: i64,
}

impl From<LandRow> for LandUnit {
    fn from(r: LandRow) -> Self {
        LandUnit {
            uid: r.uid as i32, own: r.own as NatId,
            x: r.x as Coord, y: r.y as Coord,
            land_type: r.land_type as i8, effic: r.effic as i8,
            mobil: r.mobil as i8, off: r.off != 0, tech: r.tech as i16,
            army: r.army.chars().next().unwrap_or(' '),
            opx: r.opx as Coord, opy: r.opy as Coord,
            mission: r.mission as i16, mission_radius: r.mission_radius as i16,
            ship: r.ship as i32, harden: r.harden as i8,
            retreat: r.retreat as i16,
            retreat_flags: RetreatFlags::from_bits_truncate(r.retreat_flags as u32),
            retreat_path: r.retreat_path,
            scar: r.scar as u8,
            items: items_from_json(&r.items),
            pstage: r.pstage as i16, ptime: r.ptime as i16,
            carried_by_land: r.carried_by_land as i32,
            access: r.access as i16,
        }
    }
}

pub async fn get(db: &Db, uid: i32) -> DbResult<Option<LandUnit>> {
    Ok(sqlx::query_as::<_, LandRow>("SELECT * FROM land_units WHERE uid=?")
        .bind(uid).fetch_optional(db.pool()).await?.map(LandUnit::from))
}
pub async fn require(db: &Db, uid: i32) -> DbResult<LandUnit> {
    get(db, uid).await?.ok_or_else(|| DbError::NotFound(format!("land unit {uid}")))
}
pub async fn get_all(db: &Db) -> DbResult<Vec<LandUnit>> {
    Ok(sqlx::query_as::<_, LandRow>("SELECT * FROM land_units ORDER BY uid")
        .fetch_all(db.pool()).await?.into_iter().map(LandUnit::from).collect())
}
pub async fn get_by_owner(db: &Db, own: NatId) -> DbResult<Vec<LandUnit>> {
    Ok(sqlx::query_as::<_, LandRow>("SELECT * FROM land_units WHERE own=? ORDER BY uid")
        .bind(own as i64)
        .fetch_all(db.pool()).await?.into_iter().map(LandUnit::from).collect())
}
pub async fn get_by_army(db: &Db, own: NatId, army: char) -> DbResult<Vec<LandUnit>> {
    Ok(sqlx::query_as::<_, LandRow>(
        "SELECT * FROM land_units WHERE own=? AND army=? ORDER BY uid",
    )
    .bind(own as i64).bind(army.to_string())
    .fetch_all(db.pool()).await?.into_iter().map(LandUnit::from).collect())
}
pub async fn get_on_ship(db: &Db, ship_uid: i32) -> DbResult<Vec<LandUnit>> {
    Ok(sqlx::query_as::<_, LandRow>(
        "SELECT * FROM land_units WHERE ship=? ORDER BY uid",
    )
    .bind(ship_uid as i64)
    .fetch_all(db.pool()).await?.into_iter().map(LandUnit::from).collect())
}

pub async fn get_at_xy(db: &Db, x: Coord, y: Coord) -> DbResult<Vec<LandUnit>> {
    Ok(sqlx::query_as::<_, LandRow>(
        "SELECT * FROM land_units WHERE x=? AND y=? ORDER BY uid",
    )
    .bind(x as i64).bind(y as i64)
    .fetch_all(db.pool()).await?.into_iter().map(LandUnit::from).collect())
}

pub async fn put(db: &Db, u: &LandUnit) -> DbResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO land_units \
         (uid,own,x,y,land_type,effic,mobil,off,tech,army,opx,opy,\
          mission,mission_radius,ship,harden,retreat,retreat_flags,retreat_path,\
          scar,items,pstage,ptime,carried_by_land,access,updated_at) \
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,strftime('%s','now'))",
    )
    .bind(u.uid).bind(u.own as i64).bind(u.x as i64).bind(u.y as i64)
    .bind(u.land_type as i64).bind(u.effic as i64).bind(u.mobil as i64)
    .bind(u.off as i64).bind(u.tech as i64).bind(u.army.to_string())
    .bind(u.opx as i64).bind(u.opy as i64)
    .bind(u.mission as i64).bind(u.mission_radius as i64)
    .bind(u.ship as i64).bind(u.harden as i64).bind(u.retreat as i64)
    .bind(u.retreat_flags.bits() as i64).bind(&u.retreat_path)
    .bind(u.scar as i64).bind(items_to_json(&u.items))
    .bind(u.pstage as i64).bind(u.ptime as i64)
    .bind(u.carried_by_land as i64).bind(u.access as i64)
    .execute(db.pool()).await?;
    Ok(())
}

pub async fn delete(db: &Db, uid: i32) -> DbResult<()> {
    sqlx::query("DELETE FROM land_units WHERE uid=?")
        .bind(uid).execute(db.pool()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;

    fn make_unit(uid: i32, own: u8) -> LandUnit {
        LandUnit {
            uid, own, x: 0, y: 0, land_type: 0, effic: 100, mobil: 60,
            off: false, tech: 50, army: ' ',
            opx: 0, opy: 0, mission: 0, mission_radius: 0,
            ship: -1, harden: 0, retreat: 50,
            retreat_flags: RetreatFlags::empty(), retreat_path: String::new(),
            scar: 0, items: Inventory::zero(),
            pstage: 0, ptime: 0, carried_by_land: -1, access: 0,
        }
    }

    #[tokio::test]
    async fn land_round_trip() {
        let db = test_db().await;
        let u = make_unit(0, 1);
        put(&db, &u).await.unwrap();
        let got = get(&db, 0).await.unwrap().unwrap();
        assert_eq!(got.own, 1);
        assert_eq!(got.effic, 100);
    }

    #[tokio::test]
    async fn get_by_owner_works() {
        let db = test_db().await;
        put(&db, &make_unit(0, 1)).await.unwrap();
        put(&db, &make_unit(1, 2)).await.unwrap();
        assert_eq!(get_by_owner(&db, 1).await.unwrap().len(), 1);
    }
}
