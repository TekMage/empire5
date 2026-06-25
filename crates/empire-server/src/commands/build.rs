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
// Ported from: src/lib/commands/buil.c
// Known contributors to the original:
//    Steve McClure, 1998-2000
//    Markus Armbruster, 2004-2016

// "build" command — build ships, planes, or land units from a sector.
//
// Usage: build s|l|p SECT-SPEC TYPE-NUMBER [COUNT]
//   s = ship, l = land unit, p = plane
//
// For ships the sector must be a Harbor (*) or Naval (n) base.
// For planes the sector must be an Airfield (a).
// For land units any owned sector works.
//
// The command deducts LCM, HCM, and avail from the sector and inserts a new
// unit record at 10%/10% efficiency with zero mobility.

use empire_db::{sectors, ships, planes, land_units};
use empire_types::commodity::{Inventory, Item};
use empire_types::land::LandUnit;
use empire_types::plane::Plane;
use empire_types::sector::SectorType;
use empire_types::ship::{RetreatFlags, Ship};
use empire_types::ship_chr::ShipChr;
use empire_types::land_chr::LandChr;
use empire_types::plane_chr::PlaneChr;
use empire_types::plane::PlaneFlags;
use super::ctx::CmdCtx;
use super::sector_sel::matches_area;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 3 {
        return "10 Usage: build s|l|p SECT-SPEC TYPE-NUMBER [COUNT]\n".to_string();
    }
    let what = parts[0];
    let sect_spec = parts[1];
    let type_str = parts[2];
    let count: u32 = if parts.len() >= 4 {
        match parts[3].parse::<u32>() {
            Ok(n) => {
                if n > 1 && !ctx.is_deity {
                    return "10 Only deity can build more than one at a time\n".to_string();
                }
                n
            }
            Err(_) => return "10 Invalid count\n".to_string(),
        }
    } else {
        1
    };

    let type_idx: usize = match type_str.parse::<usize>() {
        Ok(n) => n,
        Err(_) => return format!("10 Invalid type number '{}'\n", type_str),
    };

    match what {
        "s" | "ship" => build_ships(ctx, sect_spec, type_idx, count).await,
        "l" | "land" => build_land(ctx, sect_spec, type_idx, count).await,
        "p" | "plane" => build_planes(ctx, sect_spec, type_idx, count).await,
        _ => "10 Usage: build s|l|p SECT-SPEC TYPE-NUMBER [COUNT]\n".to_string(),
    }
}

// ── Ship building ─────────────────────────────────────────────────────────────

async fn build_ships(ctx: &CmdCtx<'_>, sect_spec: &str, type_idx: usize, count: u32) -> String {
    let mchr = match ShipChr::for_type(type_idx) {
        Some(c) => c,
        None => return format!("10 Unknown ship type {type_idx}\n"),
    };

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let mut built = 0u32;

    'outer: for _ in 0..count {
        for mut sect in all_sectors.iter().cloned() {
            if sect.own != ctx.cnum && !ctx.is_deity {
                continue;
            }
            if sect.own == 0 {
                continue;
            }
            if !matches_area(&sect, sect_spec, ctx) {
                continue;
            }

            // Ships must be built in harbor or naval base (unless deity)
            if !ctx.is_deity
                && sect.sector_type != SectorType::Harbor
                && sect.sector_type != SectorType::Naval
            {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!("1 {xy}: ships must be built in harbor or naval base\n"));
                continue;
            }

            // Check efficiency >= 60 (unless deity)
            if !ctx.is_deity && sect.effic < 60 {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!("1 {xy}: sector must be >= 60% efficient\n"));
                continue;
            }

            // Avail cost: (bwork * SHIP_MINEFF + 99) / 100
            let avail_cost = (mchr.bwork * 20 + 99) / 100;
            if !ctx.is_deity && sect.avail < avail_cost as i16 {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!(
                    "1 {xy}: not enough workforce (need {} avail)\n",
                    avail_cost
                ));
                continue;
            }

            // Material cost at 20% efficiency
            let lcm_need = (mchr.lcm * 20 + 99) / 100;
            let hcm_need = (mchr.hcm * 20 + 99) / 100;
            let sect_lcm = sect.items.get(Item::Lcm);
            let sect_hcm = sect.items.get(Item::Hcm);

            if !ctx.is_deity && (sect_lcm < lcm_need as i16 || sect_hcm < hcm_need as i16) {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!(
                    "1 {xy}: not enough materials (need {} lcm, {} hcm; have {} lcm, {} hcm)\n",
                    lcm_need, hcm_need, sect_lcm, sect_hcm,
                ));
                continue;
            }

            // Deduct materials and avail
            if !ctx.is_deity {
                sect.items.set(Item::Lcm, sect_lcm - lcm_need as i16);
                sect.items.set(Item::Hcm, sect_hcm - hcm_need as i16);
                sect.avail -= avail_cost as i16;
            }

            // Allocate a new UID: max existing uid + 1 (INSERT OR REPLACE with 0 will
            // become max+1 when SQLite auto-assigns if uid=0 doesn't exist yet)
            let new_uid = next_ship_uid(ctx).await;

            let ship = Ship {
                uid: new_uid,
                own: ctx.cnum,
                x: sect.x,
                y: sect.y,
                ship_type: type_idx as i8,
                effic: 20, // SHIP_MINEFF
                mobil: 0,
                off: false,
                tech: 0,
                fleet: ' ',
                opx: sect.x,
                opy: sect.y,
                mission: 0,
                mission_radius: 0,
                items: Inventory::zero(),
                pstage: 0,
                ptime: 0,
                access: 0,
                name: String::new(),
                orig_x: sect.x,
                orig_y: sect.y,
                orig_own: ctx.cnum,
                retreat_flags: RetreatFlags::empty(),
                retreat_path: String::new(),
            };

            let xy = ctx.format_xy(sect.x, sect.y);
            if let Err(e) = sectors::put(ctx.db, &sect).await {
                out.push_str(&format!("1 {xy}: sector save error: {e}\n"));
                continue;
            }
            if let Err(e) = ships::put(ctx.db, &ship).await {
                out.push_str(&format!("1 {xy}: ship save error: {e}\n"));
                continue;
            }

            out.push_str(&format!(
                "1 Building a {} in {xy}\n",
                mchr.name,
            ));
            if !ctx.is_deity {
                out.push_str(&format!(
                    "1 Deducted {} lcm, {} hcm.\n",
                    lcm_need, hcm_need,
                ));
            }
            built += 1;
            if built >= count {
                break 'outer;
            }
        }
        if built == 0 && count == 1 {
            break;
        }
    }

    if built == 0 {
        out.push_str("1 No sectors suitable for building.\n");
    }
    out.push_str("0 build\n");
    out
}

// ── Land unit building ────────────────────────────────────────────────────────

async fn build_land(ctx: &CmdCtx<'_>, sect_spec: &str, type_idx: usize, count: u32) -> String {
    let lchr = match LandChr::for_type(type_idx) {
        Some(c) => c,
        None => return format!("10 Unknown land unit type {type_idx}\n"),
    };

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let mut built = 0u32;

    'outer: for _ in 0..count {
        for mut sect in all_sectors.iter().cloned() {
            if sect.own != ctx.cnum && !ctx.is_deity {
                continue;
            }
            if sect.own == 0 {
                continue;
            }
            if !matches_area(&sect, sect_spec, ctx) {
                continue;
            }
            // Land units require Urban (capital) in C, but we allow any owned sector
            // to keep things flexible for Phase 8.

            if !ctx.is_deity && sect.effic < 60 {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!("1 {xy}: sector must be >= 60% efficient\n"));
                continue;
            }

            let avail_cost = (lchr.bwork * 10 + 99) / 100;
            if !ctx.is_deity && sect.avail < avail_cost as i16 {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!(
                    "1 {xy}: not enough workforce (need {} avail)\n",
                    avail_cost
                ));
                continue;
            }

            let lcm_need = (lchr.lcm * 10 + 99) / 100;
            let hcm_need = (lchr.hcm * 10 + 99) / 100;
            let sect_lcm = sect.items.get(Item::Lcm);
            let sect_hcm = sect.items.get(Item::Hcm);

            if !ctx.is_deity && (sect_lcm < lcm_need as i16 || sect_hcm < hcm_need as i16) {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!(
                    "1 {xy}: not enough materials (need {} lcm, {} hcm)\n",
                    lcm_need, hcm_need,
                ));
                continue;
            }

            if !ctx.is_deity {
                sect.items.set(Item::Lcm, sect_lcm - lcm_need as i16);
                sect.items.set(Item::Hcm, sect_hcm - hcm_need as i16);
                sect.avail -= avail_cost as i16;
            }

            let new_uid = next_land_uid(ctx).await;

            use empire_types::ship::RetreatFlags as RF;
            let unit = LandUnit {
                uid: new_uid,
                own: ctx.cnum,
                x: sect.x,
                y: sect.y,
                land_type: type_idx as i8,
                effic: 10, // LAND_MINEFF
                mobil: 0,
                off: false,
                tech: 0,
                army: ' ',
                opx: sect.x,
                opy: sect.y,
                mission: 0,
                mission_radius: 0,
                ship: -1,
                harden: 0,
                retreat: 50,
                retreat_flags: RF::empty(),
                retreat_path: String::new(),
                scar: 0,
                items: Inventory::zero(),
                pstage: 0,
                ptime: 0,
                carried_by_land: -1,
                access: 0,
            };

            let xy = ctx.format_xy(sect.x, sect.y);
            if let Err(e) = sectors::put(ctx.db, &sect).await {
                out.push_str(&format!("1 {xy}: sector save error: {e}\n"));
                continue;
            }
            if let Err(e) = land_units::put(ctx.db, &unit).await {
                out.push_str(&format!("1 {xy}: unit save error: {e}\n"));
                continue;
            }

            out.push_str(&format!("1 Building a {} in {xy}\n", lchr.name));
            if !ctx.is_deity {
                out.push_str(&format!(
                    "1 Deducted {} lcm, {} hcm.\n",
                    lcm_need, hcm_need,
                ));
            }
            built += 1;
            if built >= count {
                break 'outer;
            }
        }
        if built == 0 && count == 1 {
            break;
        }
    }

    if built == 0 {
        out.push_str("1 No sectors suitable for building.\n");
    }
    out.push_str("0 build\n");
    out
}

// ── Plane building ────────────────────────────────────────────────────────────

async fn build_planes(ctx: &CmdCtx<'_>, sect_spec: &str, type_idx: usize, count: u32) -> String {
    let pchr = match PlaneChr::for_type(type_idx) {
        Some(c) => c,
        None => return format!("10 Unknown plane type {type_idx}\n"),
    };

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let mut built = 0u32;

    'outer: for _ in 0..count {
        for mut sect in all_sectors.iter().cloned() {
            if sect.own != ctx.cnum && !ctx.is_deity {
                continue;
            }
            if sect.own == 0 {
                continue;
            }
            if !matches_area(&sect, sect_spec, ctx) {
                continue;
            }

            // Planes must be built in airfields (unless deity)
            if !ctx.is_deity && sect.sector_type != SectorType::Airfield {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!("1 {xy}: planes must be built in airfields\n"));
                continue;
            }

            if !ctx.is_deity && sect.effic < 60 {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!("1 {xy}: sector must be >= 60% efficient\n"));
                continue;
            }

            let avail_cost = (pchr.bwork * 10 + 99) / 100;
            if !ctx.is_deity && sect.avail < avail_cost as i16 {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!(
                    "1 {xy}: not enough workforce (need {} avail)\n",
                    avail_cost
                ));
                continue;
            }

            let lcm_need = (pchr.lcm * 10 + 99) / 100;
            let hcm_need = (pchr.hcm * 10 + 99) / 100;
            let sect_lcm = sect.items.get(Item::Lcm);
            let sect_hcm = sect.items.get(Item::Hcm);

            if !ctx.is_deity && (sect_lcm < lcm_need as i16 || sect_hcm < hcm_need as i16) {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!(
                    "1 {xy}: not enough materials (need {} lcm, {} hcm)\n",
                    lcm_need, hcm_need,
                ));
                continue;
            }

            if !ctx.is_deity {
                sect.items.set(Item::Lcm, sect_lcm - lcm_need as i16);
                sect.items.set(Item::Hcm, sect_hcm - hcm_need as i16);
                sect.avail -= avail_cost as i16;
            }

            let new_uid = next_plane_uid(ctx).await;

            let plane = Plane {
                uid: new_uid,
                own: ctx.cnum,
                x: sect.x,
                y: sect.y,
                plane_type: type_idx as i8,
                effic: 10, // PLANE_MINEFF
                mobil: 0,
                off: false,
                tech: 0,
                wing: ' ',
                opx: sect.x,
                opy: sect.y,
                mission: 0,
                mission_radius: 0,
                range: u8::MAX,
                harden: 0,
                ship: -1,
                land: -1,
                flags: PlaneFlags::empty(),
                access: 0,
                theta: 0.0,
            };

            let xy = ctx.format_xy(sect.x, sect.y);
            if let Err(e) = sectors::put(ctx.db, &sect).await {
                out.push_str(&format!("1 {xy}: sector save error: {e}\n"));
                continue;
            }
            if let Err(e) = planes::put(ctx.db, &plane).await {
                out.push_str(&format!("1 {xy}: plane save error: {e}\n"));
                continue;
            }

            out.push_str(&format!("1 Building a {} in {xy}\n", pchr.name));
            if !ctx.is_deity {
                out.push_str(&format!(
                    "1 Deducted {} lcm, {} hcm.\n",
                    lcm_need, hcm_need,
                ));
            }
            built += 1;
            if built >= count {
                break 'outer;
            }
        }
        if built == 0 && count == 1 {
            break;
        }
    }

    if built == 0 {
        out.push_str("1 No sectors suitable for building.\n");
    }
    out.push_str("0 build\n");
    out
}

// ── UID allocation ────────────────────────────────────────────────────────────

/// Allocate the next available ship UID (max existing + 1).
async fn next_ship_uid(ctx: &CmdCtx<'_>) -> i32 {
    match ships::get_all(ctx.db).await {
        Ok(all) => all.iter().map(|s| s.uid).max().unwrap_or(-1) + 1,
        Err(_) => 0,
    }
}

/// Allocate the next available land unit UID (max existing + 1).
async fn next_land_uid(ctx: &CmdCtx<'_>) -> i32 {
    match land_units::get_all(ctx.db).await {
        Ok(all) => all.iter().map(|u| u.uid).max().unwrap_or(-1) + 1,
        Err(_) => 0,
    }
}

/// Allocate the next available plane UID (max existing + 1).
async fn next_plane_uid(ctx: &CmdCtx<'_>) -> i32 {
    match planes::get_all(ctx.db).await {
        Ok(all) => all.iter().map(|p| p.uid).max().unwrap_or(-1) + 1,
        Err(_) => 0,
    }
}

