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
// Ported from: could_be_on_ship()/put_plane_on_ship()/ship_can_carry()/
// inc_shp_nplane()/carrier_planes() in subs/plnsub.c.

// Shared logic for putting planes aboard ships -- used by the `load`/
// `unload` commands (any CARRIER- or MISSILE-flagged ship, no sector
// requirement beyond matching coordinates) and by `fly`/`recon`/`sweep`
// landing at the end of a mission (CARRIER-flagged ships only, and
// only those at >= SHIP_AIROPS_EFF efficiency -- matches 4.4.1's
// carrier_planes(), which is stricter than could_be_on_ship() since it
// also gates whether a ship is even *offered* as a landing site).

use empire_types::coords::{Coord, NatId};
use empire_types::plane::Plane;
use empire_types::plane_chr::{PlaneChr, PlaneChrFlags};
use empire_types::ship::Ship;
use empire_types::ship_chr::{ShipChr, ShipChrFlags};

/// Minimum ship efficiency for air operations (SHP_AIROPS_EFF in ship.h).
/// Only relevant to landing during flight, not to the `load` command.
pub const SHIP_AIROPS_EFF: i8 = 50;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CarryBucket {
    Chopper,
    XLight,
    Missile,
    FixedWing,
}

/// Classify a plane type for carrier/sub capacity purposes. Ported
/// from inc_shp_nplane(): HELO and XLIGHT are checked first regardless
/// of LIGHT; anything else needs LIGHT to be loadable at all, and
/// among LIGHT planes, MISSILE-flagged ones go in the missile bucket
/// and everything else is plain fixed-wing.
pub fn classify_plane(flags: PlaneChrFlags) -> Option<CarryBucket> {
    if flags.contains(PlaneChrFlags::HELO) {
        Some(CarryBucket::Chopper)
    } else if flags.contains(PlaneChrFlags::XLIGHT) {
        Some(CarryBucket::XLight)
    } else if !flags.contains(PlaneChrFlags::LIGHT) {
        None
    } else if flags.contains(PlaneChrFlags::MISSILE) {
        Some(CarryBucket::Missile)
    } else {
        Some(CarryBucket::FixedWing)
    }
}

/// Can `ship_chr` carry everything already in `existing` plus one more
/// plane with `incoming_flags`? Ported from ship_can_carry(): chopper
/// overflow beyond nchoppers spills into the fixed-wing count, xlight
/// overflow beyond nxlight spills into the missile count, missile-
/// bucket planes need MISSILE or CARRIER, fixed-wing needs CARRIER.
pub fn ship_can_carry(
    ship_chr: &ShipChr,
    existing: &[Plane],
    chrs: &[PlaneChr],
    incoming_flags: PlaneChrFlags,
) -> bool {
    let Some(incoming_bucket) = classify_plane(incoming_flags) else {
        return false;
    };

    let mut n: i32 = existing.len() as i32 + 1;
    let mut nch: i32 = 0;
    let mut nxl: i32 = 0;
    let mut nmsl: i32 = 0;

    for p in existing {
        if let Some(chr) = chrs.get(p.plane_type as usize) {
            match classify_plane(chr.flags) {
                Some(CarryBucket::Chopper) => nch += 1,
                Some(CarryBucket::XLight) => nxl += 1,
                Some(CarryBucket::Missile) => nmsl += 1,
                _ => {}
            }
        }
    }
    match incoming_bucket {
        CarryBucket::Chopper => nch += 1,
        CarryBucket::XLight => nxl += 1,
        CarryBucket::Missile => nmsl += 1,
        CarryBucket::FixedWing => {}
    }

    let mut nfw = n - nch - nxl - nmsl;
    if nch > ship_chr.nchoppers as i32 {
        nfw += nch - ship_chr.nchoppers as i32;
    }
    if nxl > ship_chr.nxlight as i32 {
        nmsl += nxl - ship_chr.nxlight as i32;
    }
    if nmsl > 0 && !ship_chr.flags.intersects(ShipChrFlags::MISSILE | ShipChrFlags::CARRIER) {
        return false;
    }
    if nfw > 0 && !ship_chr.flags.contains(ShipChrFlags::CARRIER) {
        return false;
    }
    n = nfw + nmsl;
    n <= ship_chr.nplanes as i32
}

/// Friendly CARRIER-flagged ships at (x, y), efficient enough for air
/// ops, sorted by uid. Used only by the flight-landing path -- `load`
/// does its own broader filtering since it must also reach
/// MISSILE-flagged ships (missile subs), which never qualify here.
pub fn eligible_carriers<'a>(
    ships: &'a [Ship],
    ship_chrs: &[ShipChr],
    cnum: NatId,
    is_deity: bool,
    x: Coord,
    y: Coord,
) -> Vec<&'a Ship> {
    let mut carriers: Vec<&Ship> = ships
        .iter()
        .filter(|s| s.x == x && s.y == y)
        .filter(|s| s.own == cnum || is_deity)
        .filter(|s| s.effic >= SHIP_AIROPS_EFF)
        .filter(|s| {
            ship_chrs
                .get(s.ship_type as usize)
                .map(|c| c.flags.contains(ShipChrFlags::CARRIER))
                .unwrap_or(false)
        })
        .collect();
    carriers.sort_by_key(|s| s.uid);
    carriers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plane(plane_type: i8) -> Plane {
        use empire_types::plane::PlaneFlags;
        Plane {
            uid: 0, own: 1, x: 0, y: 0, plane_type,
            effic: 100, mobil: 30, off: false, tech: 200,
            wing: ' ', opx: 0, opy: 0, mission: 0, mission_radius: 0,
            range: 10, harden: 0, ship: -1, land: -1,
            flags: PlaneFlags::empty(), access: 0, theta: 0.0,
        }
    }

    #[test]
    fn light_plane_fits_on_carrier() {
        let chrs = PlaneChr::all();
        let ship_chrs = ShipChr::all();
        // type 2: F-4 Phantom (jf1) is LIGHT; type 21: aircraft carrier is CARRIER
        let carrier = &ship_chrs[21];
        assert!(ship_can_carry(carrier, &[], chrs, chrs[2].flags));
    }

    #[test]
    fn missile_fits_on_missile_sub_but_not_plain_ship() {
        let chrs = PlaneChr::all();
        let ship_chrs = ShipChr::all();
        // type 32 (slbm Trident) is MISSILE|LIGHT (a real sub-launched
        // missile, unlike the silo-based Atlas/Titan/Minuteman ICBMs
        // which aren't LIGHT-flagged and so can't be shipborne at all).
        // type 27: nuc miss sub is MISSILE-flagged, not CARRIER.
        let sub = &ship_chrs[27];
        assert!(ship_can_carry(sub, &[], chrs, chrs[32].flags));

        // A plain non-carrier, non-missile ship (type 0: fishing boat) can't.
        let plain = &ship_chrs[0];
        assert!(!ship_can_carry(plain, &[], chrs, chrs[32].flags));
    }

    #[test]
    fn non_light_plane_never_fits() {
        let chrs = PlaneChr::all();
        let ship_chrs = ShipChr::all();
        // type 23: E2-C Hawkeye (spy only, no LIGHT/XLIGHT/HELO/MISSILE)
        let carrier = &ship_chrs[21];
        assert!(!ship_can_carry(carrier, &[], chrs, chrs[23].flags));
    }

    #[test]
    fn chopper_overflow_spills_into_fixed_wing_and_needs_carrier() {
        let chrs = PlaneChr::all();
        let ship_chrs = ShipChr::all();
        // type 16: AH-1 Cobra is HELO. nuc carrier (22) has nchoppers=4.
        let carrier = &ship_chrs[22];
        let existing: Vec<Plane> = (0..4).map(|_| make_plane(16)).collect();
        // A 5th chopper overflows into the fixed-wing bucket, which the
        // carrier (CARRIER-flagged) can still absorb given ample nplanes.
        assert!(ship_can_carry(carrier, &existing, chrs, chrs[16].flags));
    }

    #[test]
    fn nplanes_capacity_is_enforced() {
        let chrs = PlaneChr::all();
        let ship_chrs = ShipChr::all();
        // light carrier (20) has nplanes=20.
        let carrier = &ship_chrs[20];
        let existing: Vec<Plane> = (0..20).map(|_| make_plane(2)).collect();
        assert!(!ship_can_carry(carrier, &existing, chrs, chrs[2].flags));
    }
}
