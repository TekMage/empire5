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
// Ported from: src/lib/common/xdump.c, include/xdump.h
// Known contributors to the original:
//    Markus Armbruster, 2004-2016

// xdump — write game state as a text dump compatible with the Empire wire format.
//
// Format:
//   XDUMP <type> <timestamp-secs>
//   <field-names...>
//   <values...>
//   ...
//   /
//   <record-count> records
//
// All values are space-separated on each row.  Strings are single-quoted and
// spaces within them are replaced with '_'.  Missing/unset chars print as '-'.
// ref: src/server/xdump.c (empire4.4.1)

use empire_types::{Nation, Sector, Ship, Plane, LandUnit, Nuke};
use empire_types::commodity::Item;

// ── Public entry points ───────────────────────────────────────────────────────

pub fn dump_nations(nations: &[Nation], ts: i64) -> String {
    let fields = &[
        "uid", "cnum", "status", "flags",
        "name", "representative", "host",
        "xcap", "ycap", "xorg", "yorg",
        "money", "reserve", "tech", "research", "education", "happiness",
        "login_count", "passwd_hash", "last_login", "last_logout",
    ];
    let mut out = header("nation", ts);
    out.push_str(&fields.join(" "));
    out.push('\n');
    for n in nations {
        let row = vec![
            n.uid.to_string(), n.cnum.to_string(), (n.status as i32).to_string(),
            n.flags.bits().to_string(),
            quote_str(&n.name), quote_str(&n.representative), quote_str(&n.host_addr),
            n.xcap.to_string(), n.ycap.to_string(),
            n.xorg.to_string(), n.yorg.to_string(),
            n.money.to_string(), n.reserve.to_string(),
            format!("{:.2}", n.tech), format!("{:.2}", n.research),
            format!("{:.2}", n.education), format!("{:.2}", n.happiness),
            n.login_count.to_string(),
            quote_str(&n.passwd_hash),
            n.last_login.to_string(), n.last_logout.to_string(),
        ];
        out.push_str(&row.join(" "));
        out.push('\n');
    }
    out.push_str(&footer(nations.len()));
    out
}

pub fn dump_sectors(sectors: &[Sector], ts: i64) -> String {
    let fields = &[
        "x", "y", "uid", "own", "type", "eff", "mobil",
        "work", "loyal", "fertil", "oil", "uran", "mines",
        "fallout", "civil", "milit", "food", "shell", "gun",
        "petrol", "iron", "dust", "bar", "lcm", "hcm", "rad",
    ];
    let mut out = header("sector", ts);
    out.push_str(&fields.join(" "));
    out.push('\n');
    for s in sectors {
        let inv = &s.items;
        let row = vec![
            s.x.to_string(), s.y.to_string(), s.uid.to_string(), s.own.to_string(),
            (s.sector_type as i32).to_string(), s.effic.to_string(), s.mobil.to_string(),
            s.work.to_string(), s.loyal.to_string(),
            s.fertil.to_string(), s.oil.to_string(), s.uran.to_string(),
            s.mines.to_string(), s.fallout.to_string(),
            inv.get(Item::Civil).to_string(), inv.get(Item::Milit).to_string(),
            inv.get(Item::Food).to_string(), inv.get(Item::Shell).to_string(),
            inv.get(Item::Gun).to_string(), inv.get(Item::Petrol).to_string(),
            inv.get(Item::Iron).to_string(), inv.get(Item::Dust).to_string(),
            inv.get(Item::Bar).to_string(), inv.get(Item::Lcm).to_string(),
            inv.get(Item::Hcm).to_string(), inv.get(Item::Rad).to_string(),
        ];
        out.push_str(&row.join(" "));
        out.push('\n');
    }
    out.push_str(&footer(sectors.len()));
    out
}

pub fn dump_ships(ships: &[Ship], ts: i64) -> String {
    let fields = &[
        "uid", "own", "x", "y", "type", "eff", "mobil", "tech",
        "fleet", "orig_x", "orig_y", "orig_own",
        "civil", "milit", "food", "shell", "gun", "petrol",
        "iron", "dust", "bar", "lcm", "hcm", "rad",
        "retreat_flags", "name",
    ];
    let mut out = header("ship", ts);
    out.push_str(&fields.join(" "));
    out.push('\n');
    for s in ships {
        let inv = &s.items;
        let row = vec![
            s.uid.to_string(), s.own.to_string(), s.x.to_string(), s.y.to_string(),
            s.ship_type.to_string(), s.effic.to_string(), s.mobil.to_string(),
            s.tech.to_string(), fmt_char(s.fleet),
            s.orig_x.to_string(), s.orig_y.to_string(), s.orig_own.to_string(),
            inv.get(Item::Civil).to_string(), inv.get(Item::Milit).to_string(),
            inv.get(Item::Food).to_string(), inv.get(Item::Shell).to_string(),
            inv.get(Item::Gun).to_string(), inv.get(Item::Petrol).to_string(),
            inv.get(Item::Iron).to_string(), inv.get(Item::Dust).to_string(),
            inv.get(Item::Bar).to_string(), inv.get(Item::Lcm).to_string(),
            inv.get(Item::Hcm).to_string(), inv.get(Item::Rad).to_string(),
            s.retreat_flags.bits().to_string(),
            quote_str(&s.name),
        ];
        out.push_str(&row.join(" "));
        out.push('\n');
    }
    out.push_str(&footer(ships.len()));
    out
}

pub fn dump_planes(planes: &[Plane], ts: i64) -> String {
    let fields = &[
        "uid", "own", "x", "y", "type", "eff", "mobil", "tech",
        "wing", "range", "harden", "ship", "land", "flags",
    ];
    let mut out = header("plane", ts);
    out.push_str(&fields.join(" "));
    out.push('\n');
    for p in planes {
        let row = vec![
            p.uid.to_string(), p.own.to_string(), p.x.to_string(), p.y.to_string(),
            p.plane_type.to_string(), p.effic.to_string(), p.mobil.to_string(),
            p.tech.to_string(), fmt_char(p.wing),
            p.range.to_string(), p.harden.to_string(),
            p.ship.to_string(), p.land.to_string(),
            p.flags.bits().to_string(),
        ];
        out.push_str(&row.join(" "));
        out.push('\n');
    }
    out.push_str(&footer(planes.len()));
    out
}

pub fn dump_land_units(units: &[LandUnit], ts: i64) -> String {
    let fields = &[
        "uid", "own", "x", "y", "type", "eff", "mobil", "tech",
        "army", "ship", "harden", "scar",
        "civil", "milit", "food", "shell", "gun", "petrol",
        "iron", "dust", "bar", "lcm", "hcm", "rad",
    ];
    let mut out = header("land", ts);
    out.push_str(&fields.join(" "));
    out.push('\n');
    for u in units {
        let inv = &u.items;
        let row = vec![
            u.uid.to_string(), u.own.to_string(), u.x.to_string(), u.y.to_string(),
            u.land_type.to_string(), u.effic.to_string(), u.mobil.to_string(),
            u.tech.to_string(), fmt_char(u.army),
            u.ship.to_string(), u.harden.to_string(), u.scar.to_string(),
            inv.get(Item::Civil).to_string(), inv.get(Item::Milit).to_string(),
            inv.get(Item::Food).to_string(), inv.get(Item::Shell).to_string(),
            inv.get(Item::Gun).to_string(), inv.get(Item::Petrol).to_string(),
            inv.get(Item::Iron).to_string(), inv.get(Item::Dust).to_string(),
            inv.get(Item::Bar).to_string(), inv.get(Item::Lcm).to_string(),
            inv.get(Item::Hcm).to_string(), inv.get(Item::Rad).to_string(),
        ];
        out.push_str(&row.join(" "));
        out.push('\n');
    }
    out.push_str(&footer(units.len()));
    out
}

pub fn dump_nukes(nukes: &[Nuke], ts: i64) -> String {
    let fields = &["uid", "own", "x", "y", "type", "eff", "tech", "stockpile", "plane"];
    let mut out = header("nuke", ts);
    out.push_str(&fields.join(" "));
    out.push('\n');
    for n in nukes {
        let row = vec![
            n.uid.to_string(), n.own.to_string(), n.x.to_string(), n.y.to_string(),
            n.nuke_type.to_string(), n.effic.to_string(), n.tech.to_string(),
            fmt_char(n.stockpile), n.plane.to_string(),
        ];
        out.push_str(&row.join(" "));
        out.push('\n');
    }
    out.push_str(&footer(nukes.len()));
    out
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn header(kind: &str, ts: i64) -> String {
    format!("XDUMP {kind} {ts}\n")
}

fn footer(count: usize) -> String {
    format!("/\n{count} records\n")
}

fn quote_str(s: &str) -> String {
    format!("'{}'", s.replace(' ', "_"))
}

fn fmt_char(c: char) -> String {
    if c == ' ' || c == '\0' { "-".to_string() } else { c.to_string() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::nation::{NatStatus, NatFlags, Nation};
    use empire_types::sector::{Sector, SectorType, DistEntry};
    use empire_types::commodity::Inventory;

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

    #[test]
    fn nation_dump_starts_with_header() {
        let out = dump_nations(&[sample_nation()], 1000);
        assert!(out.starts_with("XDUMP nation 1000\n"));
        assert!(out.contains("'Testland'"));
        assert!(out.ends_with("1 records\n"));
    }

    #[test]
    fn sector_dump_format() {
        let s = Sector {
            uid: 0, own: 1, x: 0, y: 0,
            sector_type: SectorType::Urban, effic: 100, mobil: 60,
            off: false, loyal: 0, terr: [0;4], dterr: 0,
            dist_x: 0, dist_y: 0, avail: 0, flags: 0, elev: 0,
            work: 100, coastal: false, new_type: SectorType::Urban,
            min: 0, gmin: 0, fertil: 80, oil: 0, uran: 0, old_own: 0,
            items: Inventory::zero(), del: [DistEntry::default(); 26],
            mines: 0, pstage: 0, ptime: 0, fallout: 0,
        };
        let out = dump_sectors(&[s], 0);
        assert!(out.contains("XDUMP sector 0\n"));
        assert!(out.ends_with("1 records\n"));
    }

    #[test]
    fn empty_dump() {
        let out = dump_ships(&[], 0);
        assert!(out.contains("0 records\n"));
    }
}
