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
// Ported from: src/lib/subs/lndsub.c
// Known contributors to the original:
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995
//    Steve McClure, 1998-2000
//    Markus Armbruster, 2006-2021

// Land unit combat subsystem — fire, damage, attack eligibility, and support.
// All functions are pure (no I/O, no DB).

use empire_types::land::{LandUnit, LAND_MIN_EFF, LAND_MIN_FIRE_EFF};
use empire_types::land_chr::LandChr;
use super::damage;

/// Compute damage a land unit fires.
///
/// Formula (simplified from lndsub.c `lnd_fire`):
///   `damage = (lchr.dam * unit.effic) / 100`
///
/// Returns raw damage points; zero if unit is below minimum efficiency to fire.
pub fn lnd_fire(unit: &LandUnit, lchr: &LandChr) -> i32 {
    if unit.effic < LAND_MIN_FIRE_EFF {
        return 0;
    }
    if lchr.dam == 0 {
        return 0;
    }
    let dam = (lchr.dam as i32 * unit.effic as i32) / 100;
    dam.max(0)
}

/// Apply `dam` damage to `unit`.  Returns `true` if the unit is destroyed (effic <= 0).
///
/// Uses asymptotic PERCENT_DAMAGE then a plain `damage` call matching C's `land_damage`.
pub fn lnd_damage(unit: &mut LandUnit, dam: i32) -> bool {
    if dam <= 0 {
        return unit.effic <= 0;
    }
    let pct = damage::percent_damage(dam);
    let new_eff = damage::damage(unit.effic as i32, pct);
    unit.effic = new_eff as i8;
    unit.effic <= 0
}

/// Return true if the unit can participate in a ground attack:
///   - effic is above the minimum
///   - not aboard a ship
///   - not carried by another land unit
pub fn lnd_can_attack(unit: &LandUnit) -> bool {
    unit.effic >= LAND_MIN_EFF && unit.ship < 0 && unit.carried_by_land < 0
}

/// Compute total defensive fire from a slice of land units (support fire).
///
/// Only units that can fire (`lnd_fire > 0`) and are above `LAND_MIN_FIRE_EFF`
/// contribute.  Each unit fires independently and the damage sums.
///
/// `lchr_table`: the global LandChr table (use `LandChr::all()`).
pub fn lnd_support(units: &[LandUnit], lchr_table: &[LandChr]) -> i32 {
    units.iter().map(|u| {
        let idx = u.land_type as usize;
        match lchr_table.get(idx) {
            Some(lchr) => lnd_fire(u, lchr),
            None       => 0,
        }
    }).sum()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::land::LandUnit;
    use empire_types::land_chr::{LandChr, LandChrFlags};
    use empire_types::commodity::Inventory;
    use empire_types::ship::RetreatFlags;

    fn make_unit(effic: i8, land_type: i8) -> LandUnit {
        LandUnit {
            uid: 0, own: 1, x: 0, y: 0, land_type,
            effic, mobil: 60, off: false, tech: 50,
            army: ' ', opx: 0, opy: 0, mission: 0, mission_radius: 0,
            ship: -1, harden: 0, retreat: 50,
            retreat_flags: RetreatFlags::empty(), retreat_path: String::new(),
            scar: 0, items: Inventory::zero(),
            pstage: 0, ptime: 0, carried_by_land: -1, access: 0,
        }
    }

    fn artillery_chr() -> LandChr {
        LandChr {
            name: "artillery", sname: "art",
            lcm: 20, hcm: 10, bwork: 60, tech: 35, cost: 800,
            att: 0.1, def: 0.4, vul: 70, spd: 18, vis: 20, spy: 1, rad: 0,
            frg: 8, acc: 50, dam: 5, ammo: 2, aaf: 1,
            nxlight: 0, nland: 0,
            flags: LandChrFlags::LIGHT,
        }
    }

    #[test]
    fn fire_zero_below_min_fire_eff() {
        let unit = make_unit(LAND_MIN_FIRE_EFF - 1, 11);
        let chr = artillery_chr();
        assert_eq!(lnd_fire(&unit, &chr), 0);
    }

    #[test]
    fn fire_at_full_effic() {
        let unit = make_unit(100, 11);
        let chr = artillery_chr(); // dam = 5
        assert_eq!(lnd_fire(&unit, &chr), 5);
    }

    #[test]
    fn fire_scaled_by_effic() {
        let unit = make_unit(50, 11);
        let chr = artillery_chr(); // dam = 5
        assert_eq!(lnd_fire(&unit, &chr), 2); // (5 * 50) / 100 = 2
    }

    #[test]
    fn damage_reduces_unit_effic() {
        let mut unit = make_unit(100, 11);
        let destroyed = lnd_damage(&mut unit, 50);
        // 50 raw → percent_damage(50) = 100*50/150 = 33%
        // damage(100, 33) = 100 - 33 = 67
        assert!(!destroyed, "unit should survive 50 damage from 100% effic");
        assert!(unit.effic < 100, "effic should decrease");
    }

    #[test]
    fn lnd_damage_return_true_when_zero() {
        let mut unit = make_unit(0, 11);
        let destroyed = lnd_damage(&mut unit, 0);
        // effic is already 0 with 0 dam — function returns (unit.effic <= 0)
        assert!(destroyed, "unit with effic=0 should report destroyed");
    }

    #[test]
    fn can_attack_healthy_unit() {
        let unit = make_unit(100, 11);
        assert!(lnd_can_attack(&unit));
    }

    #[test]
    fn cannot_attack_on_ship() {
        let mut unit = make_unit(100, 11);
        unit.ship = 0;
        assert!(!lnd_can_attack(&unit));
    }

    #[test]
    fn cannot_attack_below_min_eff() {
        let unit = make_unit(LAND_MIN_EFF - 1, 11);
        assert!(!lnd_can_attack(&unit));
    }

    #[test]
    fn support_sums_fire() {
        let u1 = make_unit(100, 11); // art: dam=5, effic=100 → fire=5
        let u2 = make_unit(100, 11);
        let total = lnd_support(&[u1, u2], LandChr::all());
        assert_eq!(total, 10);
    }
}
