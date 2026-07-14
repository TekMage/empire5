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
// Ported from: src/lib/commands/march.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995

// "march" command — march land units along a route.
//
// Usage: march UNIT-SPEC ROUTE
//
// UNIT-SPEC: a land unit uid ("3"), a uid range ("0-5"), a comma list,
//   "*" for all owned units, "~" for units with no army assigned, or a
//   single letter naming an army (see 'info army').
// ROUTE: either a direction string (e.g. "uujnb") using chars from
//        geo::DIRCH, or a destination "X,Y" (player-relative) which
//        triggers pathfinding.
//
// Mobility cost per step: 1 mob unit (simplified from C's terrain-based formula).
// A unit with ship != -1 is considered loaded and cannot march.

use empire_db::{sectors, land_units};
use empire_types::coords::Coord;
use empire_types::sector::SectorType;
use crate::subs::geo::{DIROFF, DIRCH, DIR_FIRST, DIR_LAST, x_norm, y_norm, dir_from_char};
use crate::subs::pathfind::find_path;
use crate::subs::lndsub::land_spec_matches;
use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return "10 Usage: march UNIT-SPEC ROUTE\n".to_string();
    }
    let unit_spec = parts[0].trim();
    let route_str = parts[1].trim();

    // Load matching land units
    let all_units = match land_units::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let mut processed = 0u32;

    for mut unit in all_units {
        // Ownership check
        if unit.own != ctx.cnum && !ctx.is_deity {
            continue;
        }
        if unit.own == 0 {
            continue;
        }
        // UID/army filter
        if !land_spec_matches(unit_spec, &unit) {
            continue;
        }
        // Cannot march if loaded on a ship
        if unit.ship >= 0 {
            out.push_str(&format!(
                "1 Unit {} is loaded on ship {} — cannot march\n",
                unit.uid, unit.ship,
            ));
            continue;
        }

        processed += 1;

        // Build direction list from route
        let directions = match build_route(route_str, unit.x, unit.y, ctx).await {
            Ok(d) => d,
            Err(msg) => {
                out.push_str(&format!("1 Unit {}: {}\n", unit.uid, msg));
                continue;
            }
        };

        if directions.is_empty() {
            out.push_str(&format!(
                "1 Unit {} at {} — already at destination\n",
                unit.uid,
                ctx.format_xy(unit.x, unit.y),
            ));
            continue;
        }

        // March each step
        let mut path_taken: Vec<char> = Vec::new();
        for dir_idx in directions {
            if unit.mobil <= 0 {
                out.push_str(&format!(
                    "1 Unit {} ran out of mobility at {}\n",
                    unit.uid,
                    ctx.format_xy(unit.x, unit.y),
                ));
                break;
            }

            let (ddx, ddy) = DIROFF[dir_idx as usize];
            let nx = x_norm(unit.x + ddx, ctx.world_x);
            let ny = y_norm(unit.y + ddy, ctx.world_y);

            // Load destination sector
            let dest_sect = match sectors::get_at(ctx.db, nx, ny).await {
                Ok(Some(s)) => s,
                Ok(None) => {
                    out.push_str(&format!(
                        "1 Unit {}: sector {} does not exist\n",
                        unit.uid,
                        ctx.format_xy(nx, ny),
                    ));
                    break;
                }
                Err(e) => {
                    out.push_str(&format!("1 Unit {}: db error: {e}\n", unit.uid));
                    break;
                }
            };

            // Passability: not sea, and either unowned or owned by us or allied
            if dest_sect.sector_type == SectorType::Sea {
                out.push_str(&format!(
                    "1 Unit {}: {} is a sea sector — cannot march\n",
                    unit.uid,
                    ctx.format_xy(nx, ny),
                ));
                break;
            }

            // Stop at enemy-owned sectors
            if dest_sect.own != 0
                && dest_sect.own != ctx.cnum
                && !ctx.is_deity
            {
                // In the full game this would trigger combat; for now we stop.
                out.push_str(&format!(
                    "1 Unit {}: {} is enemy territory — march stopped\n",
                    unit.uid,
                    ctx.format_xy(nx, ny),
                ));
                break;
            }

            // Deduct 1 mobility
            unit.mobil = unit.mobil.saturating_sub(1);
            unit.x = nx;
            unit.y = ny;
            path_taken.push(DIRCH[dir_idx as usize]);
        }

        if !path_taken.is_empty() {
            let from_xy = ctx.format_xy(unit.x, unit.y); // already updated
            let path_str: String = path_taken.iter().collect();
            out.push_str(&format!(
                "1 Unit {} marched {}: now at {}\n",
                unit.uid, path_str, from_xy,
            ));

            if let Err(e) = land_units::put(ctx.db, &unit).await {
                out.push_str(&format!("1 Unit {}: save error: {e}\n", unit.uid));
            }
        }
    }

    if processed == 0 {
        out.push_str("1 No matching land units.\n");
    }
    out.push_str("0 march\n");
    out
}

/// Parse the route string into a sequence of direction indices (1–6).
/// If the route looks like "X,Y" coordinates, use pathfinding.
async fn build_route(
    route_str: &str,
    from_x: Coord,
    from_y: Coord,
    ctx: &CmdCtx<'_>,
) -> Result<Vec<u8>, String> {
    // Check if the route is a destination coordinate
    if let Some((rx, ry)) = parse_rel_xy(route_str) {
        let dx = ctx.x_abs(rx);
        let dy = ctx.y_abs(ry);

        // Build passable closure: land sectors, not sea
        // We need to snapshot sectors synchronously; clone is needed since
        // find_path doesn't know about async.
        let all_sects = sectors::get_all(ctx.db)
            .await
            .map_err(|e| format!("db error: {e}"))?;

        // Build a lookup map (x, y) -> SectorType
        use std::collections::HashMap;
        let sect_map: HashMap<(Coord, Coord), SectorType> = all_sects
            .iter()
            .map(|s| ((s.x, s.y), s.sector_type))
            .collect();

        let dirs = find_path(from_x, from_y, dx, dy, ctx.world_x, ctx.world_y, |nx, ny| {
            match sect_map.get(&(nx, ny)) {
                Some(&SectorType::Sea) | None => false,
                Some(_) => true,
            }
        });

        if dirs.is_empty() && (from_x != dx || from_y != dy) {
            return Err(format!(
                "no passable path to {}",
                ctx.format_xy(dx, dy),
            ));
        }
        return Ok(dirs);
    }

    // Otherwise treat as direction chars
    let mut dirs = Vec::new();
    for ch in route_str.chars() {
        match dir_from_char(ch) {
            Some(d) if d >= DIR_FIRST && d <= DIR_LAST => dirs.push(d as u8),
            Some(0) => break, // DIR_STOP ('h')
            _ => return Err(format!("unknown direction character '{ch}'")),
        }
    }
    Ok(dirs)
}
