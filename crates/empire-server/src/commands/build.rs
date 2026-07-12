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
use empire_types::coords::Coord;
use empire_types::land::LandUnit;
use empire_types::plane::Plane;
use empire_types::sector::{Sector, SectorType};
use empire_types::ship::{RetreatFlags, Ship};
use empire_types::ship_chr::ShipChr;
use empire_types::land_chr::LandChr;
use empire_types::plane_chr::PlaneChr;
use empire_types::plane::PlaneFlags;
use super::ctx::CmdCtx;
use super::sector_sel::SectSpec;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        return "10 Usage: build s|l|p|b|t SECT-SPEC ...\n".to_string();
    }
    let what = parts[0];

    // Bridge span/tower have their own arg layout
    match what {
        "b" | "bridge" => {
            let sect_spec = parts.get(1).copied().unwrap_or("");
            let direction = parts.get(2).copied().unwrap_or("");
            return build_bridge_span(ctx, sect_spec, direction).await;
        }
        "t" | "tower" => {
            let sect_spec = parts.get(1).copied().unwrap_or("");
            let direction = parts.get(2).copied().unwrap_or("");
            return build_bridge_tower(ctx, sect_spec, direction).await;
        }
        _ => {}
    }

    if parts.len() < 3 {
        return "10 Usage: build s|l|p SECT-SPEC TYPE-NUMBER [COUNT]\n".to_string();
    }
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
        "s" | "ship"  => build_ships(ctx, sect_spec, type_idx, count).await,
        "l" | "land"  => build_land(ctx, sect_spec, type_idx, count).await,
        "p" | "plane" => build_planes(ctx, sect_spec, type_idx, count).await,
        _ => "10 Usage: build s|l|p|b|t SECT-SPEC ...\n".to_string(),
    }
}

// ── Ship building ─────────────────────────────────────────────────────────────

async fn build_ships(ctx: &CmdCtx<'_>, sect_spec: &str, type_idx: usize, count: u32) -> String {
    let mchr = match ShipChr::for_type(type_idx) {
        Some(c) => c,
        None => return format!("10 Unknown ship type {type_idx}\n"),
    };

    let filter = match SectSpec::parse(sect_spec, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
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
            if !filter.matches(&sect, ctx.world_x, ctx.world_y) {
                continue;
            }

            // Ships must be built in a harbor (unless deity)
            if !ctx.is_deity
                && sect.sector_type != SectorType::Harbor
            {
                let xy = ctx.format_xy(sect.x, sect.y);
                out.push_str(&format!("1 {xy}: ships must be built in a harbor\n"));
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
                tech: ctx.nat.tech as i16,
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

    let filter = match SectSpec::parse(sect_spec, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
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
            if !filter.matches(&sect, ctx.world_x, ctx.world_y) {
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
                tech: ctx.nat.tech as i16,
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

    let filter = match SectSpec::parse(sect_spec, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
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
            if !filter.matches(&sect, ctx.world_x, ctx.world_y) {
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
                tech: ctx.nat.tech as i16,
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

// ── Bridge span building ──────────────────────────────────────────────────────
//
// Mirrors build_bspan() in buil.c.
// Requirements:
//   tech >= 10.0  (buil_bt)
//   source sector must be BridgeHead (or BridgeTower)
//   source must have >= 100 HCM  (buil_bh)
//   source must have enough avail
//   cost $1000  (buil_bc)
//   target must be adjacent water sector
//   target must have a supporting bridge head or tower adjacent to it

const BSPAN_TECH_REQ: f64  = 10.0;
const BSPAN_HCM_REQ:  i16  = 100;
const BSPAN_CASH_REQ: f64  = 1000.0;
const BTOWER_TECH_REQ: f64 = 100.0;
const BTOWER_HCM_REQ:  i16 = 300;
const BTOWER_CASH_REQ: f64 = 3000.0;

// 6-direction offsets matching Empire hex grid (matching DIROFF in update.rs)
const DIROFF6: [(Coord, Coord); 6] = [
    (1, -1),  // NE
    (2, 0),   // E
    (1, 1),   // SE
    (-1, 1),  // SW
    (-2, 0),  // W
    (-1, -1), // NW
];

fn parse_direction(s: &str) -> Option<(Coord, Coord)> {
    match s.trim().to_lowercase().as_str() {
        "ne" | "ur" => Some((1, -1)),
        "e"  | "r"  => Some((2, 0)),
        "se" | "dr" => Some((1, 1)),
        "sw" | "dl" => Some((-1, 1)),
        "w"  | "l"  => Some((-2, 0)),
        "nw" | "ul" => Some((-1, -1)),
        _ => None,
    }
}

fn wrap(v: i16, max: i32) -> i16 {
    ((v as i32).rem_euclid(max)) as i16
}

/// True if (x,y) is adjacent to a bridge head or tower owned by any nation.
fn has_bridge_support(x: Coord, y: Coord, all: &[Sector], wx: i32, wy: i32) -> bool {
    for (dx, dy) in DIROFF6 {
        let nx = wrap(x + dx, wx);
        let ny = wrap(y + dy, wy);
        if let Some(s) = all.iter().find(|s| s.x == nx && s.y == ny) {
            if s.effic >= 60
                && (s.sector_type == SectorType::BridgeHead
                    || s.sector_type == SectorType::BridgeTower)
            {
                return true;
            }
        }
    }
    false
}

async fn build_bridge_span(ctx: &CmdCtx<'_>, sect_spec: &str, direction: &str) -> String {
    if sect_spec.is_empty() {
        return "10 Usage: build b <bridge-head-sector> <direction>\n  Directions: NE E SE SW W NW\n".to_string();
    }
    if !ctx.is_deity && ctx.nat.tech < BSPAN_TECH_REQ {
        return format!("10 Building a bridge span requires tech {:.0}\n", BSPAN_TECH_REQ);
    }
    let (dx, dy) = match parse_direction(direction) {
        Some(d) => d,
        None => return format!(
            "10 '{}' is not a valid direction (use NE E SE SW W NW)\n", direction
        ),
    };

    let filter = match SectSpec::parse(sect_spec, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
    };
    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let mut found = false;
    let wx = ctx.world_x;
    let wy = ctx.world_y;

    for mut sp in all_sectors.clone() {
        if sp.own != ctx.cnum && !ctx.is_deity { continue; }
        if sp.own == 0 { continue; }
        if !filter.matches(&sp, wx, wy) { continue; }
        if sp.sector_type != SectorType::BridgeHead
            && sp.sector_type != SectorType::BridgeTower
            && !ctx.is_deity
        {
            out.push_str(&format!(
                "1 {}: must be a bridge head (#) or tower\n",
                ctx.format_xy(sp.x, sp.y)
            ));
            found = true;
            continue;
        }
        found = true;

        // Check HCM
        if sp.items.get(Item::Hcm) < BSPAN_HCM_REQ {
            out.push_str(&format!(
                "1 {}: not enough HCM (need {})\n",
                ctx.format_xy(sp.x, sp.y), BSPAN_HCM_REQ
            ));
            continue;
        }
        // Check cash
        if ctx.nat.money < BSPAN_CASH_REQ as i32 {
            out.push_str(&format!("1 Not enough money (need ${:.0})\n", BSPAN_CASH_REQ));
            break;
        }

        let tx = wrap(sp.x + dx, wx);
        let ty = wrap(sp.y + dy, wy);
        let target = all_sectors.iter().find(|s| s.x == tx && s.y == ty);

        match target {
            None => {
                out.push_str(&format!(
                    "1 {}: no sector in that direction\n",
                    ctx.format_xy(tx, ty)
                ));
                continue;
            }
            Some(t) if t.sector_type != SectorType::Sea => {
                out.push_str(&format!(
                    "1 {}: not a water sector\n",
                    ctx.format_xy(tx, ty)
                ));
                continue;
            }
            _ => {}
        }
        if !has_bridge_support(tx, ty, &all_sectors, wx, wy) && !ctx.is_deity {
            out.push_str(&format!(
                "1 {}: not adjacent to a bridge head or tower\n",
                ctx.format_xy(tx, ty)
            ));
            continue;
        }

        // Build the span — convert target water sector to BridgeSpan
        let mut new_span = all_sectors.iter().find(|s| s.x == tx && s.y == ty).unwrap().clone();
        new_span.sector_type = SectorType::BridgeSpan;
        new_span.new_type    = SectorType::BridgeSpan;
        new_span.effic       = 10;  // SCT_MINEFF
        new_span.mobil       = 0;
        new_span.own         = sp.own;
        new_span.old_own     = sp.own;

        // Deduct HCM and cash from source sector
        sp.items.add(Item::Hcm, -BSPAN_HCM_REQ);

        if let Err(e) = sectors::put(ctx.db, &new_span).await {
            out.push_str(&format!("1 database error: {e}\n"));
            continue;
        }
        if let Err(e) = sectors::put(ctx.db, &sp).await {
            out.push_str(&format!("1 database error: {e}\n"));
            continue;
        }
        // Deduct cash from nation
        let mut nat = ctx.nat.clone();
        nat.money = (nat.money as f64 - BSPAN_CASH_REQ) as i32;
        let _ = empire_db::nations::put(ctx.db, &nat).await;

        out.push_str(&format!(
            "1 Bridge span built over {}\n",
            ctx.format_xy(tx, ty)
        ));
    }

    if !found {
        out.push_str(&format!("1 {sect_spec}: No sector(s)\n"));
    }
    out.push_str("0 build\n");
    out
}

async fn build_bridge_tower(ctx: &CmdCtx<'_>, sect_spec: &str, direction: &str) -> String {
    if sect_spec.is_empty() {
        return "10 Usage: build t <bridge-span-sector> <direction>\n  Directions: NE E SE SW W NW\n".to_string();
    }
    if !ctx.is_deity && ctx.nat.tech < BTOWER_TECH_REQ {
        return format!("10 Building a bridge tower requires tech {:.0}\n", BTOWER_TECH_REQ);
    }
    let (dx, dy) = match parse_direction(direction) {
        Some(d) => d,
        None => return format!(
            "10 '{}' is not a valid direction (use NE E SE SW W NW)\n", direction
        ),
    };

    let filter = match SectSpec::parse(sect_spec, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
    };
    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let mut found = false;
    let wx = ctx.world_x;
    let wy = ctx.world_y;

    for mut sp in all_sectors.clone() {
        if sp.own != ctx.cnum && !ctx.is_deity { continue; }
        if sp.own == 0 { continue; }
        if !filter.matches(&sp, wx, wy) { continue; }
        if sp.sector_type != SectorType::BridgeSpan && !ctx.is_deity {
            out.push_str(&format!(
                "1 {}: towers can only be built from bridge spans\n",
                ctx.format_xy(sp.x, sp.y)
            ));
            found = true;
            continue;
        }
        found = true;

        if sp.items.get(Item::Hcm) < BTOWER_HCM_REQ {
            out.push_str(&format!(
                "1 {}: not enough HCM (need {})\n",
                ctx.format_xy(sp.x, sp.y), BTOWER_HCM_REQ
            ));
            continue;
        }
        if ctx.nat.money < BTOWER_CASH_REQ as i32 {
            out.push_str(&format!("1 Not enough money (need ${:.0})\n", BTOWER_CASH_REQ));
            break;
        }

        let tx = wrap(sp.x + dx, wx);
        let ty = wrap(sp.y + dy, wy);
        let target = all_sectors.iter().find(|s| s.x == tx && s.y == ty);

        match target {
            None => {
                out.push_str(&format!("1 {}: no sector\n", ctx.format_xy(tx, ty)));
                continue;
            }
            Some(t) if t.sector_type != SectorType::Sea => {
                out.push_str(&format!("1 {}: not a water sector\n", ctx.format_xy(tx, ty)));
                continue;
            }
            _ => {}
        }

        // Tower cannot be adjacent to land (except water/bridge types)
        let land_adjacent = DIROFF6.iter().any(|&(odx, ody)| {
            let nx = wrap(tx + odx, wx);
            let ny = wrap(ty + ody, wy);
            if let Some(adj) = all_sectors.iter().find(|s| s.x == nx && s.y == ny) {
                let t = adj.sector_type;
                t != SectorType::Sea && t != SectorType::BridgeSpan && t != SectorType::BridgeTower
            } else { false }
        });
        if land_adjacent && !ctx.is_deity {
            out.push_str(&format!(
                "1 {}: can't build tower next to land\n",
                ctx.format_xy(tx, ty)
            ));
            continue;
        }

        let mut new_tower = all_sectors.iter().find(|s| s.x == tx && s.y == ty).unwrap().clone();
        new_tower.sector_type = SectorType::BridgeTower;
        new_tower.new_type    = SectorType::BridgeTower;
        new_tower.effic       = 10;
        new_tower.mobil       = 0;
        new_tower.own         = sp.own;
        new_tower.old_own     = sp.own;

        sp.items.add(Item::Hcm, -BTOWER_HCM_REQ);

        if let Err(e) = sectors::put(ctx.db, &new_tower).await {
            out.push_str(&format!("1 database error: {e}\n"));
            continue;
        }
        if let Err(e) = sectors::put(ctx.db, &sp).await {
            out.push_str(&format!("1 database error: {e}\n"));
            continue;
        }
        let mut nat = ctx.nat.clone();
        nat.money = (nat.money as f64 - BTOWER_CASH_REQ) as i32;
        let _ = empire_db::nations::put(ctx.db, &nat).await;

        out.push_str(&format!(
            "1 Bridge tower built at {}\n",
            ctx.format_xy(tx, ty)
        ));
    }

    if !found {
        out.push_str(&format!("1 {sect_spec}: No sector(s)\n"));
    }
    out.push_str("0 build\n");
    out
}

