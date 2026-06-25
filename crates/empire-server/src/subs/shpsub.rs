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
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/subs/shpsub.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Ken Stevens, 1995
//    Steve McClure, 1996-2000
//    Markus Armbruster, 2006-2021

// Ship combat subsystem — fire, range, and damage helpers.
// All functions are pure (no I/O, no DB); callers persist mutated objects.

use empire_types::ship::Ship;
use empire_types::ship_chr::ShipChr;
use empire_types::coords::Coord;
use crate::subs::geo::map_dist;

/// Damage per gun fired — matches C's GUN_DAMAGE constant.
const DAM_PER_GUN: i32 = 8;

/// Compute damage a ship deals when firing at a target.
///
/// Formula (from shpsub.c `shp_fire`):
///   `damage = (guns * DAM_PER_GUN * tech_factor * effic) / 100`
/// where `tech_factor = 1.0 + ship.tech as f64 / 100.0`.
///
/// Returns raw damage points (0 if the ship cannot fire).
pub fn shp_fire_at_sect(ship: &Ship, mchr: &ShipChr, _target_effic: i8, _range: i32) -> i32 {
    if mchr.glim == 0 || ship.effic <= 0 {
        return 0;
    }
    // Number of guns limited by the ship's chr gun limit and current efficiency
    let guns = (mchr.glim as i32 * ship.effic as i32 / 100).max(0);
    if guns == 0 {
        return 0;
    }
    let tech_factor = 1.0 + ship.tech as f64 / 100.0;
    let raw = (guns * DAM_PER_GUN) as f64 * tech_factor;
    let damage = (raw * ship.effic as f64 / 100.0).round() as i32;
    damage.max(0)
}

/// Return true if the ship's guns can reach (tx, ty).
///
/// Wraps `geo::map_dist`; compares against `mchr.frnge` (firing range in sectors).
pub fn shp_in_range(
    ship: &Ship,
    mchr: &ShipChr,
    tx: Coord,
    ty: Coord,
    world_x: i32,
    world_y: i32,
) -> bool {
    if mchr.frnge == 0 {
        return false;
    }
    let dist = map_dist(ship.x, ship.y, tx, ty, world_x, world_y);
    dist <= mchr.frnge
}

/// Apply `dam` damage to `ship`.  Returns `true` if the ship is sunk (effic <= 0).
///
/// Uses the asymptotic PERCENT_DAMAGE approach from damage.c.
pub fn shp_damage(ship: &mut Ship, dam: i32) -> bool {
    if dam <= 0 {
        return ship.effic <= 0;
    }
    let pct = super::damage::percent_damage(dam);
    let new_eff = super::damage::damage(ship.effic as i32, pct);
    ship.effic = new_eff as i8;
    ship.effic <= 0
}

/// Return true if the ship is still afloat (effic > 0).
pub fn shp_is_afloat(ship: &Ship) -> bool {
    ship.effic > 0
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::ship::{Ship, RetreatFlags};
    use empire_types::ship_chr::{ShipChr, ShipChrFlags};
    use empire_types::commodity::Inventory;

    fn make_ship(effic: i8, tech: i16) -> Ship {
        Ship {
            uid: 0, own: 1, x: 0, y: 0, ship_type: 13, // battleship
            effic, mobil: 60, off: false, tech,
            fleet: ' ', opx: 0, opy: 0, mission: 0, mission_radius: 0,
            items: Inventory::zero(), pstage: 0, ptime: 0, access: 0,
            name: "USS Test".into(),
            orig_x: 0, orig_y: 0, orig_own: 1,
            retreat_flags: RetreatFlags::empty(), retreat_path: String::new(),
        }
    }

    fn battleship_chr() -> ShipChr {
        ShipChr {
            name: "battleship", sname: "bb",
            lcm: 50, hcm: 70, bwork: 210, tech: 45, cost: 1800,
            armor: 95, speed: 25, visib: 35, vrnge: 6, frnge: 10, glim: 7,
            nxlight: 1, nchoppers: 0, nplanes: 0, nland: 2,
            flags: ShipChrFlags::empty(),
        }
    }

    #[test]
    fn fire_zero_when_no_guns() {
        let ship = make_ship(100, 100);
        let mut chr = battleship_chr();
        chr.glim = 0;
        assert_eq!(shp_fire_at_sect(&ship, &chr, 80, 5), 0);
    }

    #[test]
    fn fire_positive_with_guns() {
        let ship = make_ship(100, 100);
        let chr = battleship_chr();
        let dam = shp_fire_at_sect(&ship, &chr, 80, 5);
        assert!(dam > 0, "expected damage > 0, got {dam}");
    }

    #[test]
    fn in_range_true() {
        let ship = make_ship(100, 100);
        let chr = battleship_chr(); // frnge = 10
        assert!(shp_in_range(&ship, &chr, 5, 0, 64, 32));
    }

    #[test]
    fn in_range_false_too_far() {
        let ship = make_ship(100, 100);
        let chr = battleship_chr(); // frnge = 10
        // dist(0,0 → 30,0) on 64x32 world = 15 > 10
        assert!(!shp_in_range(&ship, &chr, 30, 0, 64, 32));
    }

    #[test]
    fn shp_damage_reduces_effic() {
        let mut ship = make_ship(100, 100);
        let sunk = shp_damage(&mut ship, 50);
        assert!(!sunk, "ship should survive 50 damage from 100% effic");
        assert!(ship.effic < 100, "effic should decrease");
    }

    #[test]
    fn shp_damage_reports_sunk_when_zero() {
        let mut ship = make_ship(0, 100);
        let sunk = shp_damage(&mut ship, 0);
        assert!(sunk, "ship with effic=0 reports sunk");
    }

    #[test]
    fn shp_damage_partial() {
        let mut ship = make_ship(100, 100);
        let sunk = shp_damage(&mut ship, 20);
        assert!(!sunk);
        assert!(ship.effic < 100);
    }
}
