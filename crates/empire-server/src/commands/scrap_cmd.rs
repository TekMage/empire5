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
// Ported from: src/lib/commands/scra.c

// "scrap" command — destroy a ship, plane, or land unit and recover
// some of its build materials into the sector it's standing in.
//
// Usage: scrap s <ship-spec>
//        scrap p <plane-spec>
//        scrap l <land-spec>
//
// Ships must be in a friendly harbor at >=60% efficiency; planes in a
// friendly airfield at >=60% efficiency; land units in a friendly
// sector (same-owner, matching this codebase's existing "friendly"
// simplification elsewhere -- see fly.rs/recon_cmd.rs). Recovers 2/3
// of the unit's lcm/hcm build cost, scaled by its current efficiency,
// dumped into the sector -- this codebase's build-cost model only
// tracks lcm/hcm (see build.rs), so that's all that comes back, unlike
// 4.4.1's full per-item material vector.
//
// Ships: setting effic to 0 sinks it -- empire_db::ships::put() clears
// ownership below SHIP_MINEFF (20%), matching 4.4.1's shp_prewrite()
// hook, so it's gone from the fleet immediately and prod_ships (which
// skips own==0) will never revive it. Planes and land units have no
// such rule (confirmed against 4.4.1's planerepair()/landrepair() --
// no special-case for 0%), so they persist and *will* rebuild from
// scratch if their sector still has LCM/HCM flowing to it. Scrapping
// one of those you don't want back means also cutting its supply, or
// scrapping it somewhere with none.

use empire_db::{land_units, planes, sectors, ships};
use empire_types::commodity::Item;
use empire_types::land_chr::LandChr;
use empire_types::plane_chr::PlaneChr;
use empire_types::sector::SectorType;
use empire_types::ship_chr::ShipChr;

use super::ctx::CmdCtx;
use crate::subs::lndsub::land_spec_matches;
use crate::subs::plnsub::plane_spec_matches;
use crate::subs::shpsub::ship_spec_matches;

/// 2/3 of build cost recovered, scaled by current efficiency -- ported
/// from scra.c's `mvec[i] * 2 / 3 * eff` (float math, truncated on
/// assignment back into an integer sector item count).
fn recovered(build_cost: i32, effic: i8) -> i16 {
    ((build_cost as f64) * (2.0 / 3.0) * (effic as f64 / 100.0)) as i16
}

fn drop_cargo(sect_items: &mut empire_types::commodity::Inventory, cargo: &empire_types::commodity::Inventory) {
    for item in [
        Item::Civil, Item::Milit, Item::Shell, Item::Gun, Item::Petrol,
        Item::Iron, Item::Dust, Item::Bar, Item::Food, Item::Oil,
        Item::Lcm, Item::Hcm, Item::Uw, Item::Rad,
    ] {
        let amt = cargo.get(item);
        if amt > 0 {
            sect_items.add(item, amt);
        }
    }
}

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: scrap <s|p|l> <spec>\n".to_string();
    }
    let kind = parts[0];
    let spec = parts[1];

    match kind.chars().next() {
        Some('s') => scrap_ships(spec, ctx).await,
        Some('p') => scrap_planes(spec, ctx).await,
        Some('l') => scrap_land(spec, ctx).await,
        _ => "10 Ships, land units, or planes only! (s, l, p)\n".to_string(),
    }
}

async fn scrap_ships(spec: &str, ctx: &CmdCtx<'_>) -> String {
    let all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let chrs = ShipChr::all();
    let mut out = String::new();
    let mut n = 0u32;

    for mut ship in all_ships {
        if ship.own != ctx.cnum && !ctx.is_deity { continue; }
        if !ship_spec_matches(spec, &ship) { continue; }

        let Some(sect) = (match sectors::get_at(ctx.db, ship.x, ship.y).await {
            Ok(v) => v,
            Err(e) => { out.push_str(&format!("1 DB error: {e}\n")); continue; }
        }) else {
            out.push_str(&format!("1 Ship #{} is not in any sector\n", ship.uid));
            continue;
        };
        if sect.sector_type != SectorType::Harbor || sect.effic < 60
            || (sect.own != ctx.cnum && !ctx.is_deity)
        {
            out.push_str(&format!(
                "1 Ship #{} is not in a friendly 60%+ efficient harbor\n", ship.uid
            ));
            continue;
        }
        let Some(chr) = chrs.get(ship.ship_type as usize) else { continue };

        let mut sect = sect;
        drop_cargo(&mut sect.items, &ship.items);
        sect.items.add(Item::Lcm, recovered(chr.lcm, ship.effic));
        sect.items.add(Item::Hcm, recovered(chr.hcm, ship.effic));
        ship.effic = 0;

        if let Err(e) = sectors::put(ctx.db, &sect).await {
            out.push_str(&format!("1 Sector save error: {e}\n"));
            continue;
        }
        if let Err(e) = ships::put(ctx.db, &ship).await {
            out.push_str(&format!("1 Ship #{} save error: {e}\n", ship.uid));
            continue;
        }
        out.push_str(&format!("1 {} #{} scrapped in {}\n", chr.name, ship.uid, ctx.format_xy(sect.x, sect.y)));
        n += 1;
    }

    if n == 0 { out.push_str("1 No matching ships scrapped.\n"); }
    out.push_str("0 scrap\n");
    out
}

async fn scrap_planes(spec: &str, ctx: &CmdCtx<'_>) -> String {
    let all_planes = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let chrs = PlaneChr::all();
    let mut out = String::new();
    let mut n = 0u32;

    for mut plane in all_planes {
        if plane.own != ctx.cnum && !ctx.is_deity { continue; }
        if !plane_spec_matches(spec, &plane) { continue; }

        let Some(sect) = (match sectors::get_at(ctx.db, plane.x, plane.y).await {
            Ok(v) => v,
            Err(e) => { out.push_str(&format!("1 DB error: {e}\n")); continue; }
        }) else {
            out.push_str(&format!("1 Plane #{} is not in any sector\n", plane.uid));
            continue;
        };
        if sect.sector_type != SectorType::Airfield || sect.effic < 60
            || (sect.own != ctx.cnum && !ctx.is_deity)
        {
            out.push_str(&format!(
                "1 Plane #{} is not in a friendly 60%+ efficient airfield\n", plane.uid
            ));
            continue;
        }
        let Some(chr) = chrs.get(plane.plane_type as usize) else { continue };

        let mut sect = sect;
        sect.items.add(Item::Lcm, recovered(chr.lcm, plane.effic));
        sect.items.add(Item::Hcm, recovered(chr.hcm, plane.effic));
        plane.effic = 0;

        if let Err(e) = sectors::put(ctx.db, &sect).await {
            out.push_str(&format!("1 Sector save error: {e}\n"));
            continue;
        }
        if let Err(e) = planes::put(ctx.db, &plane).await {
            out.push_str(&format!("1 Plane #{} save error: {e}\n", plane.uid));
            continue;
        }
        out.push_str(&format!("1 {} #{} scrapped in {}\n", chr.name, plane.uid, ctx.format_xy(sect.x, sect.y)));
        n += 1;
    }

    if n == 0 { out.push_str("1 No matching planes scrapped.\n"); }
    out.push_str("0 scrap\n");
    out
}

async fn scrap_land(spec: &str, ctx: &CmdCtx<'_>) -> String {
    let all_units = match land_units::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let chrs = LandChr::all();
    let mut out = String::new();
    let mut n = 0u32;

    for mut unit in all_units {
        if unit.own != ctx.cnum && !ctx.is_deity { continue; }
        if !land_spec_matches(spec, &unit) { continue; }

        let Some(sect) = (match sectors::get_at(ctx.db, unit.x, unit.y).await {
            Ok(v) => v,
            Err(e) => { out.push_str(&format!("1 DB error: {e}\n")); continue; }
        }) else {
            out.push_str(&format!("1 Land unit #{} is not in any sector\n", unit.uid));
            continue;
        };
        if sect.own != ctx.cnum && !ctx.is_deity {
            out.push_str(&format!("1 Land unit #{} is not in a friendly sector\n", unit.uid));
            continue;
        }
        let Some(chr) = chrs.get(unit.land_type as usize) else { continue };

        let mut sect = sect;
        drop_cargo(&mut sect.items, &unit.items);
        sect.items.add(Item::Lcm, recovered(chr.lcm, unit.effic));
        sect.items.add(Item::Hcm, recovered(chr.hcm, unit.effic));
        unit.effic = 0;

        if let Err(e) = sectors::put(ctx.db, &sect).await {
            out.push_str(&format!("1 Sector save error: {e}\n"));
            continue;
        }
        if let Err(e) = land_units::put(ctx.db, &unit).await {
            out.push_str(&format!("1 Land unit #{} save error: {e}\n", unit.uid));
            continue;
        }
        out.push_str(&format!("1 {} #{} scrapped in {}\n", chr.name, unit.uid, ctx.format_xy(sect.x, sect.y)));
        n += 1;
    }

    if n == 0 { out.push_str("1 No matching land units scrapped.\n"); }
    out.push_str("0 scrap\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovered_at_full_efficiency() {
        // 2/3 of build cost at 100% efficiency, truncated.
        assert_eq!(recovered(60, 100), 40);
        assert_eq!(recovered(10, 100), 6); // 6.666.. truncates to 6
    }

    #[test]
    fn recovered_scales_with_efficiency() {
        assert_eq!(recovered(60, 50), 20); // half efficiency halves the recovery
        assert_eq!(recovered(60, 0), 0);
    }
}
