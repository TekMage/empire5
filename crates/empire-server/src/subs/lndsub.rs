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
// Ported from: src/lib/subs/lndsub.c, src/lib/subs/landgun.c
// Known contributors to the original:
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995
//    Steve McClure, 1998-2000
//    Markus Armbruster, 2006-2021

// Land unit combat subsystem — fire, damage, attack eligibility, and support.
// All functions are pure (no I/O, no DB).

use rand::Rng;
use empire_types::land::{LandUnit, LAND_MIN_EFF, LAND_MIN_FIRE_EFF};
use empire_types::land_chr::LandChr;
use empire_types::commodity::Item;
use crate::subs::tech::techfact;
use super::damage;

/// Eligibility to fire (artillery): effic above minimum, not loaded on a
/// ship or another unit, has a nonzero damage stat, and has guns/shells/
/// crew aboard. Mirrors `c_fire()`'s land-unit branch (minus the
/// auto-resupply-from-sector step, which empire5 doesn't model yet — a
/// unit with 0 shells on hand simply can't fire in v1).
pub fn lnd_can_fire(unit: &LandUnit, lchr: &LandChr) -> bool {
    unit.effic >= LAND_MIN_FIRE_EFF
        && unit.ship < 0
        && unit.carried_by_land < 0
        && lchr.dam != 0
        && unit.items.get(Item::Gun) > 0
        && unit.items.get(Item::Shell) > 0
        && unit.items.get(Item::Milit) >= 1
}

/// Effective firing range for a land unit: `techfact(tech, lchr.frg)`.
pub fn lnd_gun_range(unit: &LandUnit, lchr: &LandChr) -> i32 {
    techfact(lchr.frg as f64, unit.tech as f64) as i32
}

/// `landunitgun()`: effic/100 * sum_{1..guns}(4 + roll(0..6)).
pub fn landunitgun_damage(effic: i8, guns: i32, rng: &mut impl Rng) -> i32 {
    if guns <= 0 { return 0; }
    let raw: i32 = (0..guns).map(|_| 4 + rng.gen_range(0..6)).sum();
    (raw * effic as i32) / 100
}

/// Fire one shot from `unit`, mutating its shell count. Guns fired are
/// capped by the unit-type's damage stat and guns carried; damage is
/// computed with the full gun count, then proportionally reduced (and
/// shell consumption capped) if fewer shells are on hand than the unit's
/// per-shot ammo requirement. Mirrors `lnd_fire()` in landgun.c exactly.
/// Caller checks `lnd_can_fire` first.
pub fn lnd_fire_shot(unit: &mut LandUnit, lchr: &LandChr, rng: &mut impl Rng) -> i32 {
    let guns = lchr.dam.min(unit.items.get(Item::Gun) as i32).max(0);
    if guns <= 0 { return 0; }

    let ammo = if lchr.ammo == 0 { 1 } else { lchr.ammo };
    let shells = unit.items.get(Item::Shell) as i32;
    if shells <= 0 { return 0; }

    let mut dam = landunitgun_damage(unit.effic, guns, rng) as f64;
    let shells_used = shells.min(ammo);
    if shells < ammo {
        dam *= shells as f64 / ammo as f64;
    }
    unit.items.add(Item::Shell, -(shells_used as i16));

    dam.round() as i32
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
/// Only units above `LAND_MIN_FIRE_EFF` with a nonzero damage stat and guns
/// aboard contribute. This is a non-random preview estimate (ground-combat
/// support callers don't have `&mut` access to consume ammo here) — kept
/// as a simple approximation since this function currently has no live
/// callers (ground-combat support fire is a separate, not-yet-built
/// feature); it is not the same code path as `lnd_fire_shot`.
///
/// `lchr_table`: the global LandChr table (use `LandChr::all()`).
pub fn lnd_support(units: &[LandUnit], lchr_table: &[LandChr]) -> i32 {
    units.iter().map(|u| {
        let idx = u.land_type as usize;
        match lchr_table.get(idx) {
            Some(lchr) if u.effic >= LAND_MIN_FIRE_EFF && lchr.dam != 0 => {
                let guns = lchr.dam.min(u.items.get(Item::Gun) as i32).max(0);
                (guns * u.effic as i32) / 100
            }
            _ => 0,
        }
    }).sum()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
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
    fn cannot_fire_below_min_eff() {
        let mut unit = make_unit(LAND_MIN_FIRE_EFF - 1, 11);
        unit.items.set(Item::Gun, 5);
        unit.items.set(Item::Shell, 5);
        unit.items.set(Item::Milit, 5);
        assert!(!lnd_can_fire(&unit, &artillery_chr()));
    }

    #[test]
    fn cannot_fire_without_ammo() {
        let mut unit = make_unit(100, 11);
        unit.items.set(Item::Gun, 5);
        unit.items.set(Item::Milit, 5);
        assert!(!lnd_can_fire(&unit, &artillery_chr()), "no shells yet");
        unit.items.set(Item::Shell, 5);
        assert!(lnd_can_fire(&unit, &artillery_chr()));
    }

    #[test]
    fn gun_range_scales_with_tech() {
        let low = make_unit(100, 11);
        let mut high = make_unit(100, 11);
        high.tech = 300;
        let chr = artillery_chr();
        assert!(lnd_gun_range(&high, &chr) > lnd_gun_range(&low, &chr));
    }

    #[test]
    fn fire_shot_full_ammo_uses_full_guns() {
        let mut unit = make_unit(100, 11);
        unit.items.set(Item::Gun, 5);
        unit.items.set(Item::Shell, 10);
        let chr = artillery_chr(); // dam=5, ammo=2
        let mut rng = StdRng::seed_from_u64(5);
        let dam = lnd_fire_shot(&mut unit, &chr, &mut rng);
        assert!(dam > 0);
        assert_eq!(unit.items.get(Item::Shell), 8, "ammo=2 consumed");
    }

    #[test]
    fn fire_shot_scales_down_when_short_on_shells() {
        let mut full_ammo_unit = make_unit(100, 11);
        full_ammo_unit.items.set(Item::Gun, 5);
        full_ammo_unit.items.set(Item::Shell, 10);
        let mut short_unit = make_unit(100, 11);
        short_unit.items.set(Item::Gun, 5);
        short_unit.items.set(Item::Shell, 1); // ammo=2, only 1 on hand
        let chr = artillery_chr();

        let mut rng1 = StdRng::seed_from_u64(9);
        let full_dam = lnd_fire_shot(&mut full_ammo_unit, &chr, &mut rng1);
        let mut rng2 = StdRng::seed_from_u64(9);
        let short_dam = lnd_fire_shot(&mut short_unit, &chr, &mut rng2);

        assert!(short_dam < full_dam, "shell shortage should scale damage down");
        assert_eq!(short_unit.items.get(Item::Shell), 0, "only the 1 available shell consumed");
    }

    #[test]
    fn fire_shot_zero_when_no_shells() {
        let mut unit = make_unit(100, 11);
        unit.items.set(Item::Gun, 5);
        let chr = artillery_chr();
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(lnd_fire_shot(&mut unit, &chr, &mut rng), 0);
    }

    #[test]
    fn damage_reduces_unit_effic() {
        let mut unit = make_unit(100, 11);
        let destroyed = lnd_damage(&mut unit, 50);
        assert!(!destroyed, "unit should survive 50 damage from 100% effic");
        assert!(unit.effic < 100, "effic should decrease");
    }

    #[test]
    fn lnd_damage_return_true_when_zero() {
        let mut unit = make_unit(0, 11);
        let destroyed = lnd_damage(&mut unit, 0);
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
        let mut u1 = make_unit(100, 11);
        u1.items.set(Item::Gun, 5);
        let mut u2 = make_unit(100, 11);
        u2.items.set(Item::Gun, 5);
        let total = lnd_support(&[u1, u2], LandChr::all());
        assert_eq!(total, 10); // dam=5 capped by gun=5, *2 units
    }
}
