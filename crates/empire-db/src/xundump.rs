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
// Ported from: src/lib/common/xdump.c, src/util/xundump.c
// Known contributors to the original:
//    Ron Koenderink, 2005
//    Markus Armbruster, 2005-2016

// xundump — parse game state from xdump text format back into the database.
//
// Accepts the format produced by xdump.rs:
//   XDUMP <type> <timestamp>
//   <field-names...>
//   <values...>
//   ...
//   /
//   <N> records
//
// ref: src/server/xundump.c (empire4.4.1)

use std::collections::HashMap;
use empire_types::nation::{Nation, NatFlags, NatStatus};
use empire_types::sector::{Sector, SectorType, DistEntry};
use empire_types::ship::{Ship, RetreatFlags};
use empire_types::plane::{Plane, PlaneFlags};
use empire_types::land::LandUnit;
use empire_types::nuke::Nuke;
use empire_types::commodity::{Inventory, Item};
use crate::Db;

#[derive(Debug)]
pub struct UndumpError(pub String);
impl std::fmt::Display for UndumpError { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "xundump: {}", self.0) } }
impl std::error::Error for UndumpError {}

pub type UndumpResult<T> = Result<T, UndumpError>;

// ── Parse helpers ─────────────────────────────────────────────────────────────

struct Parser<'a> {
    lines: std::str::Lines<'a>,
    fields: Vec<String>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> UndumpResult<(String, i64, Self)> {
        let mut lines = input.lines();

        // XDUMP <type> <timestamp>
        let header = lines.next().ok_or_else(|| UndumpError("empty input".into()))?;
        let parts: Vec<&str> = header.splitn(3, ' ').collect();
        if parts.len() != 3 || parts[0] != "XDUMP" {
            return Err(UndumpError(format!("bad header: {header}")));
        }
        let kind = parts[1].to_string();
        let ts: i64 = parts[2].parse().map_err(|_| UndumpError("bad timestamp".into()))?;

        // field names
        let field_line = lines.next().ok_or_else(|| UndumpError("missing field line".into()))?;
        let fields = field_line.split_whitespace().map(str::to_string).collect();

        Ok((kind, ts, Parser { lines, fields }))
    }

    fn parse_rows(&mut self) -> UndumpResult<Vec<HashMap<String, String>>> {
        let mut rows = Vec::new();
        for line in &mut self.lines {
            let line = line.trim();
            if line == "/" { break; }
            if line.is_empty() { continue; }
            let vals: Vec<&str> = line.split_whitespace().collect();
            if vals.len() != self.fields.len() {
                return Err(UndumpError(format!(
                    "field count mismatch: got {} expected {}: {line}",
                    vals.len(), self.fields.len()
                )));
            }
            let mut row = HashMap::new();
            for (k, v) in self.fields.iter().zip(vals.iter()) {
                row.insert(k.clone(), unquote(v));
            }
            rows.push(row);
        }
        Ok(rows)
    }
}

fn unquote(s: &str) -> String {
    if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
        s[1..s.len()-1].replace('_', " ")
    } else {
        s.to_string()
    }
}
fn get_i64(row: &HashMap<String, String>, key: &str) -> UndumpResult<i64> {
    row.get(key)
       .ok_or_else(|| UndumpError(format!("missing field {key}")))?
       .parse::<i64>().map_err(|_| UndumpError(format!("bad i64 for {key}")))
}
fn get_i32(row: &HashMap<String, String>, key: &str) -> UndumpResult<i32> {
    Ok(get_i64(row, key)? as i32)
}
fn get_i16(row: &HashMap<String, String>, key: &str) -> UndumpResult<i16> {
    Ok(get_i64(row, key)? as i16)
}
fn get_i8(row: &HashMap<String, String>, key: &str) -> UndumpResult<i8> {
    Ok(get_i64(row, key)? as i8)
}
fn get_u8(row: &HashMap<String, String>, key: &str) -> UndumpResult<u8> {
    Ok(get_i64(row, key)? as u8)
}
fn get_f64(row: &HashMap<String, String>, key: &str) -> UndumpResult<f64> {
    row.get(key)
       .ok_or_else(|| UndumpError(format!("missing field {key}")))?
       .parse::<f64>().map_err(|_| UndumpError(format!("bad f64 for {key}")))
}
fn get_str<'a>(row: &'a HashMap<String, String>, key: &str) -> UndumpResult<&'a str> {
    row.get(key).map(|s| s.as_str()).ok_or_else(|| UndumpError(format!("missing field {key}")))
}
fn get_char(row: &HashMap<String, String>, key: &str) -> UndumpResult<char> {
    let s = get_str(row, key)?;
    Ok(if s == "-" { ' ' } else { s.chars().next().unwrap_or(' ') })
}
fn inventory_from_row(row: &HashMap<String, String>) -> UndumpResult<Inventory> {
    let mut inv = Inventory::zero();
    let pairs = [
        ("civil", Item::Civil), ("milit", Item::Milit), ("food", Item::Food),
        ("shell", Item::Shell), ("gun", Item::Gun), ("petrol", Item::Petrol),
        ("iron", Item::Iron), ("dust", Item::Dust), ("bar", Item::Bar),
        ("lcm", Item::Lcm), ("hcm", Item::Hcm), ("rad", Item::Rad),
    ];
    for (k, item) in &pairs {
        if let Ok(v) = get_i64(row, k) { inv.set(*item, v as i16); }
    }
    Ok(inv)
}
fn nat_status(v: i64) -> NatStatus {
    match v { 1=>NatStatus::New, 2=>NatStatus::Visitor, 3=>NatStatus::Sanct,
              4=>NatStatus::Active, 5=>NatStatus::Deity, _=>NatStatus::Unused }
}
fn sect_type(v: i64) -> SectorType {
    match v {
        0 =>SectorType::Sea,          1 =>SectorType::Mountain,
        2 =>SectorType::Sanctuary,    3 =>SectorType::Wasteland,
        4 =>SectorType::Wilderness,   5 =>SectorType::Capital,
        6 =>SectorType::UraniumMine,  7 =>SectorType::Park,
        8 =>SectorType::DefensePlant, 9 =>SectorType::ShellIndus,
        10=>SectorType::Mine,         11=>SectorType::GoldMine,
        12=>SectorType::Harbor,       13=>SectorType::Warehouse,
        14=>SectorType::Airfield,     15=>SectorType::Agri,
        16=>SectorType::OilField,     17=>SectorType::LightManuf,
        18=>SectorType::HeavyManuf,   19=>SectorType::Fortress,
        20=>SectorType::TechCenter,   21=>SectorType::ResearchLab,
        22=>SectorType::NuclearPlant, 23=>SectorType::Library,
        24=>SectorType::Highway,      25=>SectorType::Radar,
        26=>SectorType::Headquarters, 27=>SectorType::BridgeHead,
        28=>SectorType::BridgeSpan,   29=>SectorType::Bank,
        30=>SectorType::Refinery,     31=>SectorType::Enlist,
        32=>SectorType::Plains,       33=>SectorType::BridgeTower,
        _ =>SectorType::Wilderness,
    }
}

// ── Row → struct converters ───────────────────────────────────────────────────

fn row_to_nation(row: &HashMap<String, String>) -> UndumpResult<Nation> {
    Ok(Nation {
        uid: get_i32(row, "uid")?, cnum: get_u8(row, "cnum")?,
        status: nat_status(get_i64(row, "status")?),
        flags: NatFlags::from_bits_truncate(get_i64(row, "flags")? as u32),
        name: get_str(row, "name")?.to_string(),
        representative: get_str(row, "representative")?.to_string(),
        host_addr: get_str(row, "host")?.to_string(),
        user_id: String::new(),
        xcap: get_i16(row, "xcap")?, ycap: get_i16(row, "ycap")?,
        xorg: get_i16(row, "xorg")?, yorg: get_i16(row, "yorg")?,
        money: get_i32(row, "money")?, reserve: get_i32(row, "reserve")?,
        tech: get_f64(row, "tech")?, research: get_f64(row, "research")?,
        education: get_f64(row, "education")?, happiness: get_f64(row, "happiness")?,
        login_count: get_i32(row, "login_count")?, tele_cnt: 0, ann_cnt: 0, last_ann_read: 0,
        passwd_hash: row.get("passwd_hash").cloned().unwrap_or_default(),
        last_login: get_i64(row, "last_login").unwrap_or(0),
        last_logout: get_i64(row, "last_logout").unwrap_or(0),
        news_time: 0,
    })
}

fn row_to_sector(row: &HashMap<String, String>) -> UndumpResult<Sector> {
    Ok(Sector {
        uid: get_i32(row, "uid")?, own: get_u8(row, "own")?,
        x: get_i16(row, "x")?, y: get_i16(row, "y")?,
        sector_type: sect_type(get_i64(row, "type")?),
        effic: get_i8(row, "eff")?, mobil: get_i8(row, "mobil")?,
        off: false, loyal: get_u8(row, "loyal")?,
        terr: [0;4], dterr: 0, dist_x: 0, dist_y: 0, avail: 0,
        flags: 0, elev: 0, work: get_u8(row, "work")?, coastal: false,
        new_type: sect_type(get_i64(row, "type")?),
        min: 0, gmin: 0,
        fertil: get_u8(row, "fertil")?, oil: get_u8(row, "oil")?,
        uran: get_u8(row, "uran")?, old_own: 0,
        che: 0, che_target: 0,
        items: inventory_from_row(row)?,
        del: [DistEntry::default(); 26],
        mines: get_i16(row, "mines")?, pstage: 0, ptime: 0,
        fallout: get_i64(row, "fallout")? as i32,
    })
}

fn row_to_ship(row: &HashMap<String, String>) -> UndumpResult<Ship> {
    Ok(Ship {
        uid: get_i32(row, "uid")?, own: get_u8(row, "own")?,
        x: get_i16(row, "x")?, y: get_i16(row, "y")?,
        ship_type: get_i8(row, "type")?, effic: get_i8(row, "eff")?,
        mobil: get_i8(row, "mobil")?, off: false, tech: get_i16(row, "tech")?,
        fleet: get_char(row, "fleet")?,
        opx: 0, opy: 0, mission: 0, mission_radius: 0,
        items: inventory_from_row(row)?, pstage: 0, ptime: 0, access: 0,
        name: get_str(row, "name")?.to_string(),
        orig_x: get_i16(row, "orig_x")?, orig_y: get_i16(row, "orig_y")?,
        orig_own: get_u8(row, "orig_own")?,
        retreat_flags: RetreatFlags::from_bits_truncate(get_i64(row, "retreat_flags")? as u32),
        retreat_path: String::new(),
    })
}

fn row_to_plane(row: &HashMap<String, String>) -> UndumpResult<Plane> {
    Ok(Plane {
        uid: get_i32(row, "uid")?, own: get_u8(row, "own")?,
        x: get_i16(row, "x")?, y: get_i16(row, "y")?,
        plane_type: get_i8(row, "type")?, effic: get_i8(row, "eff")?,
        mobil: get_i8(row, "mobil")?, off: false, tech: get_i16(row, "tech")?,
        wing: get_char(row, "wing")?,
        opx: 0, opy: 0, mission: 0, mission_radius: 0,
        range: get_u8(row, "range")?, harden: get_i8(row, "harden")?,
        ship: get_i32(row, "ship")?, land: get_i32(row, "land")?,
        flags: PlaneFlags::from_bits_truncate(get_i64(row, "flags")? as u32),
        access: 0, theta: 0.0,
    })
}

fn row_to_land(row: &HashMap<String, String>) -> UndumpResult<LandUnit> {
    Ok(LandUnit {
        uid: get_i32(row, "uid")?, own: get_u8(row, "own")?,
        x: get_i16(row, "x")?, y: get_i16(row, "y")?,
        land_type: get_i8(row, "type")?, effic: get_i8(row, "eff")?,
        mobil: get_i8(row, "mobil")?, off: false, tech: get_i16(row, "tech")?,
        army: get_char(row, "army")?,
        opx: 0, opy: 0, mission: 0, mission_radius: 0,
        ship: get_i32(row, "ship")?, harden: get_i8(row, "harden")?,
        retreat: 0, retreat_flags: RetreatFlags::empty(), retreat_path: String::new(),
        scar: get_u8(row, "scar")?,
        items: inventory_from_row(row)?, pstage: 0, ptime: 0,
        carried_by_land: -1, access: 0,
    })
}

fn row_to_nuke(row: &HashMap<String, String>) -> UndumpResult<Nuke> {
    Ok(Nuke {
        uid: get_i32(row, "uid")?, own: get_u8(row, "own")?,
        x: get_i16(row, "x")?, y: get_i16(row, "y")?,
        nuke_type: get_i8(row, "type")?, effic: get_i8(row, "eff")?,
        tech: get_i16(row, "tech")?,
        stockpile: get_char(row, "stockpile")?,
        plane: get_i32(row, "plane")?,
    })
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Load an xdump text block into the database.  Replaces any existing records
/// (INSERT OR REPLACE via the individual `put` functions).
pub async fn load(db: &Db, input: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let (kind, _ts, mut parser) = Parser::new(input)?;
    let rows = parser.parse_rows()?;
    let count = rows.len();
    match kind.as_str() {
        "nation" => {
            for row in &rows {
                let n = row_to_nation(row)?;
                crate::nations::put(db, &n).await?;
            }
        }
        "sector" => {
            for row in &rows {
                let s = row_to_sector(row)?;
                crate::sectors::put(db, &s).await?;
            }
        }
        "ship" => {
            for row in &rows {
                let s = row_to_ship(row)?;
                crate::ships::put(db, &s).await?;
            }
        }
        "plane" => {
            for row in &rows {
                let p = row_to_plane(row)?;
                crate::planes::put(db, &p).await?;
            }
        }
        "land" => {
            for row in &rows {
                let u = row_to_land(row)?;
                crate::land_units::put(db, &u).await?;
            }
        }
        "nuke" => {
            for row in &rows {
                let n = row_to_nuke(row)?;
                crate::nukes::put(db, &n).await?;
            }
        }
        other => return Err(Box::new(UndumpError(format!("unknown type '{other}'")))),
    }
    Ok(count)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_db, xdump, nations};
    use empire_types::nation::{NatStatus, NatFlags, Nation};

    fn sample_nation() -> Nation {
        Nation {
            uid: 1, cnum: 1, status: NatStatus::Active,
            flags: NatFlags::empty(),
            name: "Testland".into(), representative: "Bob".into(),
            host_addr: "127.0.0.1".into(), user_id: "bob".into(),
            xcap: 0, ycap: 0, xorg: 0, yorg: 0,
            money: 20_000, reserve: 0,
            tech: 10.5, research: 0.0, education: 0.0, happiness: 50.0,
            login_count: 3, tele_cnt: 0,
            passwd_hash: "".into(), last_login: 0, last_logout: 0,
        }
    }

    #[tokio::test]
    async fn round_trip_nation() {
        let db = test_db().await;
        let orig = sample_nation();
        nations::put(&db, &orig).await.unwrap();

        let all = nations::get_all(&db).await.unwrap();
        let dump = xdump::dump_nations(&all, 1234);

        // clear and reload
        let db2 = test_db().await;
        let n = load(&db2, &dump).await.unwrap();
        assert_eq!(n, 1);

        let restored = nations::get(&db2, 1).await.unwrap().unwrap();
        assert_eq!(restored.name, "Testland");
        assert_eq!(restored.money, 20_000);
        assert!((restored.tech - 10.5).abs() < 0.01);
    }
}
