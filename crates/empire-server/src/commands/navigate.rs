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
// Ported from: src/lib/commands/nav.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995

// "navigate" command — navigate ships along a route.
//
// Usage: navigate SHIP-SPEC ROUTE
//
// SHIP-SPEC: a ship uid (e.g. "3"), or "*" for all owned ships.
// ROUTE: a direction string (e.g. "uujnb") or a destination "X,Y"
//        (player-relative) which triggers pathfinding.
//
// Ships can move through sea, harbor, and naval base sectors.
// Mobility cost per step: 1 mob unit (simplified).

use empire_db::{sectors, ships};
use empire_types::coords::Coord;
use empire_types::sector::SectorType;
use crate::subs::geo::{DIROFF, DIRCH, DIR_FIRST, DIR_LAST, x_norm, y_norm, dir_from_char};
use crate::subs::pathfind::find_path;
use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return "10 Usage: navigate SHIP-SPEC ROUTE\n".to_string();
    }
    let ship_spec = parts[0].trim();
    let route_str = parts[1].trim();

    let all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let mut processed = 0u32;

    for mut ship in all_ships {
        if ship.own != ctx.cnum && !ctx.is_deity {
            continue;
        }
        if ship.own == 0 {
            continue;
        }
        if !ship_matches(ship_spec, ship.uid) {
            continue;
        }

        processed += 1;

        let directions = match build_route(route_str, ship.x, ship.y, ctx).await {
            Ok(d) => d,
            Err(msg) => {
                out.push_str(&format!("1 Ship {}: {}\n", ship.uid, msg));
                continue;
            }
        };

        if directions.is_empty() {
            out.push_str(&format!(
                "1 Ship {} already at {}\n",
                ship.uid,
                ctx.format_xy(ship.x, ship.y),
            ));
            continue;
        }

        let mut path_taken: Vec<char> = Vec::new();
        for dir_idx in directions {
            if ship.mobil <= 0 {
                out.push_str(&format!(
                    "1 Ship {} ran out of mobility at {}\n",
                    ship.uid,
                    ctx.format_xy(ship.x, ship.y),
                ));
                break;
            }

            let (ddx, ddy) = DIROFF[dir_idx as usize];
            let nx = x_norm(ship.x + ddx, ctx.world_x);
            let ny = y_norm(ship.y + ddy, ctx.world_y);

            let dest_sect = match sectors::get_at(ctx.db, nx, ny).await {
                Ok(Some(s)) => s,
                Ok(None) => {
                    out.push_str(&format!(
                        "1 Ship {}: sector {} does not exist\n",
                        ship.uid,
                        ctx.format_xy(nx, ny),
                    ));
                    break;
                }
                Err(e) => {
                    out.push_str(&format!("1 Ship {}: db error: {e}\n", ship.uid));
                    break;
                }
            };

            // Ships can navigate sea, harbor, and naval base sectors
            if !is_navigable(dest_sect.sector_type) {
                out.push_str(&format!(
                    "1 Ship {}: {} is not navigable\n",
                    ship.uid,
                    ctx.format_xy(nx, ny),
                ));
                break;
            }

            ship.mobil = ship.mobil.saturating_sub(1);
            ship.x = nx;
            ship.y = ny;
            path_taken.push(DIRCH[dir_idx as usize]);
        }

        if !path_taken.is_empty() {
            let at_xy = ctx.format_xy(ship.x, ship.y);
            let path_str: String = path_taken.iter().collect();
            out.push_str(&format!(
                "1 Ship {} navigated {}: now at {}\n",
                ship.uid, path_str, at_xy,
            ));

            if let Err(e) = ships::put(ctx.db, &ship).await {
                out.push_str(&format!("1 Ship {}: save error: {e}\n", ship.uid));
            }
        }
    }

    if processed == 0 {
        out.push_str("1 No matching ships.\n");
    }
    out.push_str("0 navigate\n");
    out
}

/// True if ships can navigate through this sector type.
fn is_navigable(st: SectorType) -> bool {
    matches!(st, SectorType::Sea | SectorType::Harbor)
}

/// Determine if a ship uid matches the spec.
fn ship_matches(spec: &str, uid: i32) -> bool {
    if spec == "*" {
        return true;
    }
    if let Ok(n) = spec.trim_start_matches('#').parse::<i32>() {
        return uid == n;
    }
    false
}

/// Parse the route string into a sequence of direction indices (1–6).
/// If the route looks like "X,Y" coordinates, use pathfinding.
async fn build_route(
    route_str: &str,
    from_x: Coord,
    from_y: Coord,
    ctx: &CmdCtx<'_>,
) -> Result<Vec<u8>, String> {
    if let Some((rx, ry)) = parse_rel_xy(route_str) {
        let dx = ctx.x_abs(rx);
        let dy = ctx.y_abs(ry);

        let all_sects = sectors::get_all(ctx.db)
            .await
            .map_err(|e| format!("db error: {e}"))?;

        use std::collections::HashMap;
        let sect_map: HashMap<(Coord, Coord), SectorType> = all_sects
            .iter()
            .map(|s| ((s.x, s.y), s.sector_type))
            .collect();

        let dirs = find_path(from_x, from_y, dx, dy, ctx.world_x, ctx.world_y, |nx, ny| {
            match sect_map.get(&(nx, ny)) {
                Some(&st) => is_navigable(st),
                None => false,
            }
        });

        if dirs.is_empty() && (from_x != dx || from_y != dy) {
            return Err(format!(
                "no navigable path to {}",
                ctx.format_xy(dx, dy),
            ));
        }
        return Ok(dirs);
    }

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
