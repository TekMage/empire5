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
// Ported from: src/lib/commands/load.c

// "load"/"unload" commands — transfer commodities between a ship and
// the harbor sector it is docked in, or (see load_plane below) put
// planes aboard/off a carrier or missile sub.
//
// Usage: load  <commodity> <ship-spec> <amount>
//        unload <commodity> <ship-spec> <amount>
//        load  plane <ship-spec> <plane-spec>
//        unload plane <ship-spec> <plane-spec>
//
// The ship must be in a harbor (type 'h') sector owned by the player,
// and that harbor must be ≥2% efficient. Loading/unloading *planes*
// (see load_plane) has no harbor requirement -- only commodities do.

use empire_db::{planes, sectors, ships};
use empire_types::commodity::Item;
use empire_types::plane::Plane;
use empire_types::plane_chr::PlaneChr;
use empire_types::sector::SectorType;
use empire_types::ship::Ship;
use empire_types::ship_chr::ShipChr;
use super::ctx::CmdCtx;
use crate::subs::shipcarry;
use crate::subs::shpsub::ship_spec_matches;
use crate::subs::plnsub::plane_spec_matches;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    run_inner(args, ctx, false).await
}

pub async fn run_unload(args: &str, ctx: &CmdCtx<'_>) -> String {
    run_inner(args, ctx, true).await
}

async fn run_inner(args: &str, ctx: &CmdCtx<'_>, unload: bool) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 3 {
        let cmd = if unload { "unload" } else { "load" };
        return format!("10 Usage: {cmd} <commodity> <ship-spec> <amount>\n");
    }

    if parts[0].eq_ignore_ascii_case("plane") {
        return run_plane_load(&parts, ctx, unload).await;
    }

    let item = match Item::from_mnemonic(parts[0].chars().next().unwrap_or(' ')) {
        Some(i) => i,
        None => {
            // Try full name prefix match
            match parse_item_name(parts[0]) {
                Some(i) => i,
                None => return format!("10 Unknown commodity '{}'\n", parts[0]),
            }
        }
    };

    let ship_spec = parts[1];
    let amount: i16 = match parts[2].parse() {
        Ok(n) => n,
        Err(_) => return format!("10 Invalid amount '{}'\n", parts[2]),
    };
    if amount <= 0 {
        return "10 Amount must be positive\n".to_string();
    }

    let all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let matching_ships: Vec<_> = all_ships.into_iter()
        .filter(|s| s.own == ctx.cnum || ctx.is_deity)
        .filter(|s| matches_ship(s.uid, ship_spec))
        .collect();

    if matching_ships.is_empty() {
        return format!("1 No ships match '{}'\n0 {}\n",
            ship_spec, if unload { "unload" } else { "load" });
    }

    let mut out = String::new();
    let cmd_name = if unload { "unload" } else { "load" };

    for mut ship in matching_ships {
        let sx = ship.x;
        let sy = ship.y;

        // Find the harbor sector at the ship's location
        let harbor = all_sectors.iter().find(|s| s.x == sx && s.y == sy);
        let harbor = match harbor {
            Some(h) => h.clone(),
            None => {
                out.push_str(&format!("1 Ship {} not in any sector\n", ship.uid));
                continue;
            }
        };

        if harbor.sector_type != SectorType::Harbor {
            let xy = ctx.format_xy(sx, sy);
            out.push_str(&format!(
                "1 {xy} is not a harbor (ship {} must be docked to load/unload)\n",
                ship.uid
            ));
            continue;
        }
        if harbor.own != ctx.cnum && !ctx.is_deity {
            let xy = ctx.format_xy(sx, sy);
            out.push_str(&format!("1 {xy} is not your harbor\n"));
            continue;
        }
        if harbor.effic < 2 {
            let xy = ctx.format_xy(sx, sy);
            out.push_str(&format!("1 {xy} harbor efficiency too low (need ≥2%)\n"));
            continue;
        }

        let mut sect = harbor;
        let ship_have = ship.items.get(item);
        let sect_have = sect.items.get(item);

        // Get per-commodity cargo limit for this ship type
        let ship_cap = ShipChr::for_type(ship.ship_type as usize)
            .map(|c| c.cargo_cap(item))
            .unwrap_or(0);

        let actual = if unload {
            // Move from ship to sector
            let can_move = ship_have.min(amount);
            if can_move <= 0 {
                out.push_str(&format!("1 Ship {} has no {}\n", ship.uid, item.name()));
                continue;
            }
            ship.items.set(item, ship_have - can_move);
            sect.items.set(item, sect_have + can_move);
            can_move
        } else {
            // Move from sector to ship — enforce cargo capacity
            let type_name = ShipChr::for_type(ship.ship_type as usize)
                .map(|c| c.name)
                .unwrap_or("ship");
            if ship_cap == 0 {
                out.push_str(&format!(
                    "1 Ship {} ({}) cannot carry {}\n", ship.uid, type_name, item.name()
                ));
                continue;
            }
            let room = (ship_cap - ship_have).max(0);
            if room == 0 {
                out.push_str(&format!(
                    "1 Ship {} {} full (cap {} {})\n",
                    ship.uid, type_name, ship_cap, item.name()
                ));
                continue;
            }
            let can_move = sect_have.min(amount).min(room);
            if can_move <= 0 {
                let xy = ctx.format_xy(sx, sy);
                out.push_str(&format!("1 {xy} has no {}\n", item.name()));
                continue;
            }
            sect.items.set(item, sect_have - can_move);
            ship.items.set(item, ship_have + can_move);
            can_move
        };

        if let Err(e) = ships::put(ctx.db, &ship).await {
            out.push_str(&format!("1 Ship {} save error: {e}\n", ship.uid));
            continue;
        }
        if let Err(e) = sectors::put(ctx.db, &sect).await {
            out.push_str(&format!("1 Sector save error: {e}\n"));
            continue;
        }

        let xy = ctx.format_xy(sx, sy);
        if unload {
            out.push_str(&format!(
                "1 Unloaded {} {} from ship {} to {xy}\n",
                actual, item.name(), ship.uid
            ));
        } else {
            out.push_str(&format!(
                "1 Loaded {} {} onto ship {} from {xy}\n",
                actual, item.name(), ship.uid
            ));
        }
    }

    if out.is_empty() {
        out.push_str(&format!("1 Nothing to {cmd_name}\n"));
    }
    out.push_str(&format!("0 {cmd_name}\n"));
    out
}

/// `load plane <ship-spec> <plane-spec>` / `unload plane <ship-spec> <plane-spec>`.
///
/// Ported from load_plane_ship() in load.c / could_be_on_ship() and
/// put_plane_on_ship() in plnsub.c. Unlike commodities, loading a
/// plane has no harbor requirement -- only that the plane and ship
/// currently share a coordinate (open sea is fine).
///
/// Once aboard, a plane's x/y is kept in sync with the ship's by
/// navigate.rs whenever the ship moves (see 'info navigate').
async fn run_plane_load(parts: &[&str], ctx: &CmdCtx<'_>, unload: bool) -> String {
    let cmd_name = if unload { "unload" } else { "load" };
    if parts.len() < 3 {
        return format!("10 Usage: {cmd_name} plane <ship-spec> <plane-spec>\n");
    }
    let ship_spec = parts[1];
    let plane_spec = parts[2];

    let all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let all_planes = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let ship_chrs = ShipChr::all();
    let plane_chrs = PlaneChr::all();

    let mut matching_ships: Vec<Ship> = all_ships
        .into_iter()
        .filter(|s| s.own == ctx.cnum || ctx.is_deity)
        .filter(|s| ship_spec_matches(ship_spec, s))
        .collect();
    matching_ships.sort_by_key(|s| s.uid);
    if matching_ships.is_empty() {
        return format!("1 No ships match '{ship_spec}'\n0 {cmd_name}\n");
    }

    let mut matching_planes: Vec<Plane> = all_planes
        .into_iter()
        .filter(|p| p.own == ctx.cnum || ctx.is_deity)
        .filter(|p| plane_spec_matches(plane_spec, p))
        .collect();
    matching_planes.sort_by_key(|p| p.uid);
    if matching_planes.is_empty() {
        return format!("1 No planes match '{plane_spec}'\n0 {cmd_name}\n");
    }

    let mut out = String::new();
    let mut placed: std::collections::HashSet<i32> = std::collections::HashSet::new();

    for ship in &matching_ships {
        let Some(ship_chr) = ship_chrs.get(ship.ship_type as usize) else { continue };

        // Running manifest for this ship: seeded from the DB, then
        // updated in-memory as planes are placed within this call so
        // capacity is enforced correctly across multiple loads at once.
        let mut manifest = match planes::get_on_ship(ctx.db, ship.uid).await {
            Ok(v) => v,
            Err(e) => {
                out.push_str(&format!("1 DB error: {e}\n"));
                continue;
            }
        };

        for plane in matching_planes.iter_mut() {
            if placed.contains(&plane.uid) {
                continue;
            }
            if unload {
                if plane.ship != ship.uid {
                    continue;
                }
            } else if plane.x != ship.x || plane.y != ship.y {
                continue;
            }

            let Some(chr) = plane_chrs.get(plane.plane_type as usize) else { continue };

            if unload {
                plane.ship = -1;
                plane.x = ship.x;
                plane.y = ship.y;
                if let Err(e) = planes::put(ctx.db, plane).await {
                    out.push_str(&format!("1 Plane #{}: save error: {e}\n", plane.uid));
                    continue;
                }
                manifest.retain(|p| p.uid != plane.uid);
                out.push_str(&format!(
                    "1 Unloaded plane #{} ({}) from {} #{}\n",
                    plane.uid, chr.name, ship_chr.name, ship.uid
                ));
                placed.insert(plane.uid);
                continue;
            }

            if shipcarry::classify_plane(chr.flags).is_none() {
                out.push_str(&format!(
                    "1 Plane #{}: only light planes, helos, xtra-light, or missiles can load onto ships\n",
                    plane.uid
                ));
                placed.insert(plane.uid); // ineligible by type -- don't retry other ships
                continue;
            }
            if !shipcarry::ship_can_carry(ship_chr, &manifest, plane_chrs, chr.flags) {
                out.push_str(&format!(
                    "1 {} #{} has no room for plane #{}\n",
                    ship_chr.name, ship.uid, plane.uid
                ));
                continue; // full/wrong bucket on this ship -- try the next one
            }

            plane.ship = ship.uid;
            plane.x = ship.x;
            plane.y = ship.y;
            if let Err(e) = planes::put(ctx.db, plane).await {
                out.push_str(&format!("1 Plane #{}: save error: {e}\n", plane.uid));
                continue;
            }
            manifest.push(plane.clone());
            out.push_str(&format!(
                "1 Loaded plane #{} ({}) onto {} #{}\n",
                plane.uid, chr.name, ship_chr.name, ship.uid
            ));
            placed.insert(plane.uid);
        }
    }

    if out.is_empty() {
        out.push_str(&format!("1 Nothing to {cmd_name}\n"));
    }
    out.push_str(&format!("0 {cmd_name}\n"));
    out
}

fn matches_ship(uid: i32, spec: &str) -> bool {
    if spec.is_empty() || spec == "*" { return true; }
    if let Ok(n) = spec.parse::<i32>() { return uid == n; }
    if let Some((lo, hi)) = spec.split_once('-') {
        if let (Ok(lo), Ok(hi)) = (lo.trim().parse::<i32>(), hi.trim().parse::<i32>()) {
            return uid >= lo && uid <= hi;
        }
    }
    true
}

fn parse_item_name(s: &str) -> Option<Item> {
    let s_lc = s.to_lowercase();
    let all_items = [
        Item::Civil, Item::Milit, Item::Shell, Item::Gun, Item::Petrol,
        Item::Iron, Item::Dust, Item::Bar, Item::Food, Item::Oil,
        Item::Lcm, Item::Hcm, Item::Uw, Item::Rad,
    ];
    match s_lc.as_str() {
        "dust" | "gold dust" => return Some(Item::Dust),
        "bar" | "bars" | "gold bars" => return Some(Item::Bar),
        "lcm" | "light" => return Some(Item::Lcm),
        "hcm" | "heavy" => return Some(Item::Hcm),
        "uw" | "undesirable" | "undesirables" => return Some(Item::Uw),
        _ => {}
    }
    for &item in &all_items {
        if item.name().starts_with(s_lc.as_str()) {
            return Some(item);
        }
    }
    None
}
