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
// Ported from: include/plane.h
// Known contributors to the original:
//    Dave Pare, 1986
//    Ken Stevens, 1995
//    Steve McClure, 1998

// DB accessors for the planes table.
// ref: struct plnstr / plane.h

use sqlx::FromRow;
use empire_types::plane::{Plane, PlaneFlags};
use empire_types::coords::{Coord, NatId};
use crate::{Db, DbError, DbResult};

#[derive(FromRow)]
struct PlaneRow {
    uid: i64, own: i64, x: i64, y: i64,
    plane_type: i64, effic: i64, mobil: i64, off: i64, tech: i64,
    wing: String,
    opx: i64, opy: i64, mission: i64, mission_radius: i64,
    range: i64, harden: i64,
    ship: i64, land: i64,
    flags: i64, access: i64, theta: f64,
}

impl From<PlaneRow> for Plane {
    fn from(r: PlaneRow) -> Self {
        Plane {
            uid: r.uid as i32, own: r.own as NatId,
            x: r.x as Coord, y: r.y as Coord,
            plane_type: r.plane_type as i8, effic: r.effic as i8,
            mobil: r.mobil as i8, off: r.off != 0, tech: r.tech as i16,
            wing: r.wing.chars().next().unwrap_or(' '),
            opx: r.opx as Coord, opy: r.opy as Coord,
            mission: r.mission as i16, mission_radius: r.mission_radius as i16,
            range: r.range as u8, harden: r.harden as i8,
            ship: r.ship as i32, land: r.land as i32,
            flags: PlaneFlags::from_bits_truncate(r.flags as u32),
            access: r.access as i16, theta: r.theta as f32,
        }
    }
}

pub async fn get(db: &Db, uid: i32) -> DbResult<Option<Plane>> {
    Ok(sqlx::query_as::<_, PlaneRow>("SELECT * FROM planes WHERE uid=?")
        .bind(uid).fetch_optional(db.pool()).await?.map(Plane::from))
}
pub async fn require(db: &Db, uid: i32) -> DbResult<Plane> {
    get(db, uid).await?.ok_or_else(|| DbError::NotFound(format!("plane {uid}")))
}
pub async fn get_all(db: &Db) -> DbResult<Vec<Plane>> {
    Ok(sqlx::query_as::<_, PlaneRow>("SELECT * FROM planes ORDER BY uid")
        .fetch_all(db.pool()).await?.into_iter().map(Plane::from).collect())
}
pub async fn get_by_owner(db: &Db, own: NatId) -> DbResult<Vec<Plane>> {
    Ok(sqlx::query_as::<_, PlaneRow>("SELECT * FROM planes WHERE own=? ORDER BY uid")
        .bind(own as i64)
        .fetch_all(db.pool()).await?.into_iter().map(Plane::from).collect())
}
pub async fn get_by_wing(db: &Db, own: NatId, wing: char) -> DbResult<Vec<Plane>> {
    Ok(sqlx::query_as::<_, PlaneRow>(
        "SELECT * FROM planes WHERE own=? AND wing=? ORDER BY uid",
    )
    .bind(own as i64).bind(wing.to_string())
    .fetch_all(db.pool()).await?.into_iter().map(Plane::from).collect())
}
pub async fn get_on_ship(db: &Db, ship_uid: i32) -> DbResult<Vec<Plane>> {
    Ok(sqlx::query_as::<_, PlaneRow>(
        "SELECT * FROM planes WHERE ship=? ORDER BY uid",
    )
    .bind(ship_uid as i64)
    .fetch_all(db.pool()).await?.into_iter().map(Plane::from).collect())
}

pub async fn get_at_xy(db: &Db, x: Coord, y: Coord) -> DbResult<Vec<Plane>> {
    Ok(sqlx::query_as::<_, PlaneRow>(
        "SELECT * FROM planes WHERE x=? AND y=? ORDER BY uid",
    )
    .bind(x as i64).bind(y as i64)
    .fetch_all(db.pool()).await?.into_iter().map(Plane::from).collect())
}

pub async fn put(db: &Db, p: &Plane) -> DbResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO planes \
         (uid,own,x,y,plane_type,effic,mobil,off,tech,wing,opx,opy,\
          mission,mission_radius,range,harden,ship,land,flags,access,theta,updated_at) \
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,strftime('%s','now'))",
    )
    .bind(p.uid).bind(p.own as i64).bind(p.x as i64).bind(p.y as i64)
    .bind(p.plane_type as i64).bind(p.effic as i64).bind(p.mobil as i64)
    .bind(p.off as i64).bind(p.tech as i64).bind(p.wing.to_string())
    .bind(p.opx as i64).bind(p.opy as i64)
    .bind(p.mission as i64).bind(p.mission_radius as i64)
    .bind(p.range as i64).bind(p.harden as i64)
    .bind(p.ship as i64).bind(p.land as i64)
    .bind(p.flags.bits() as i64).bind(p.access as i64).bind(p.theta as f64)
    .execute(db.pool()).await?;
    Ok(())
}

pub async fn delete(db: &Db, uid: i32) -> DbResult<()> {
    sqlx::query("DELETE FROM planes WHERE uid=?")
        .bind(uid).execute(db.pool()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;

    fn make_plane(uid: i32, own: u8) -> Plane {
        Plane {
            uid, own, x: 0, y: 0, plane_type: 0, effic: 100, mobil: 60,
            off: false, tech: 100, wing: ' ',
            opx: 0, opy: 0, mission: 0, mission_radius: 0,
            range: 8, harden: 0, ship: -1, land: -1,
            flags: PlaneFlags::empty(), access: 0, theta: 0.0,
        }
    }

    #[tokio::test]
    async fn plane_round_trip() {
        let db = test_db().await;
        let p = make_plane(0, 1);
        put(&db, &p).await.unwrap();
        let got = get(&db, 0).await.unwrap().unwrap();
        assert_eq!(got.own, 1);
        assert_eq!(got.range, 8);
    }

    #[tokio::test]
    async fn get_on_ship_filters() {
        let db = test_db().await;
        let mut p = make_plane(0, 1);
        p.ship = 42;
        put(&db, &p).await.unwrap();
        put(&db, &make_plane(1, 1)).await.unwrap();
        assert_eq!(get_on_ship(&db, 42).await.unwrap().len(), 1);
        assert_eq!(get_on_ship(&db, 99).await.unwrap().len(), 0);
    }
}
