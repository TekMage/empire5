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
// Ported from: src/lib/commands/flee.c

// "fleetadd" command — assign a group of ships to a fleet letter, so the
// fleet can be used as a group designation on navigate/fire/torpedo/tend.
//
// Usage: fleetadd FLEET-LETTER SHIP-SPEC
//   fleetadd c *          put all owned ships into fleet c
//   fleetadd c 0-5        put ships 0-5 into fleet c
//   fleetadd d ~          put all unassigned ships into fleet d
//   fleetadd e c          rename/merge fleet c's ships into fleet e
//   fleetadd ~ c          clear fleet c's ships back to unassigned
//
// A ship joining a fleet clears its own group-retreat flag first; if
// another of your ships is already in the target fleet at the exact same
// square with a group-retreat plan set, the joining ship inherits that
// plan (retreat path + flags) — mirrors flee.c's snxtitem_group() lookup,
// restricted here to your own ships only (the C version's ship-of-any-
// nation lookup at that step reads as an oversight rather than intent).
// Since empire5 has no retreat-execution engine yet, this inheritance is
// inert data-only for now, same as the fields themselves.
//
// SHIP-SPEC also accepts the same ?realm=N/&type=X suffix filters as
// 'ship' (see 'info ship'/'info realm'), e.g.:
//   fleetadd c *?realm=2&type=can   put your nuc carriers in realm 2
//                                   into fleet c

use empire_db::ships;
use empire_types::ship::RetreatFlags;
use empire_types::ship_chr::ShipChr;

use super::ctx::CmdCtx;
use super::sector_sel::{in_range_wrap, parse_unit_filters, resolve_realm_filter};
use crate::subs::shpsub::ship_spec_matches;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: fleetadd FLEET-LETTER SHIP-SPEC\n".to_string();
    }

    let letter_arg = parts[0];
    let (ship_spec, filters) = parse_unit_filters(parts[1]);
    let realm = match resolve_realm_filter(&filters, ctx).await {
        Ok(r) => r,
        Err(e) => return format!("10 {e}\n"),
    };
    let type_filter = filters.iter().find(|(k, _)| *k == "type").map(|(_, v)| *v);

    let new_fleet = if letter_arg == "~" {
        ' '
    } else if letter_arg.len() == 1 && letter_arg.chars().next().unwrap().is_ascii_alphabetic() {
        letter_arg.chars().next().unwrap()
    } else {
        return format!(
            "10 '{letter_arg}' is not a valid fleet letter (use a-z, A-Z, or '~' to clear)\n"
        );
    };

    let mut all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let target_uids: Vec<i32> = all_ships.iter()
        .filter(|s| s.own == ctx.cnum && ship_spec_matches(ship_spec, s))
        .filter(|s| match &realm {
            Some(r) => in_range_wrap(s.x, r.xl, r.xh, ctx.world_x as i16)
                && in_range_wrap(s.y, r.yl, r.yh, ctx.world_y as i16),
            None => true,
        })
        .filter(|s| match type_filter {
            Some(t) => ShipChr::for_type(s.ship_type as usize)
                .map(|c| c.sname.eq_ignore_ascii_case(t)
                    || c.name.to_lowercase().contains(&t.to_lowercase()))
                .unwrap_or(false),
            None => true,
        })
        .map(|s| s.uid)
        .collect();

    if target_uids.is_empty() {
        return "10 No ships match that specification.\n".to_string();
    }

    let mut n = 0u32;
    for uid in target_uids {
        let Some(idx) = all_ships.iter().position(|s| s.uid == uid) else { continue };
        if all_ships[idx].fleet == new_fleet {
            continue;
        }

        all_ships[idx].retreat_flags.remove(RetreatFlags::GROUP);

        let (mx, my) = (all_ships[idx].x, all_ships[idx].y);
        let inherited = all_ships.iter()
            .find(|s| {
                s.uid != uid
                    && s.own == ctx.cnum
                    && s.fleet == new_fleet
                    && s.x == mx && s.y == my
                    && s.retreat_flags.contains(RetreatFlags::GROUP)
            })
            .map(|s| (s.retreat_path.clone(), s.retreat_flags));

        if let Some((path, flags)) = inherited {
            all_ships[idx].retreat_path = path;
            all_ships[idx].retreat_flags = flags;
        }

        all_ships[idx].fleet = new_fleet;
        if let Err(e) = ships::put(ctx.db, &all_ships[idx]).await {
            return format!("10 DB error saving ship #{uid}: {e}\n");
        }
        n += 1;
    }

    let plural = if n == 1 { "" } else { "s" };
    let mut out = String::new();
    if new_fleet == ' ' {
        out.push_str(&format!("1 {n} ship{plural} cleared from their fleet\n"));
    } else {
        out.push_str(&format!("1 {n} ship{plural} added to fleet `{new_fleet}'\n"));
    }
    out.push_str("0 fleetadd\n");
    out
}
