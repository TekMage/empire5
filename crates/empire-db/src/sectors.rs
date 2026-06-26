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
// Ported from: include/sect.h
// Known contributors to the original:
//    Dave Pare
//    Ken Stevens, 1995
//    Steve McClure, 1998

// DB accessors for the sectors table.
// ref: struct sctstr / sect.h

use sqlx::FromRow;
use empire_types::sector::{Sector, SectorType, DistEntry};
use empire_types::commodity::Inventory;
use empire_types::coords::{Coord, NatId, Range};
use crate::{Db, DbError, DbResult};

// ── Raw DB row ───────────────────────────────────────────────────────────────

#[derive(FromRow)]
struct SectorRow {
    uid: i64, own: i64, x: i64, y: i64,
    sector_type: i64, effic: i64, mobil: i64, off: i64,
    loyal: i64, terr0: i64, terr1: i64, terr2: i64, terr3: i64, dterr: i64,
    dist_x: i64, dist_y: i64, avail: i64, flags: i64, elev: i64,
    work: i64, coastal: i64, new_type: i64,
    min_ore: i64, gmin: i64, fertil: i64, oil: i64, uran: i64,
    old_own: i64, che: i64, che_target: i64,
    items: String,           // JSON [i16; 14]
    mines: i64, pstage: i64, ptime: i64, fallout: i64,
    thresholds_json: String, // JSON [i16; 14] — per-item distribution thresholds
}

// ── Conversions ──────────────────────────────────────────────────────────────

fn sect_type(v: i64) -> SectorType {
    match v {
        -1 => SectorType::Sea,       0 => SectorType::Land,
        1  => SectorType::Mountain,  2 => SectorType::Agri,
        3  => SectorType::Uranium,   4 => SectorType::Plain,
        5  => SectorType::Park,      6 => SectorType::Urban,
        7  => SectorType::Research,  8 => SectorType::Wasteland,
        9  => SectorType::Defense,   10 => SectorType::Bank,
        11 => SectorType::Engineer,  12 => SectorType::Airfield,
        13 => SectorType::Highway,   14 => SectorType::Radar,
        15 => SectorType::Naval,     16 => SectorType::Missile,
        17 => SectorType::Harbor,    18 => SectorType::Fort,
        19 => SectorType::Tech,      20 => SectorType::Bravery,
        21 => SectorType::LightIndus, 22 => SectorType::HeavyIndus,
        23 => SectorType::Gold,      24 => SectorType::Oil,
        25 => SectorType::Unknown,   26 => SectorType::Warehouse,
        _  => SectorType::Unknown,
    }
}

fn items_from_json(s: &str) -> Inventory {
    let vals: Vec<i16> = serde_json::from_str(s).unwrap_or_default();
    let mut inv = Inventory::zero();
    for (i, v) in vals.iter().enumerate().take(14) {
        inv.0[i] = *v;
    }
    inv
}

fn items_to_json(inv: &Inventory) -> String {
    serde_json::to_string(&inv.0.as_slice()).unwrap_or_else(|_| "[]".to_string())
}

fn thresholds_from_json(s: &str) -> [i16; 14] {
    let vals: Vec<i16> = serde_json::from_str(s).unwrap_or_default();
    let mut out = [0i16; 14];
    for (i, v) in vals.iter().enumerate().take(14) {
        out[i] = *v;
    }
    out
}

fn thresholds_to_json(del: &[DistEntry; 26]) -> String {
    let vals: Vec<i16> = (0..14).map(|i| del[i].threshold).collect();
    serde_json::to_string(&vals).unwrap_or_else(|_| "[]".to_string())
}

impl From<SectorRow> for Sector {
    fn from(r: SectorRow) -> Self {
        Sector {
            uid: r.uid as i32,
            own: r.own as NatId,
            x: r.x as Coord, y: r.y as Coord,
            sector_type: sect_type(r.sector_type),
            effic: r.effic as i8, mobil: r.mobil as i8,
            off: r.off != 0,
            loyal: r.loyal as u8,
            terr: [r.terr0 as u8, r.terr1 as u8, r.terr2 as u8, r.terr3 as u8],
            dterr: r.dterr as u8,
            dist_x: r.dist_x as Coord, dist_y: r.dist_y as Coord,
            avail: r.avail as i16, flags: r.flags as i16,
            elev: r.elev as i16,
            work: r.work as u8, coastal: r.coastal != 0,
            new_type: sect_type(r.new_type),
            min: r.min_ore as u8, gmin: r.gmin as u8,
            fertil: r.fertil as u8, oil: r.oil as u8, uran: r.uran as u8,
            old_own: r.old_own as NatId,
            che: r.che as u8, che_target: r.che_target as NatId,
            items: items_from_json(&r.items),
            del: {
                let thresholds = thresholds_from_json(&r.thresholds_json);
                let mut del = [DistEntry::default(); 26];
                for (i, t) in thresholds.iter().enumerate() {
                    del[i].threshold = *t;
                }
                del
            },
            mines: r.mines as i16, pstage: r.pstage as i16,
            ptime: r.ptime as i16, fallout: r.fallout as i32,
        }
    }
}

// ── Reads ─────────────────────────────────────────────────────────────────────

pub async fn get(db: &Db, uid: i32) -> DbResult<Option<Sector>> {
    Ok(sqlx::query_as::<_, SectorRow>("SELECT * FROM sectors WHERE uid = ?")
        .bind(uid).fetch_optional(db.pool()).await?.map(Sector::from))
}

pub async fn get_at(db: &Db, x: Coord, y: Coord) -> DbResult<Option<Sector>> {
    Ok(sqlx::query_as::<_, SectorRow>("SELECT * FROM sectors WHERE x=? AND y=?")
        .bind(x as i64).bind(y as i64)
        .fetch_optional(db.pool()).await?.map(Sector::from))
}

pub async fn get_all(db: &Db) -> DbResult<Vec<Sector>> {
    Ok(sqlx::query_as::<_, SectorRow>("SELECT * FROM sectors ORDER BY uid")
        .fetch_all(db.pool()).await?.into_iter().map(Sector::from).collect())
}

pub async fn get_by_owner(db: &Db, own: NatId) -> DbResult<Vec<Sector>> {
    Ok(sqlx::query_as::<_, SectorRow>("SELECT * FROM sectors WHERE own=? ORDER BY uid")
        .bind(own as i64)
        .fetch_all(db.pool()).await?.into_iter().map(Sector::from).collect())
}

pub async fn get_in_range(db: &Db, r: &Range) -> DbResult<Vec<Sector>> {
    Ok(sqlx::query_as::<_, SectorRow>(
        "SELECT * FROM sectors WHERE x>=? AND x<=? AND y>=? AND y<=? ORDER BY uid",
    )
    .bind(r.lx as i64).bind(r.hx as i64)
    .bind(r.ly as i64).bind(r.hy as i64)
    .fetch_all(db.pool()).await?.into_iter().map(Sector::from).collect())
}

pub async fn require(db: &Db, uid: i32) -> DbResult<Sector> {
    get(db, uid).await?
        .ok_or_else(|| DbError::NotFound(format!("sector uid={uid}")))
}

pub async fn count(db: &Db) -> DbResult<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sectors")
        .fetch_one(db.pool()).await?;
    Ok(row.0)
}

// ── Writes ─────────────────────────────────────────────────────────────────────

pub async fn put(db: &Db, s: &Sector) -> DbResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO sectors \
         (uid,own,x,y,sector_type,effic,mobil,off,loyal,\
          terr0,terr1,terr2,terr3,dterr,dist_x,dist_y,avail,flags,elev,\
          work,coastal,new_type,min_ore,gmin,fertil,oil,uran,old_own,che,che_target,\
          items,mines,pstage,ptime,fallout,thresholds_json,updated_at) \
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,strftime('%s','now'))",
    )
    .bind(s.uid).bind(s.own as i64)
    .bind(s.x as i64).bind(s.y as i64)
    .bind(s.sector_type as i64).bind(s.effic as i64).bind(s.mobil as i64)
    .bind(s.off as i64).bind(s.loyal as i64)
    .bind(s.terr[0] as i64).bind(s.terr[1] as i64)
    .bind(s.terr[2] as i64).bind(s.terr[3] as i64)
    .bind(s.dterr as i64)
    .bind(s.dist_x as i64).bind(s.dist_y as i64)
    .bind(s.avail as i64).bind(s.flags as i64).bind(s.elev as i64)
    .bind(s.work as i64).bind(s.coastal as i64)
    .bind(s.new_type as i64)
    .bind(s.min as i64).bind(s.gmin as i64).bind(s.fertil as i64)
    .bind(s.oil as i64).bind(s.uran as i64).bind(s.old_own as i64)
    .bind(s.che as i64).bind(s.che_target as i64)
    .bind(items_to_json(&s.items))
    .bind(s.mines as i64).bind(s.pstage as i64)
    .bind(s.ptime as i64).bind(s.fallout as i64)
    .bind(thresholds_to_json(&s.del))
    .execute(db.pool()).await?;
    Ok(())
}

/// Bulk-insert a freshly-generated world map.
pub async fn put_many(db: &Db, sectors: &[Sector]) -> DbResult<()> {
    let mut tx = db.pool().begin().await?;
    for s in sectors {
        sqlx::query(
            "INSERT OR REPLACE INTO sectors \
             (uid,own,x,y,sector_type,effic,mobil,off,loyal,\
              terr0,terr1,terr2,terr3,dterr,dist_x,dist_y,avail,flags,elev,\
              work,coastal,new_type,min_ore,gmin,fertil,oil,uran,old_own,che,che_target,\
              items,mines,pstage,ptime,fallout,thresholds_json,updated_at) \
             VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,strftime('%s','now'))",
        )
        .bind(s.uid).bind(s.own as i64)
        .bind(s.x as i64).bind(s.y as i64)
        .bind(s.sector_type as i64).bind(s.effic as i64).bind(s.mobil as i64)
        .bind(s.off as i64).bind(s.loyal as i64)
        .bind(s.terr[0] as i64).bind(s.terr[1] as i64)
        .bind(s.terr[2] as i64).bind(s.terr[3] as i64)
        .bind(s.dterr as i64)
        .bind(s.dist_x as i64).bind(s.dist_y as i64)
        .bind(s.avail as i64).bind(s.flags as i64).bind(s.elev as i64)
        .bind(s.work as i64).bind(s.coastal as i64)
        .bind(s.new_type as i64)
        .bind(s.min as i64).bind(s.gmin as i64).bind(s.fertil as i64)
        .bind(s.oil as i64).bind(s.uran as i64).bind(s.old_own as i64)
        .bind(s.che as i64).bind(s.che_target as i64)
        .bind(items_to_json(&s.items))
        .bind(s.mines as i64).bind(s.pstage as i64)
        .bind(s.ptime as i64).bind(s.fallout as i64)
        .bind(thresholds_to_json(&s.del))
        .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;
    use empire_types::commodity::Item;

    fn make_sector(uid: i32, x: i16, y: i16, own: u8) -> Sector {
        let mut s = Sector {
            uid, own, x, y,
            sector_type: SectorType::Urban, effic: 100, mobil: 127,
            off: false, loyal: 0, terr: [0;4], dterr: 0,
            dist_x: 0, dist_y: 0, avail: 50, flags: 0, elev: 0,
            work: 100, coastal: false, new_type: SectorType::Urban,
            min: 0, gmin: 0, fertil: 80, oil: 0, uran: 0, old_own: 0,
            che: 0, che_target: 0,
            items: Inventory::zero(), del: [DistEntry::default();26],
            mines: 0, pstage: 0, ptime: 0, fallout: 0,
        };
        s.items.set(Item::Civil, 500);
        s
    }

    #[tokio::test]
    async fn round_trip_sector() {
        let db = test_db().await;
        let s = make_sector(0, 0, 0, 1);
        put(&db, &s).await.unwrap();
        let got = get(&db, 0).await.unwrap().unwrap();
        assert_eq!(got.own, 1);
        assert_eq!(got.effic, 100);
        assert_eq!(got.items.get(Item::Civil), 500);
    }

    #[tokio::test]
    async fn get_at_coord() {
        let db = test_db().await;
        put(&db, &make_sector(0, 0, 0, 1)).await.unwrap();
        put(&db, &make_sector(1, 2, 0, 2)).await.unwrap();
        let s = get_at(&db, 2, 0).await.unwrap().unwrap();
        assert_eq!(s.own, 2);
    }
}
