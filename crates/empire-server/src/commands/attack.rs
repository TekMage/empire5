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
// Ported from: src/lib/commands/atta.c
// Known contributors to the original:
//    Ken Stevens, 1995
//    Steve McClure, 1996-2000

// "attack" / "atta" command — ground combat.
//
// Usage: attack SECT [UNIT-SPEC] [EXTRA-MIL]
//
// SECT: target sector (player-relative "X,Y")
// UNIT-SPEC: optional land unit spec ("*", single uid, or comma-separated uids)
// EXTRA-MIL: optional integer amount of extra military to commit

use empire_db::{sectors, land_units, nations, relations};
use empire_types::commodity::Item;
use empire_types::land_chr::LandChr;

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;
use crate::subs::attsub::{att_resolve, at_war};
use crate::subs::takeover::takeover_sector;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        return "10 Usage: attack SECT [UNIT-SPEC] [EXTRA-MIL]\n".to_string();
    }

    // Parse target sector coordinate
    let Some((rx, ry)) = parse_rel_xy(parts[0]) else {
        return format!("10 Bad sector specification: '{}'\n", parts[0]);
    };
    let tx = ctx.x_abs(rx);
    let ty = ctx.y_abs(ry);

    // Parse optional unit spec (default: none)
    let unit_spec = parts.get(1).copied().unwrap_or("none");
    // Parse optional extra military
    let extra_mil: i32 = parts.get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
        .max(0);

    // Load target sector
    let target_sector = match sectors::get_at(ctx.db, tx, ty).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 Sector {} doesn't exist\n", ctx.format_xy(tx, ty)),
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    // Cannot attack own sector
    if target_sector.own == ctx.cnum {
        return format!(
            "10 Sector {} is your own sector.\n",
            ctx.format_xy(tx, ty)
        );
    }

    // Cannot attack unowned sectors (nothing to conquer from no one)
    if target_sector.own == 0 && !ctx.is_deity {
        return format!(
            "10 Sector {} is unowned — use 'march' to occupy it.\n",
            ctx.format_xy(tx, ty)
        );
    }

    // Relation check — must be at war (deities exempt)
    if !ctx.is_deity && target_sector.own != 0 {
        let rel = match relations::get(ctx.db, ctx.cnum, target_sector.own).await {
            Ok(r)  => r,
            Err(e) => return format!("10 DB error: {e}\n"),
        };
        if !at_war(ctx.nat.status, rel) {
            // Load defender name for message
            let def_name = match nations::get_by_cnum(ctx.db, target_sector.own).await {
                Ok(Some(n)) => n.name,
                _ => format!("nation #{}", target_sector.own),
            };
            return format!(
                "10 You are not at war with {def_name}. Declare war first.\n"
            );
        }
    }

    // Gather attacking land units (owned by player, not on a ship, matching spec)
    let all_units = match land_units::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let att_units: Vec<_> = all_units.iter()
        .filter(|u| {
            u.own == ctx.cnum
                && u.ship < 0
                && u.carried_by_land < 0
                && crate::subs::lndsub::lnd_can_attack(u)
                && unit_matches(unit_spec, u.uid)
        })
        .cloned()
        .collect();

    // Gather defending units in the target sector
    let def_units: Vec<_> = all_units.iter()
        .filter(|u| u.own == target_sector.own && u.x == tx && u.y == ty && u.ship < 0)
        .cloned()
        .collect();

    let def_mil = target_sector.items.get(Item::Milit) as i32;

    // Tech factors
    let tech_att = 1.0 + ctx.nat.tech / 100.0;
    let def_tech = if target_sector.own != 0 {
        match nations::get_by_cnum(ctx.db, target_sector.own).await {
            Ok(Some(n)) => 1.0 + n.tech / 100.0,
            _ => 1.0,
        }
    } else {
        1.0
    };

    let lchr = LandChr::all();

    // Use SmallRng (Send-safe) seeded from thread_rng so the rng doesn't
    // cross await points.
    let (result, rng_n) = {
        let mut rng = StdRng::from_entropy();
        let res = att_resolve(
            &att_units, lchr,
            extra_mil,
            &target_sector,
            &def_units, lchr,
            def_mil,
            tech_att, def_tech,
            &mut rng,
        );
        let n = rng.gen_range(0..100i32);
        (res, n)
    };

    let mut out = String::new();
    out.push_str(&format!(
        "1 Attack on {} by {} (#{})\n",
        ctx.format_xy(tx, ty),
        ctx.nat.name,
        ctx.cnum,
    ));
    out.push_str(&format!(
        "1 Attackers: {} land units + {} extra military\n",
        att_units.len(),
        extra_mil,
    ));
    out.push_str(&format!(
        "1 Defenders: {} military, {} land units\n",
        def_mil,
        def_units.len(),
    ));

    for line in &result.log {
        out.push_str(&format!("1 {line}\n"));
    }

    if result.sector_taken {
        // Take over the sector
        let mut new_sector = target_sector.clone();
        takeover_sector(&mut new_sector, ctx.cnum, 1.0, rng_n);

        if let Err(e) = sectors::put(ctx.db, &new_sector).await {
            out.push_str(&format!("1 Error saving sector: {e}\n"));
        }

        // Destroy defending units
        for mut unit in def_units {
            unit.effic = 0;
            let _ = land_units::put(ctx.db, &unit).await;
        }

        out.push_str(&format!(
            "1 You took sector {}! Att casualties: {}, Def casualties: {}\n",
            ctx.format_xy(tx, ty),
            result.att_casualties,
            result.def_casualties,
        ));
    } else {
        out.push_str(&format!(
            "1 Attack on {} failed. Att casualties: {}, Def casualties: {}\n",
            ctx.format_xy(tx, ty),
            result.att_casualties,
            result.def_casualties,
        ));
    }

    out.push_str("0 attack\n");
    out
}

/// Return true if `uid` matches the unit spec string.
/// Supports `*` (all), a single integer uid, or comma-separated uid list.
fn unit_matches(spec: &str, uid: i32) -> bool {
    if spec == "*" || spec == "none" {
        return spec == "*"; // "none" means no units selected
    }
    for part in spec.split(',') {
        let part = part.trim().trim_start_matches('#');
        if let Ok(n) = part.parse::<i32>() {
            if n == uid {
                return true;
            }
        }
    }
    false
}
