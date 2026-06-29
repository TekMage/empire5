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
// Ported from: src/lib/commands/assa.c

// "assault" command — land troops and civilians from a ship onto a coastal sector.
//
// Usage: assault SECT SHIP-SPEC [TROOPS]
//
// SECT     — target sector (player-relative coords)
// SHIP-SPEC — ship uid or "*" for any eligible ship
// TROOPS   — how many mil to land (default: all available)
//
// Ship must be in a sea sector adjacent to the target, or already at the target.
// The target must be coastal (coa=1) unless the ship has the LAND flag.
//
// SEMI ships (M_SEMILAND) can land up to 1/4 of their mil in combat vs defenders.
// LAND ships (M_LAND) can land all mil and civilian colonists.
// For unowned or own sectors: all mil and civs may land (no landing limit).
//
// Combat vs enemy-owned sectors is not yet fully implemented (placeholder).

use empire_db::{sectors, ships};
use empire_types::commodity::Item;
use empire_types::sector::SectorType;
use empire_types::ship_chr::{ShipChr, ShipChrFlags};
use crate::subs::geo::neighbors;
use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return "10 Usage: assault SECT SHIP-SPEC [TROOPS]\n".to_string();
    }

    // Parse target sector
    let Some((rx, ry)) = parse_rel_xy(parts[0]) else {
        return format!("10 Bad sector: {}\n", parts[0]);
    };
    let tx = ctx.x_abs(rx);
    let ty = ctx.y_abs(ry);

    let ship_spec = parts[1].trim();
    let troop_req: Option<i16> = parts.get(2)
        .and_then(|s| s.trim().parse().ok());

    // Load target sector
    let Ok(Some(mut target)) = sectors::get_at(ctx.db, tx, ty).await else {
        return format!("10 Sector {},{} doesn't exist\n", rx, ry);
    };

    // Cannot assault mountains, sea, sanctuaries
    {
        use empire_types::sector_chr::SectorChr;
        let dchr = SectorChr::for_type(target.sector_type);
        if dchr.is_water {
            return format!("1 {},{} is a sea sector — cannot assault\n0 assault\n", rx, ry);
        }
        if target.sector_type == SectorType::Sanctuary {
            return format!("1 {},{} is a sanctuary\n0 assault\n", rx, ry);
        }
    }

    // Load all ships, find one that:
    //   (a) belongs to us
    //   (b) matches the spec
    //   (c) is in a sea sector adjacent to the target (or at target if coastal)
    let all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();

    // Precompute neighbors of target so we can check adjacency
    let adj = neighbors(tx, ty, ctx.world_x, ctx.world_y);

    let mut found_ship: Option<empire_types::ship::Ship> = None;
    for s in &all_ships {
        if s.own != ctx.cnum && !ctx.is_deity { continue; }
        if s.own == 0 { continue; }
        if !ship_matches(ship_spec, s.uid) { continue; }

        // Ship must be adjacent to (or at) the target
        let at_target = s.x == tx && s.y == ty;
        let is_adj = adj.iter().any(|&(nx, ny)| s.x == nx && s.y == ny);
        if !at_target && !is_adj {
            out.push_str(&format!(
                "1 Ship {} is not adjacent to {},{}\n", s.uid, rx, ry
            ));
            continue;
        }

        found_ship = Some(s.clone());
        break;
    }

    let Some(mut ship) = found_ship else {
        if out.is_empty() {
            out.push_str(&format!("1 No ship adjacent to {},{}\n", rx, ry));
        }
        out.push_str("0 assault\n");
        return out;
    };

    let shpchr = match ShipChr::for_type(ship.ship_type as usize) {
        Some(c) => c,
        None => {
            out.push_str("1 Unknown ship type\n0 assault\n");
            return out;
        }
    };

    let can_land  = shpchr.flags.contains(ShipChrFlags::LAND);
    let can_semi  = shpchr.flags.contains(ShipChrFlags::SEMI_LAND);
    if !can_land && !can_semi {
        out.push_str(&format!(
            "1 {} has no landing capability\n0 assault\n", shpchr.name
        ));
        return out;
    }

    let ship_mil = ship.items.get(Item::Milit);
    let ship_civ = ship.items.get(Item::Civil);

    // --- Unowned or own sector: land everything, no combat ---
    if target.own == 0 || target.own == ctx.cnum {
        if ship_mil == 0 && ship_civ == 0 {
            out.push_str("1 No troops or civilians aboard to land\n0 assault\n");
            return out;
        }

        // Determine how many mil land (default: all; caller may restrict)
        let mil_to_land = if let Some(req) = troop_req {
            req.min(ship_mil).max(0)
        } else {
            ship_mil
        };

        // For unowned sectors (colonization) or own sectors: all passengers land.
        // SEMI/LAND restriction only applies to combat against enemy sectors.
        let civ_to_land = ship_civ;

        // Move mil from ship to sector
        if mil_to_land > 0 {
            ship.items.add(Item::Milit, -mil_to_land);
            target.items.add(Item::Milit, mil_to_land);
        }
        // Move civs
        if civ_to_land > 0 {
            ship.items.add(Item::Civil, -civ_to_land);
            target.items.add(Item::Civil, civ_to_land);
        }

        // Claim unowned sector
        if target.own == 0 {
            target.own = ctx.cnum;
            target.old_own = ctx.cnum;
            if target.effic < 1 { target.effic = 1; }
            out.push_str(&format!(
                "1 {},{} captured!\n", rx, ry
            ));
        }

        if mil_to_land > 0 {
            out.push_str(&format!("1 Landed {} military at {},{}\n", mil_to_land, rx, ry));
        }
        if civ_to_land > 0 {
            out.push_str(&format!("1 Landed {} civilians at {},{}\n", civ_to_land, rx, ry));
        }

        // Save ship and sector
        if let Err(e) = ships::put(ctx.db, &ship).await {
            out.push_str(&format!("1 Error saving ship: {e}\n"));
        }
        if let Err(e) = sectors::put(ctx.db, &target).await {
            out.push_str(&format!("1 Error saving sector: {e}\n"));
        }

        out.push_str("0 assault\n");
        return out;
    }

    // --- Enemy sector: not yet fully implemented ---
    // SEMI ships land 1/4 of mil; LAND ships land all.
    let max_landing_mil = if can_land {
        ship_mil
    } else {
        ship_mil / 4  // SEMI: 25% landing capacity
    };
    let troops = if let Some(req) = troop_req {
        req.min(max_landing_mil).max(0)
    } else {
        max_landing_mil
    };

    if troops <= 0 {
        out.push_str(&format!(
            "1 {} has insufficient mil to assault (max landing: {})\n",
            shpchr.name, max_landing_mil
        ));
        out.push_str("0 assault\n");
        return out;
    }

    let def_mil = target.items.get(Item::Milit);
    let def_civ = target.items.get(Item::Civil);

    // Simplified combat: attacker needs 3× defender's strength to win
    let att_str = troops as f64 * (ship.effic as f64 / 100.0);
    let def_str = def_mil as f64 + def_civ as f64 * 0.1 + target.effic as f64 * 0.5;

    out.push_str(&format!(
        "1 Assaulting {},{} with {} troops (att str {:.0} vs def str {:.0})\n",
        rx, ry, troops, att_str, def_str
    ));

    if att_str > def_str {
        // Attacker wins: pay casualties and take sector
        let att_loss = ((def_str * 0.5) as i16).max(0);
        let def_loss = def_mil.min((att_str * 0.8) as i16);
        let survivors = (troops - att_loss).max(1);

        ship.items.add(Item::Milit, -(troops - survivors));
        target.items.add(Item::Milit, -def_loss);
        target.items.add(Item::Milit,  survivors);

        let old_own = target.own;
        target.own = ctx.cnum;
        target.old_own = 0;  // recently captured, not yet normalized
        target.loyal = 0;

        out.push_str(&format!(
            "1 Assault successful! {} survives, sector taken from country {}\n",
            survivors, old_own
        ));

        if let Err(e) = ships::put(ctx.db, &ship).await { out.push_str(&format!("1 Ship save error: {e}\n")); }
        if let Err(e) = sectors::put(ctx.db, &target).await { out.push_str(&format!("1 Sector save error: {e}\n")); }
    } else {
        // Defender wins
        let att_loss = (troops as f64 * 0.8) as i16;
        ship.items.add(Item::Milit, -att_loss);
        out.push_str(&format!(
            "1 Assault repelled! Lost {} troops\n", att_loss
        ));
        if let Err(e) = ships::put(ctx.db, &ship).await { out.push_str(&format!("1 Ship save error: {e}\n")); }
    }

    out.push_str("0 assault\n");
    out
}

fn ship_matches(spec: &str, uid: i32) -> bool {
    if spec == "*" { return true; }
    if let Ok(n) = spec.trim_start_matches('#').parse::<i32>() {
        return uid == n;
    }
    false
}
