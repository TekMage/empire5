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
// Ported from: src/lib/commands/army.c

// "army" command — assign a group of land units to an army letter, so the
// army can be used as a group designation on march/attack/fire.
//
// Usage: army ARMY-LETTER UNIT-SPEC
//   army c *          put all owned units into army c
//   army c 0-5        put units 0-5 into army c
//   army d ~          put all unassigned units into army d
//   army e c          rename/merge army c's units into army e
//   army ~ c          clear army c's units back to unassigned
//
// A unit joining an army clears its own group-retreat flag first; if
// another of your units is already in the target army at the exact same
// square with a group-retreat plan set, the joining unit inherits that
// plan (retreat path + flags) — mirrors army.c's snxtitem_group() lookup,
// restricted here to your own units only, same as fleetadd. Since
// empire5 has no retreat-execution engine yet, this inheritance is inert
// data-only for now.
//
// UNIT-SPEC also accepts the same ?realm=N/&type=X suffix filters as
// 'land' (see 'info land'/'info realm'), e.g.:
//   army c *?realm=2&type=cav   put your cavalry in realm 2 into army c

use empire_db::land_units;
use empire_types::land_chr::LandChr;
use empire_types::ship::RetreatFlags;

use super::ctx::CmdCtx;
use super::sector_sel::{in_range_wrap, parse_unit_filters, resolve_realm_filter};
use crate::subs::lndsub::land_spec_matches;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: army ARMY-LETTER UNIT-SPEC\n".to_string();
    }

    let letter_arg = parts[0];
    let (unit_spec, filters) = parse_unit_filters(parts[1]);
    let realm = match resolve_realm_filter(&filters, ctx).await {
        Ok(r) => r,
        Err(e) => return format!("10 {e}\n"),
    };
    let type_filter = filters.iter().find(|(k, _)| *k == "type").map(|(_, v)| *v);

    let new_army = if letter_arg == "~" {
        ' '
    } else if letter_arg.len() == 1 && letter_arg.chars().next().unwrap().is_ascii_alphabetic() {
        letter_arg.chars().next().unwrap()
    } else {
        return format!(
            "10 '{letter_arg}' is not a valid army letter (use a-z, A-Z, or '~' to clear)\n"
        );
    };

    let mut all_units = match land_units::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let target_uids: Vec<i32> = all_units.iter()
        .filter(|u| u.own == ctx.cnum && land_spec_matches(unit_spec, u))
        .filter(|u| match &realm {
            Some(r) => in_range_wrap(u.x, r.xl, r.xh, ctx.world_x as i16)
                && in_range_wrap(u.y, r.yl, r.yh, ctx.world_y as i16),
            None => true,
        })
        .filter(|u| match type_filter {
            Some(t) => LandChr::for_type(u.land_type as usize)
                .map(|c| c.sname.eq_ignore_ascii_case(t)
                    || c.name.to_lowercase().contains(&t.to_lowercase()))
                .unwrap_or(false),
            None => true,
        })
        .map(|u| u.uid)
        .collect();

    if target_uids.is_empty() {
        return "10 No land units match that specification.\n".to_string();
    }

    let mut n = 0u32;
    for uid in target_uids {
        let Some(idx) = all_units.iter().position(|u| u.uid == uid) else { continue };
        if all_units[idx].army == new_army {
            continue;
        }

        all_units[idx].retreat_flags.remove(RetreatFlags::GROUP);

        let (mx, my) = (all_units[idx].x, all_units[idx].y);
        let inherited = all_units.iter()
            .find(|u| {
                u.uid != uid
                    && u.own == ctx.cnum
                    && u.army == new_army
                    && u.x == mx && u.y == my
                    && u.retreat_flags.contains(RetreatFlags::GROUP)
            })
            .map(|u| (u.retreat_path.clone(), u.retreat_flags));

        if let Some((path, flags)) = inherited {
            all_units[idx].retreat_path = path;
            all_units[idx].retreat_flags = flags;
        }

        all_units[idx].army = new_army;
        if let Err(e) = land_units::put(ctx.db, &all_units[idx]).await {
            return format!("10 DB error saving land unit #{uid}: {e}\n");
        }
        n += 1;
    }

    let plural = if n == 1 { "" } else { "s" };
    let mut out = String::new();
    if new_army == ' ' {
        out.push_str(&format!("1 {n} unit{plural} cleared from their army\n"));
    } else {
        out.push_str(&format!("1 {n} unit{plural} added to army `{new_army}'\n"));
    }
    out.push_str("0 army\n");
    out
}
