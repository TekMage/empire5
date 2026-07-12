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
// Ported from: src/lib/subs/landgun.c (fort_fire, fortgun, fortrange)

// Fortress-sector gunnery — a fort is just a sector of type `Fortress` with
// guns/shells/crew aboard. All functions are pure (no I/O, no DB); the
// command layer deducts ammo and persists the sector.

use rand::Rng;
use empire_types::sector::{Sector, SectorType};
use empire_types::commodity::Item;
use crate::subs::tech::techfact;

/// Minimum efficiency for a fortress to fire its guns.
pub const FORTEFF: i8 = 5;

/// Eligibility to fire: sector is a Fortress, effic>=5, has guns/shells,
/// and at least 5 militia aboard. Mirrors `fort_fire()`'s gate.
pub fn fort_can_fire(sector: &Sector) -> bool {
    sector.sector_type == SectorType::Fortress
        && sector.effic >= FORTEFF
        && sector.items.get(Item::Gun) > 0
        && sector.items.get(Item::Shell) > 0
        && sector.items.get(Item::Milit) >= 5
}

/// Effective firing range: `techfact(tech, 7) + 1` if effic>=60, else
/// without the bonus. Mirrors `fortrange()` (base factor is `14/2`).
pub fn fort_gun_range(effic: i8, tech: f64) -> i32 {
    let base = techfact(7.0, tech) as i32;
    if effic >= 60 { base + 1 } else { base }
}

/// `fortgun()`: effic/100 * (roll(0..30)+19) * (min(guns,7)/7).
pub fn fortgun_damage(effic: i8, guns: i32, rng: &mut impl Rng) -> i32 {
    if guns <= 0 { return 0; }
    let roll = 19 + rng.gen_range(0..30);
    let capped = guns.min(7);
    (roll * capped * effic as i32) / (7 * 100)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use empire_types::commodity::Inventory;
    use empire_types::sector::DistEntry;

    fn make_fort(effic: i8) -> Sector {
        Sector {
            uid: 0, own: 1, x: 0, y: 0, sector_type: SectorType::Fortress,
            effic, mobil: 60, off: false, loyal: 0, terr: [0; 4], dterr: 0,
            dist_x: 0, dist_y: 0, avail: 0, flags: 0, elev: 0, work: 100,
            coastal: true, new_type: SectorType::Fortress,
            min: 0, gmin: 0, fertil: 0, oil: 0, uran: 0,
            old_own: 1, che: 0, che_target: 0,
            items: Inventory::zero(),
            del: [DistEntry::default(); 26],
            mines: 0, pstage: 0, ptime: 0, fallout: 0,
        }
    }

    #[test]
    fn cannot_fire_wrong_type() {
        let mut sector = make_fort(100);
        sector.sector_type = SectorType::Capital;
        sector.items.set(Item::Gun, 5);
        sector.items.set(Item::Shell, 5);
        sector.items.set(Item::Milit, 5);
        assert!(!fort_can_fire(&sector));
    }

    #[test]
    fn cannot_fire_below_forteff() {
        let mut sector = make_fort(FORTEFF - 1);
        sector.items.set(Item::Gun, 5);
        sector.items.set(Item::Shell, 5);
        sector.items.set(Item::Milit, 5);
        assert!(!fort_can_fire(&sector));
    }

    #[test]
    fn cannot_fire_without_five_militia() {
        let mut sector = make_fort(100);
        sector.items.set(Item::Gun, 5);
        sector.items.set(Item::Shell, 5);
        sector.items.set(Item::Milit, 4);
        assert!(!fort_can_fire(&sector));
        sector.items.set(Item::Milit, 5);
        assert!(fort_can_fire(&sector));
    }

    #[test]
    fn range_bonus_at_60_effic() {
        let low = fort_gun_range(59, 100.0);
        let high = fort_gun_range(60, 100.0);
        assert_eq!(high, low + 1);
    }

    #[test]
    fn fortgun_caps_at_seven_guns() {
        let mut rng1 = StdRng::seed_from_u64(11);
        let seven = fortgun_damage(100, 7, &mut rng1);
        let mut rng2 = StdRng::seed_from_u64(11);
        let twenty = fortgun_damage(100, 20, &mut rng2);
        assert_eq!(seven, twenty, "guns beyond 7 shouldn't add damage");
    }

    #[test]
    fn fortgun_zero_guns() {
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(fortgun_damage(100, 0, &mut rng), 0);
    }
}
