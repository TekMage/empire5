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
// Ported from: src/lib/commands/look.c (do_look/look_ship/look_land)

// Shared core for the "look"/"lookout" (ship) and "llook"/"llookout" (land
// unit) commands: reveal the immediate 7-hex patch around a lookout unit
// (self + 6 neighbors, non-water only), and separately report any nearby
// *foreign* ships/land units/planes within visual range. Pure logic --
// callers own the DB round-trips and bmap persistence.

use std::collections::HashMap;
use rand::Rng;

use empire_types::commodity::Item;
use empire_types::coords::Coord;
use empire_types::land::LandUnit;
use empire_types::land_chr::{LandChr, LandChrFlags};
use empire_types::plane::{Plane, PlaneFlags};
use empire_types::plane_chr::PlaneChr;
use empire_types::sector::{Sector, SectorType};
use empire_types::sector_chr::SectorChr;
use empire_types::ship::Ship;
use empire_types::ship_chr::{ShipChr, ShipChrFlags};

use empire_db::bmap::Bmap;

use super::geo::{map_dist, neighbors};
use super::satsub::round_int_by;
use super::shpsub::shp_visib;
use super::tech::techfact;
use crate::commands::ctx::CmdCtx;

/// Max radius to scan for nearby foreign ships/units to report -- mirrors
/// 4.4.1's `ship_max_interdiction_range` (8), reused here for land lookout
/// too since the reference doesn't define a separate constant for it.
pub const LOOK_MAX_RANGE: i32 = 8;

/// Reveal the 7-hex patch (self + 6 neighbors) around (cx,cy) into `bm`,
/// skipping water -- matches `do_look()`'s `if (sect.sct_type==SCT_WATER)
/// continue` (water is neither reported nor bmap-updated by 'look'). Returns
/// one report line per non-water tile, mirroring `look_at_sect()`.
pub fn look_neighbors(
    ctx: &CmdCtx<'_>,
    all_sectors: &[Sector],
    coord_map: &HashMap<(Coord, Coord), usize>,
    cx: Coord, cy: Coord,
    bm: &mut Bmap,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut pts = vec![(cx, cy)];
    pts.extend(neighbors(cx, cy, ctx.world_x, ctx.world_y));

    for (x, y) in pts {
        let Some(&idx) = coord_map.get(&(x, y)) else { continue };
        let s = &all_sectors[idx];
        if s.sector_type == SectorType::Sea { continue; }
        out.push(look_at_sect(ctx, s));
        bm.set(x, y, s.sector_type.mnemonic() as u8);
    }
    out
}

/// One sector's report line, own sectors shown exactly, foreign ones
/// rounded to the nearest 10 -- matches `look_at_sect(&sect, 10)`.
fn look_at_sect(ctx: &CmdCtx<'_>, s: &Sector) -> String {
    let ours = s.own == ctx.cnum;
    let who = if ours { "Your".to_string() } else { format!("Nation #{}'s", s.own) };
    let eff = if ours { s.effic as i32 } else { round_int_by(s.effic as i32, 10) };
    let civ = s.items.get(Item::Civil) as i32;
    let mil = s.items.get(Item::Milit) as i32;

    let mut line = format!("{who} {} {eff}% efficient ", SectorChr::for_type(s.sector_type).name);
    if civ > 0 {
        let shown = if ours { civ } else { round_int_by(civ, 10) };
        line.push_str(&format!("with {}{shown} civ ", if ours { "" } else { "approx " }));
    }
    if mil > 0 {
        let shown = if ours { mil } else { round_int_by(mil, 10) };
        line.push_str(&format!("with {}{shown} mil ", if ours { "" } else { "approx " }));
    }
    line.push_str(&format!("@ {}", ctx.format_xy(s.x, s.y)));
    line
}

/// Enemy-ship contact report for a ship-sourced lookout. Mirrors
/// `look_ship()`: range comes from the looker's own vrnge/tech/efficiency
/// (subs get their range capped to 1), a target's visibility to us scales
/// with its own `shp_visib`, and submerged subs are invisible to a plain
/// look (only a submarine at dist 0, i.e. surfaced, or sonar can find one).
pub fn look_ship_contacts(
    ctx: &CmdCtx<'_>,
    all_ships: &[Ship],
    all_sectors: &[Sector],
    coord_map: &HashMap<(Coord, Coord), usize>,
    looker: &Ship,
    looker_chr: &ShipChr,
) -> Vec<String> {
    let mut out = Vec::new();
    let is_sub = looker_chr.flags.contains(ShipChrFlags::SUBMARINE);

    let mut range = techfact(looker_chr.vrnge as f64, looker.tech as f64) * (looker.effic as f64 / 100.0);
    if is_sub {
        range = range.min(1.0);
    }

    for sp in all_ships {
        if sp.own == ctx.cnum || sp.own == 0 { continue; }
        let dist = map_dist(sp.x, sp.y, looker.x, looker.y, ctx.world_x, ctx.world_y);
        if dist > LOOK_MAX_RANGE { continue; }

        let Some(tchr) = ShipChr::for_type(sp.ship_type as usize) else { continue };
        let visib = shp_visib(sp, tchr);
        let divisor = if is_sub { 30.0 } else { 20.0 };
        let mut vrange = visib * range / divisor;

        let target_is_water = coord_map.get(&(sp.x, sp.y))
            .map(|&i| all_sectors[i].sector_type == SectorType::Sea)
            .unwrap_or(true);
        if !target_is_water {
            vrange = vrange.max(1.0);
        }
        if (dist as f64) > vrange { continue; }

        // Subs at sea are only found by sonar, not a plain look.
        if tchr.flags.contains(ShipChrFlags::SUBMARINE) && target_is_water { continue; }

        out.push(format!("Nation #{} {} @ {}", sp.own, tchr.name, ctx.format_xy(sp.x, sp.y)));
    }
    out
}

/// Enemy land-unit and plane contact report for a land-unit-sourced
/// lookout. Mirrors `look_land()`: range from `techfact(tech, l_spy) *
/// effic/100`; land targets use their own (tech-independent) `l_vis` stat,
/// spies are probabilistically skipped (`LND_SPY_DETECT_CHANCE`); planes
/// use a fixed visibility of 10 and must be grounded (not launched, not
/// stowed).
#[allow(clippy::too_many_arguments)]
pub fn look_land_contacts(
    ctx: &CmdCtx<'_>,
    all_units: &[LandUnit],
    all_planes: &[Plane],
    looker: &LandUnit,
    looker_chr: &LandChr,
    rng: &mut impl Rng,
) -> Vec<String> {
    let mut out = Vec::new();

    let drange = techfact(looker_chr.spy as f64, looker.tech as f64) * (looker.effic as f64 / 100.0);
    let range = drange.round() as i32;
    if range == 0 {
        return out;
    }

    for lp in all_units {
        if lp.own == ctx.cnum || lp.own == 0 { continue; }
        if lp.ship >= 0 || lp.carried_by_land >= 0 { continue; }

        let Some(tchr) = LandChr::for_type(lp.land_type as usize) else { continue };
        if tchr.flags.contains(LandChrFlags::SPY) {
            let detect_chance = (110.0 - lp.effic as f64) / 100.0;
            if !rng.gen_bool(detect_chance.clamp(0.0, 1.0)) { continue; }
        }

        let vrange = ((tchr.vis * range) as f64 / 20.0).round() as i32;
        let dist = map_dist(lp.x, lp.y, looker.x, looker.y, ctx.world_x, ctx.world_y);
        if dist > vrange { continue; }

        let mil = lp.items.get(Item::Milit) as i32;
        out.push(format!(
            "Nation #{} {} (approx {} mil) @ {}",
            lp.own, tchr.name, round_int_by(mil, 20), ctx.format_xy(lp.x, lp.y)
        ));
    }

    for pp in all_planes {
        if pp.own == ctx.cnum || pp.own == 0 { continue; }
        if pp.ship >= 0 || pp.land >= 0 { continue; }
        if pp.flags.contains(PlaneFlags::LAUNCHED) { continue; }

        let Some(pchr) = PlaneChr::for_type(pp.plane_type as usize) else { continue };
        let vrange = ((10 * range) as f64 / 20.0).round() as i32;
        let dist = map_dist(pp.x, pp.y, looker.x, looker.y, ctx.world_x, ctx.world_y);
        if dist > vrange { continue; }

        out.push(format!("Nation #{} {} @ {}", pp.own, pchr.name, ctx.format_xy(pp.x, pp.y)));
    }

    out
}
