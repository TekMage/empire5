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
// SHIP-SPEC: a ship uid ("5"), a uid range ("0-5"), a comma list, "*" for
//   all owned ships, "~" for ships with no fleet assigned, or a single
//   letter naming a fleet (see 'info fleetadd').
// ROUTE: a direction string (e.g. "uujnb") or a destination "X,Y"
//        (player-relative) which triggers pathfinding.
//
// Ships can move through sea, harbor, and naval base sectors.
// Mobility cost per step: 1 mob unit (simplified).
//
// Any planes aboard (see 'info load') travel with the ship — their
// x/y is synced to the ship's final position once the route
// completes, matching the ship itself only persisting once at the
// end rather than per intermediate step.

use empire_db::{planes, sectors, ships};
use empire_types::coords::Coord;
use empire_types::sector::SectorType;
use empire_types::sector_chr::SectorChr;
use empire_types::ship_chr::{ShipChr, ShipChrFlags};
use crate::subs::geo::{DIROFF, DIRCH, DIR_FIRST, DIR_LAST, x_norm, y_norm, dir_from_char};
use crate::subs::pathfind::find_path;
use crate::subs::shpsub::ship_spec_matches;
use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;

/// Sentinel pushed into the direction sequence for a 'v' token — "view the
/// current sector's oil/fertility without moving," matching 4.4.1's
/// unit_view(). Not a real direction, so it's outside the 1-6 range.
const VIEW_MARKER: u8 = 255;

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
        if !ship_spec_matches(ship_spec, &ship) {
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
            if dir_idx == VIEW_MARKER {
                out.push_str(&view_line(ctx, &ship).await);
                continue;
            }
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
            } else {
                // Planes aboard travel with the ship — sync their
                // position to wherever it ended up (only the final
                // position, matching the ship itself only persisting
                // once at the end of the route, not per intermediate step).
                match planes::get_on_ship(ctx.db, ship.uid).await {
                    Ok(aboard) => {
                        for mut plane in aboard {
                            plane.x = ship.x;
                            plane.y = ship.y;
                            if let Err(e) = planes::put(ctx.db, &plane).await {
                                out.push_str(&format!(
                                    "1 Plane #{} (aboard ship {}): save error: {e}\n",
                                    plane.uid, ship.uid
                                ));
                            }
                        }
                    }
                    Err(e) => out.push_str(&format!(
                        "1 Warning: could not load planes aboard ship {}: {e}\n", ship.uid
                    )),
                }
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
            _ if ch == 'v' => dirs.push(VIEW_MARKER),
            _ => return Err(format!("unknown direction character '{ch}'")),
        }
    }
    Ok(dirs)
}

/// Report the ship's current sector: oil content (if it can drill) and
/// fertility (if it can fish), plus efficiency and sector type.
/// Mirrors unit_view() in src/lib/subs/unitsub.c — triggered by 'v' in the
/// route string, doesn't move the ship or cost mobility.
async fn view_line(ctx: &CmdCtx<'_>, ship: &empire_types::ship::Ship) -> String {
    let Ok(Some(sect)) = sectors::get_at(ctx.db, ship.x, ship.y).await else {
        return format!("1 Ship {}: sector vanished\n", ship.uid);
    };
    let mut prefix = String::new();
    if let Some(shpchr) = ShipChr::for_type(ship.ship_type as usize) {
        if shpchr.flags.contains(ShipChrFlags::FISH) {
            prefix.push_str(&format!("[fert:{}] ", sect.fertil));
        }
        if shpchr.flags.contains(ShipChrFlags::OIL) {
            prefix.push_str(&format!("[oil:{}] ", sect.oil));
        }
    }
    format!(
        "1 {}Ship {} @ {} {}% {}\n",
        prefix, ship.uid, ctx.format_xy(ship.x, ship.y),
        sect.effic, SectorChr::for_type(sect.sector_type).name,
    )
}
