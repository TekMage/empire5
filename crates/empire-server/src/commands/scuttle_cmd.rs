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
// Ported from: src/lib/commands/scut.c

// "scuttle" command — destroy a ship, plane, or land unit outright, no
// location or efficiency requirement (unlike 'scrap' -- see info scrap
// for the salvage-with-recovery alternative). Cargo aboard spills into
// the current sector if you still own it there; otherwise it's lost.
//
// Usage: scuttle s <ship-spec>
//        scuttle p <plane-spec>
//        scuttle l <land-spec>
//
// Land units currently aboard a ship can't be scuttled (matches
// 4.4.1 -- disembark with 'unload' first).
//
// v1 gap: 4.4.1 pays real cash for scuttling a trade ship (M_TRADE),
// scaled by distance sailed from its origin port -- that's the actual
// trade-ship profit mechanic, distinct from the opt_MARKET trading
// block. This version doesn't have the trade-distance economy ported
// (rate constants, etc.) yet, so scuttling a trade ship here just
// destroys it for nothing, same as any other ship.

use empire_db::{land_units, planes, sectors, ships};
use empire_types::commodity::Item;
use empire_types::land_chr::LandChr;
use empire_types::plane_chr::PlaneChr;
use empire_types::ship_chr::ShipChr;

use super::ctx::CmdCtx;
use crate::subs::lndsub::land_spec_matches;
use crate::subs::plnsub::plane_spec_matches;
use crate::subs::shpsub::ship_spec_matches;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: scuttle <s|p|l> <spec>\n".to_string();
    }
    let kind = parts[0];
    let spec = parts[1];

    match kind.chars().next() {
        Some('s') => scuttle_ships(spec, ctx).await,
        Some('p') => scuttle_planes(spec, ctx).await,
        Some('l') => scuttle_land(spec, ctx).await,
        _ => "10 Ships, land units, or planes only! (s, l, p)\n".to_string(),
    }
}

async fn scuttle_ships(spec: &str, ctx: &CmdCtx<'_>) -> String {
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
        let Some(chr) = chrs.get(ship.ship_type as usize) else { continue };

        if let Ok(Some(mut sect)) = sectors::get_at(ctx.db, ship.x, ship.y).await {
            if sect.own == ship.own {
                let cargo = ship.items.clone();
                for item in ALL_ITEMS {
                    let amt = cargo.get(item);
                    if amt > 0 { sect.items.add(item, amt); }
                }
                let _ = sectors::put(ctx.db, &sect).await;
            }
        }
        ship.effic = 0;
        if let Err(e) = ships::put(ctx.db, &ship).await {
            out.push_str(&format!("1 Ship #{} save error: {e}\n", ship.uid));
            continue;
        }
        out.push_str(&format!("1 {} #{} scuttled in {}\n", chr.name, ship.uid, ctx.format_xy(ship.x, ship.y)));
        n += 1;
    }

    if n == 0 { out.push_str("1 No matching ships scuttled.\n"); }
    out.push_str("0 scuttle\n");
    out
}

async fn scuttle_planes(spec: &str, ctx: &CmdCtx<'_>) -> String {
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
        let Some(chr) = chrs.get(plane.plane_type as usize) else { continue };

        plane.effic = 0;
        if let Err(e) = planes::put(ctx.db, &plane).await {
            out.push_str(&format!("1 Plane #{} save error: {e}\n", plane.uid));
            continue;
        }
        out.push_str(&format!("1 {} #{} scuttled in {}\n", chr.name, plane.uid, ctx.format_xy(plane.x, plane.y)));
        n += 1;
    }

    if n == 0 { out.push_str("1 No matching planes scuttled.\n"); }
    out.push_str("0 scuttle\n");
    out
}

async fn scuttle_land(spec: &str, ctx: &CmdCtx<'_>) -> String {
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
        if unit.ship >= 0 {
            out.push_str(&format!("1 Land unit #{} is on a ship, and cannot be scuttled!\n", unit.uid));
            continue;
        }
        let Some(chr) = chrs.get(unit.land_type as usize) else { continue };

        if let Ok(Some(mut sect)) = sectors::get_at(ctx.db, unit.x, unit.y).await {
            if sect.own == unit.own {
                let cargo = unit.items.clone();
                for item in ALL_ITEMS {
                    let amt = cargo.get(item);
                    if amt > 0 { sect.items.add(item, amt); }
                }
                let _ = sectors::put(ctx.db, &sect).await;
            }
        }
        unit.effic = 0;
        if let Err(e) = land_units::put(ctx.db, &unit).await {
            out.push_str(&format!("1 Land unit #{} save error: {e}\n", unit.uid));
            continue;
        }
        out.push_str(&format!("1 {} #{} scuttled in {}\n", chr.name, unit.uid, ctx.format_xy(unit.x, unit.y)));
        n += 1;
    }

    if n == 0 { out.push_str("1 No matching land units scuttled.\n"); }
    out.push_str("0 scuttle\n");
    out
}

const ALL_ITEMS: [Item; 14] = [
    Item::Civil, Item::Milit, Item::Shell, Item::Gun, Item::Petrol,
    Item::Iron, Item::Dust, Item::Bar, Item::Food, Item::Oil,
    Item::Lcm, Item::Hcm, Item::Uw, Item::Rad,
];
