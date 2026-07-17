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
// Ported from: src/lib/subs/shpsub.c (shp_interdict, shp_fort_interdiction,
//              notify_coastguard)

// Passive coastal defense against ships in transit — the "gauntlet" that
// runs after every step of `navigate`, with no attack command needed:
//
//   - Fort auto-fire: any Fortress sector whose owner is Hostile-or-worse
//     toward the ship's owner, in gun range, opens fire automatically.
//     Matches shp_fort_interdiction()'s "Only fire at Hostile ships" pass.
//     No radar or prior detection is required to fire — 4.4.1's fort_fire()
//     gates only on range and relations, not visibility.
//   - Coastwatch sightings: separately, any nation with the coastwatch flag
//     on and a sector within vision range (baseline 4 hexes, 14 for a radar
//     sector, tech/effic-scaled) gets a telegram when a Neutral-or-worse
//     ship passes by — matches notify_coastguard()'s "Inform neutral and
//     worse" pass. This is pure intel and does not require Hostile relations
//     or gate the fort-fire pass in any way.
//
// Submarines are invisible to both passes entirely (4.4.1 excludes M_SUB
// ships from `shp_interdict`'s ship-list scan before either pass runs).

use rand::rngs::StdRng;
use rand::SeedableRng;

use empire_db::relations::Relation;
use empire_db::telegrams::TEL_NORM;
use empire_db::{nations, news, relations, sectors, telegrams, Db};
use empire_types::commodity::Item;
use empire_types::nation::NatFlags;
use empire_types::news::NewsVerb;
use empire_types::sector::{Sector, SectorType};
use empire_types::ship::Ship;
use empire_types::ship_chr::{ShipChr, ShipChrFlags};

use crate::subs::fortsub::{fort_can_fire, fort_gun_range, fortgun_damage};
use crate::subs::geo::{format_xy, map_dist};
use crate::subs::nat_util::format_nat;
use crate::subs::shpsub::shp_damage;
use crate::subs::tech::techfact;

/// Max radius (hexes) to scan for candidate forts — mirrors 4.4.1's
/// `fort_max_interdiction_range` constant (8).
pub const FORT_MAX_INTERDICTION_RANGE: i32 = 8;

/// Result of running the gauntlet at the ship's current position.
pub struct InterdictOutcome {
    /// Total damage the ship took from firing forts this step (0 if none fired).
    pub total_dam: i32,
    /// True if the ship was sunk by that damage.
    pub sunk: bool,
}

/// Run the passive coastal-defense gauntlet against `ship` at its current
/// (already-moved-to) position. Scans `all_sectors` for eligible hostile
/// forts and coastwatching nations, deducts shells/persists changed forts,
/// files news and telegrams, and applies pooled fort damage to `ship`.
pub async fn run(
    db: &Db,
    all_sectors: &mut [Sector],
    ship: &mut Ship,
    world_x: i32,
    world_y: i32,
) -> InterdictOutcome {
    let no_dam = InterdictOutcome { total_dam: 0, sunk: false };

    let Some(mchr) = ShipChr::for_type(ship.ship_type as usize) else {
        return no_dam;
    };
    if mchr.flags.contains(ShipChrFlags::SUBMARINE) {
        return no_dam; // subs are invisible to coastal defense entirely
    }

    coastwatch_sightings(db, all_sectors, ship, world_x, world_y).await;

    let total_dam = fort_gauntlet(db, all_sectors, ship, world_x, world_y).await;
    if total_dam <= 0 {
        return no_dam;
    }
    let sunk = shp_damage(ship, total_dam, mchr.armor);
    InterdictOutcome { total_dam, sunk }
}

/// "Only fire at Hostile ships" pass — accumulates damage from every
/// eligible fort in range, deducting a shell and persisting each one.
async fn fort_gauntlet(
    db: &Db,
    all_sectors: &mut [Sector],
    ship: &Ship,
    world_x: i32,
    world_y: i32,
) -> i32 {
    let mut firing: Vec<usize> = Vec::new();

    for (idx, sect) in all_sectors.iter().enumerate() {
        if sect.sector_type != SectorType::Fortress { continue; }
        if sect.own == 0 || sect.own == ship.own { continue; }
        if !fort_can_fire(sect) { continue; }

        let relation = relations::get(db, sect.own, ship.own).await.unwrap_or(Relation::Neutral);
        if relation > Relation::Hostile { continue; }

        let dist = map_dist(sect.x, sect.y, ship.x, ship.y, world_x, world_y);
        if dist > FORT_MAX_INTERDICTION_RANGE { continue; }

        let owner_tech = match nations::get_by_cnum(db, sect.own).await {
            Ok(Some(n)) => n.tech,
            _ => 0.0,
        };
        let range = fort_gun_range(sect.effic, owner_tech);
        if dist > range { continue; }

        firing.push(idx);
    }

    if firing.is_empty() {
        return 0;
    }

    let mut rng = StdRng::from_entropy();
    let mut total_dam = 0i32;
    for idx in firing {
        let sect = &mut all_sectors[idx];
        let guns = sect.items.get(Item::Gun) as i32;
        let dam = fortgun_damage(sect.effic, guns, &mut rng);
        sect.items.add(Item::Shell, -1);
        total_dam += dam;

        let fort_own = sect.own;
        let _ = sectors::put(db, sect).await;
        let _ = news::add_news(db, fort_own, NewsVerb::ShpShell as u8, ship.own, 1).await;
    }

    total_dam
}

/// "Inform neutral and worse" pass — tells any coastwatching nation with a
/// sector in vision range that a ship passed by, at most once per nation
/// per step. Purely informational: doesn't require Hostile relations and
/// never blocks or damages the ship.
async fn coastwatch_sightings(
    db: &Db,
    all_sectors: &[Sector],
    ship: &Ship,
    world_x: i32,
    world_y: i32,
) {
    let mut notified: Vec<u8> = Vec::new();

    for sect in all_sectors {
        if sect.own == 0 || sect.own == ship.own { continue; }
        if notified.contains(&sect.own) { continue; }

        let relation = relations::get(db, sect.own, ship.own).await.unwrap_or(Relation::Neutral);
        if relation > Relation::Neutral { continue; }

        let Ok(Some(watcher)) = nations::get_by_cnum(db, sect.own).await else { continue; };
        if !watcher.flags.contains(NatFlags::COASTWATCH) { continue; }

        let base = if sect.sector_type == SectorType::Radar { 14.0 } else { 4.0 };
        let vrange = ((base * techfact(1.0, watcher.tech) * sect.effic as f64 / 100.0) as i32).max(1);
        let dist = map_dist(sect.x, sect.y, ship.x, ship.y, world_x, world_y);
        if dist > vrange { continue; }

        notified.push(sect.own);

        let Ok(Some(owner)) = nations::get_by_cnum(db, ship.own).await else { continue; };
        let mchr_name = ShipChr::for_type(ship.ship_type as usize).map(|c| c.name).unwrap_or("ship");
        let body = format!(
            "{} {} sighted at {}",
            format_nat(&owner), mchr_name,
            format_xy(&watcher, ship.x, ship.y, world_x, world_y),
        );
        let _ = telegrams::send(db, watcher.cnum, 0, TEL_NORM, &body).await;
    }
}
