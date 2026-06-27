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
// Ported from: src/lib/subs/nsc.c, src/lib/common/nsc.c
// Known contributors to the original:
//    Dave Pare, 1989
//    Markus Armbruster, 2004-2016

// Typed scan functions: fetch all records of a type, apply NSC conditions.
// Phase 1: fetch-all + in-memory filter.  Later phases can push conditions
// into SQL WHERE clauses for large world maps.
//
// ref: src/lib/subs/nsc.c — nstr_exec / nstr_match logic

use empire_types::selector::{Condition, ObjectType, ScanSpec, SelectArea};
use empire_types::{Nation, Sector, Ship, Plane, LandUnit, Nuke};
use crate::{Db, DbResult, nations, sectors, ships, planes, land_units, nukes};

// ── Trait for applying an NSC condition to a field value ─────────────────────

pub trait MatchField {
    /// Return the i64 value of `field`, or None if not an i64 field.
    fn field_i64(&self, field: &str) -> Option<i64>;
    /// Return the f64 value of `field`, or None if not an f64 field.
    fn field_f64(&self, field: &str) -> Option<f64>;
    /// Return the str value of `field`, or None if not a str field.
    fn field_str<'a>(&'a self, field: &str) -> Option<&'a str>;

    fn matches_cond(&self, c: &Condition) -> bool {
        if let Some(v) = self.field_i64(&c.field) { return c.matches_i64(v); }
        if let Some(v) = self.field_f64(&c.field) { return c.matches_f64(v); }
        if let Some(v) = self.field_str(&c.field) { return c.matches_str(v); }
        false // unknown field — condition does not match
    }

    fn matches_spec(&self, conditions: &[Condition]) -> bool {
        conditions.iter().all(|c| self.matches_cond(c))
    }
}

// ── MatchField impls ──────────────────────────────────────────────────────────

impl MatchField for Nation {
    fn field_i64(&self, f: &str) -> Option<i64> {
        Some(match f {
            "uid" | "cnum" => self.uid as i64,
            "status"  => self.status as i64,
            "flags"   => self.flags.bits() as i64,
            "xcap"    => self.xcap as i64,
            "ycap"    => self.ycap as i64,
            "money"   => self.money as i64,
            "reserve" => self.reserve as i64,
            "login_count" => self.login_count as i64,
            _ => return None,
        })
    }
    fn field_f64(&self, f: &str) -> Option<f64> {
        Some(match f {
            "tech"      => self.tech,
            "research"  => self.research,
            "education" => self.education,
            "happiness" => self.happiness,
            _ => return None,
        })
    }
    fn field_str<'a>(&'a self, f: &str) -> Option<&'a str> {
        Some(match f {
            "name"           => &self.name,
            "representative" => &self.representative,
            _ => return None,
        })
    }
}

impl MatchField for Sector {
    fn field_i64(&self, f: &str) -> Option<i64> {
        Some(match f {
            "uid"   => self.uid as i64,
            "own"   => self.own as i64,
            "x"     => self.x as i64,
            "y"     => self.y as i64,
            "type" | "stype" => self.sector_type as i64,
            "eff" | "effic"  => self.effic as i64,
            "mobil" => self.mobil as i64,
            "work"  => self.work as i64,
            "fertil" => self.fertil as i64,
            "oil"   => self.oil as i64,
            "uran"  => self.uran as i64,
            "mines" => self.mines as i64,
            "fallout" => self.fallout as i64,
            _ => return None,
        })
    }
    fn field_f64(&self, _: &str) -> Option<f64> { None }
    fn field_str<'a>(&'a self, _: &str) -> Option<&'a str> { None }
}

impl MatchField for Ship {
    fn field_i64(&self, f: &str) -> Option<i64> {
        Some(match f {
            "uid"   => self.uid as i64,
            "own"   => self.own as i64,
            "x"     => self.x as i64,
            "y"     => self.y as i64,
            "type"  => self.ship_type as i64,
            "eff" | "effic" => self.effic as i64,
            "mobil" => self.mobil as i64,
            "tech"  => self.tech as i64,
            _ => return None,
        })
    }
    fn field_f64(&self, _: &str) -> Option<f64> { None }
    fn field_str<'a>(&'a self, f: &str) -> Option<&'a str> {
        Some(match f { "name" => &self.name, _ => return None })
    }
}

impl MatchField for Plane {
    fn field_i64(&self, f: &str) -> Option<i64> {
        Some(match f {
            "uid"   => self.uid as i64,
            "own"   => self.own as i64,
            "x"     => self.x as i64,
            "y"     => self.y as i64,
            "type"  => self.plane_type as i64,
            "eff" | "effic" => self.effic as i64,
            "mobil" => self.mobil as i64,
            "tech"  => self.tech as i64,
            "range" => self.range as i64,
            _ => return None,
        })
    }
    fn field_f64(&self, _: &str) -> Option<f64> { None }
    fn field_str<'a>(&'a self, _: &str) -> Option<&'a str> { None }
}

impl MatchField for LandUnit {
    fn field_i64(&self, f: &str) -> Option<i64> {
        Some(match f {
            "uid"   => self.uid as i64,
            "own"   => self.own as i64,
            "x"     => self.x as i64,
            "y"     => self.y as i64,
            "type"  => self.land_type as i64,
            "eff" | "effic" => self.effic as i64,
            "mobil" => self.mobil as i64,
            "tech"  => self.tech as i64,
            _ => return None,
        })
    }
    fn field_f64(&self, _: &str) -> Option<f64> { None }
    fn field_str<'a>(&'a self, _: &str) -> Option<&'a str> { None }
}

impl MatchField for Nuke {
    fn field_i64(&self, f: &str) -> Option<i64> {
        Some(match f {
            "uid"   => self.uid as i64,
            "own"   => self.own as i64,
            "x"     => self.x as i64,
            "y"     => self.y as i64,
            "type"  => self.nuke_type as i64,
            "eff" | "effic" => self.effic as i64,
            "tech"  => self.tech as i64,
            _ => return None,
        })
    }
    fn field_f64(&self, _: &str) -> Option<f64> { None }
    fn field_str<'a>(&'a self, _: &str) -> Option<&'a str> { None }
}

// ── Public scan functions ─────────────────────────────────────────────────────

pub async fn scan_nations(db: &Db, spec: &ScanSpec) -> DbResult<Vec<Nation>> {
    let all = match &spec.area {
        SelectArea::Uid(id) => nations::get(db, *id).await?.into_iter().collect(),
        SelectArea::All     => nations::get_all(db).await?,
    };
    Ok(all.into_iter().filter(|n| n.matches_spec(&spec.conditions)).collect())
}

pub async fn scan_sectors(db: &Db, spec: &ScanSpec) -> DbResult<Vec<Sector>> {
    let all = match &spec.area {
        SelectArea::Uid(id) => sectors::get(db, *id).await?.into_iter().collect(),
        SelectArea::All     => sectors::get_all(db).await?,
    };
    Ok(all.into_iter().filter(|s| s.matches_spec(&spec.conditions)).collect())
}

pub async fn scan_ships(db: &Db, spec: &ScanSpec) -> DbResult<Vec<Ship>> {
    let all = match &spec.area {
        SelectArea::Uid(id) => ships::get(db, *id).await?.into_iter().collect(),
        SelectArea::All     => ships::get_all(db).await?,
    };
    Ok(all.into_iter().filter(|s| s.matches_spec(&spec.conditions)).collect())
}

pub async fn scan_planes(db: &Db, spec: &ScanSpec) -> DbResult<Vec<Plane>> {
    let all = match &spec.area {
        SelectArea::Uid(id) => planes::get(db, *id).await?.into_iter().collect(),
        SelectArea::All     => planes::get_all(db).await?,
    };
    Ok(all.into_iter().filter(|p| p.matches_spec(&spec.conditions)).collect())
}

pub async fn scan_land_units(db: &Db, spec: &ScanSpec) -> DbResult<Vec<LandUnit>> {
    let all = match &spec.area {
        SelectArea::Uid(id) => land_units::get(db, *id).await?.into_iter().collect(),
        SelectArea::All     => land_units::get_all(db).await?,
    };
    Ok(all.into_iter().filter(|u| u.matches_spec(&spec.conditions)).collect())
}

pub async fn scan_nukes(db: &Db, spec: &ScanSpec) -> DbResult<Vec<Nuke>> {
    let all = match &spec.area {
        SelectArea::Uid(id) => nukes::get(db, *id).await?.into_iter().collect(),
        SelectArea::All     => nukes::get_all(db).await?,
    };
    Ok(all.into_iter().filter(|n| n.matches_spec(&spec.conditions)).collect())
}

/// Dispatch a ScanSpec to the correct typed scan function, returning an
/// `XdumpRows` enum for the xdump formatter.
pub enum ScanResult {
    Nations(Vec<Nation>),
    Sectors(Vec<Sector>),
    Ships(Vec<Ship>),
    Planes(Vec<Plane>),
    LandUnits(Vec<LandUnit>),
    Nukes(Vec<Nuke>),
}

pub async fn scan(db: &Db, spec: &ScanSpec) -> DbResult<ScanResult> {
    Ok(match spec.object_type {
        ObjectType::Nation | ObjectType::Realm =>
            ScanResult::Nations(scan_nations(db, spec).await?),
        ObjectType::Sector =>
            ScanResult::Sectors(scan_sectors(db, spec).await?),
        ObjectType::Ship =>
            ScanResult::Ships(scan_ships(db, spec).await?),
        ObjectType::Plane =>
            ScanResult::Planes(scan_planes(db, spec).await?),
        ObjectType::LandUnit =>
            ScanResult::LandUnits(scan_land_units(db, spec).await?),
        ObjectType::Nuke =>
            ScanResult::Nukes(scan_nukes(db, spec).await?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;
    use empire_types::selector::parse_scan_spec;
    use empire_types::sector::SectorType;
    use empire_types::sector::DistEntry;
    use empire_types::commodity::Inventory;

    #[tokio::test]
    async fn scan_sectors_all() {
        let db = test_db().await;
        let s = Sector {
            uid: 0, own: 1, x: 0, y: 0,
            sector_type: SectorType::Capital, effic: 100, mobil: 0,
            off: false, loyal: 0, terr: [0;4], dterr: 0,
            dist_x: 0, dist_y: 0, avail: 0, flags: 0, elev: 0,
            work: 100, coastal: false, new_type: SectorType::Capital,
            min: 0, gmin: 0, fertil: 50, oil: 0, uran: 0, old_own: 0,
            che: 0, che_target: 0,
            items: Inventory::zero(), del: [DistEntry::default(); 26],
            mines: 0, pstage: 0, ptime: 0, fallout: 0,
        };
        sectors::put(&db, &s).await.unwrap();
        let spec = parse_scan_spec("sect *").unwrap();
        let result = scan_sectors(&db, &spec).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn scan_sectors_condition() {
        let db = test_db().await;
        let make = |uid, own, eff| Sector {
            uid, own, x: uid as i16, y: 0,
            sector_type: SectorType::Land, effic: eff, mobil: 0,
            off: false, loyal: 0, terr: [0;4], dterr: 0,
            dist_x: 0, dist_y: 0, avail: 0, flags: 0, elev: 0,
            work: 100, coastal: false, new_type: SectorType::Land,
            min: 0, gmin: 0, fertil: 0, oil: 0, uran: 0, old_own: 0,
            che: 0, che_target: 0,
            items: Inventory::zero(), del: [DistEntry::default(); 26],
            mines: 0, pstage: 0, ptime: 0, fallout: 0,
        };
        sectors::put(&db, &make(0, 1, 30)).await.unwrap();
        sectors::put(&db, &make(1, 1, 80)).await.unwrap();
        let spec = parse_scan_spec("sect * ?eff>50").unwrap();
        let result = scan_sectors(&db, &spec).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].uid, 1);
    }
}
