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
// Ported from: src/lib/commands/torp.c (c_torpedo)

// "torpedo" command — ships with torpedo capability attack an enemy ship.
//
// Usage: torpedo <ship-spec> <target-ship-uid>
//   torpedo 3 12        fire ship #3's torpedoes at ship #12
//   torp 0-2 12         ships 0-2 each take a shot at ship #12
//   torp c 12           every ship in fleet c takes a shot at ship #12
//
// <ship-spec> accepts a uid, a uid range, a comma list, "*", "~" (ships
// with no fleet assigned), or a single letter naming a fleet (see
// 'info fleetadd').
//
// Unlike deck guns (`fire`), torpedoes always cost 3 shells and mobility
// whether or not they hit — see info/torpedo for the full mechanics and
// documented v1 gaps (no line-of-sight check, no submerged/surfaced ship
// state).

use rand::SeedableRng;
use rand::rngs::StdRng;

use empire_db::{news, ships};
use empire_types::news::NewsVerb;
use empire_types::ship_chr::{ShipChr, ShipChrFlags};

use super::ctx::CmdCtx;
use crate::subs::geo::map_dist;
use crate::subs::shpsub::{ship_spec_matches, shp_can_torp, shp_torp_at_ship, shp_torp_range};

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: torpedo <ship-uid-spec> <target-ship-uid>\n".to_string();
    }
    let firer_spec = parts[0];
    let Ok(target_uid) = parts[1].parse::<i32>() else {
        return format!("10 Bad ship uid '{}'\n", parts[1]);
    };

    let mut all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let firer_uids: Vec<i32> = all_ships.iter()
        .filter(|s| (s.own == ctx.cnum || ctx.is_deity) && ship_spec_matches(firer_spec, s))
        .map(|s| s.uid)
        .collect();
    if firer_uids.is_empty() {
        return "10 No ships match that specification.\n".to_string();
    }
    if firer_uids.contains(&target_uid) {
        return "10 A ship can't torpedo itself\n".to_string();
    }

    let Some(mut target) = all_ships.iter().find(|s| s.uid == target_uid).cloned() else {
        return format!("10 No ship #{target_uid}\n");
    };
    let Some(def_mchr) = ShipChr::for_type(target.ship_type as usize) else {
        return "10 Unknown target ship type\n".to_string();
    };

    let mut out = String::new();
    let mut rng = StdRng::from_entropy();

    for firer_uid in firer_uids.iter().copied() {
        let Some(idx) = all_ships.iter().position(|s| s.uid == firer_uid) else { continue };
        let Some(att_mchr) = ShipChr::for_type(all_ships[idx].ship_type as usize) else {
            out.push_str(&format!("1 Ship #{firer_uid}: unknown type — skipped\n"));
            continue;
        };

        if !shp_can_torp(&all_ships[idx], att_mchr) {
            out.push_str(&format!(
                "1 Ship #{firer_uid}: not eligible to fire torpedoes (effic, TORP capability, ammo, or mobility)\n"
            ));
            continue;
        }
        if def_mchr.flags.contains(ShipChrFlags::SUBMARINE) && !att_mchr.flags.contains(ShipChrFlags::SUB_TORP) {
            out.push_str(&format!(
                "1 Ship #{firer_uid}: can't target a submarine (needs sub-torpedo capability)\n"
            ));
            continue;
        }

        let dist = map_dist(all_ships[idx].x, all_ships[idx].y, target.x, target.y, ctx.world_x, ctx.world_y);
        let range = shp_torp_range(&all_ships[idx], att_mchr);
        if dist > range {
            out.push_str(&format!("1 Ship #{firer_uid}: target out of torpedo range\n"));
            continue;
        }

        let attacker = &mut all_ships[idx];
        let result = shp_torp_at_ship(attacker, att_mchr, &mut target, def_mchr, dist, &mut rng);

        if result.hit {
            out.push_str(&format!(
                "1 Ship #{firer_uid}: torpedo hits! {} damage to ship #{target_uid} (effic now {}%)\n",
                result.damage, target.effic
            ));
            if target.own != ctx.cnum && target.own != 0 {
                let _ = news::add_news(ctx.db, ctx.cnum, NewsVerb::ShipTorp as u8, target.own, 1).await;
            }
            if result.target_sunk {
                out.push_str(&format!("1 Ship #{target_uid} sinks!\n"));
            } else if shp_can_torp(&target, def_mchr) {
                let counter_dist = dist; // symmetric distance
                let counter = shp_torp_at_ship(&mut target, def_mchr, attacker, att_mchr, counter_dist, &mut rng);
                if counter.hit {
                    out.push_str(&format!(
                        "1 Ship #{target_uid} returns fire: {} damage to ship #{firer_uid} (effic now {}%)\n",
                        counter.damage, attacker.effic
                    ));
                    if counter.target_sunk {
                        out.push_str(&format!("1 Ship #{firer_uid} sinks!\n"));
                    }
                } else {
                    out.push_str(&format!("1 Ship #{target_uid} fires back but misses.\n"));
                }
            }
        } else {
            out.push_str(&format!("1 Ship #{firer_uid}: torpedo misses.\n"));
        }
    }

    for uid in firer_uids.iter().copied() {
        if let Some(s) = all_ships.iter().find(|s| s.uid == uid) {
            let _ = ships::put(ctx.db, s).await;
        }
    }
    let _ = ships::put(ctx.db, &target).await;

    if out.is_empty() {
        out.push_str("1 Nothing fired.\n");
    }
    out.push_str("0 torpedo\n");
    out
}
