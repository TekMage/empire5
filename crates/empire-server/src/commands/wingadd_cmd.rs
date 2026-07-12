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
// Ported from: src/lib/commands/wing.c
// Known contributors to the original:
//    Ron Koenderink, 2005

// "wingadd" command — assign a group of planes to a wing letter, so the
// wing letter can later be used as a PLANE-SPEC on bomb/fly missions.
//
// Usage: wingadd WING-LETTER PLANE-SPEC
//
// WING-LETTER: a single letter (a-z/A-Z) to name the wing, or "~" to
// clear the wing assignment (put the planes back in the null wing).
// PLANE-SPEC: any spec accepted by bomb/fly — "*", a uid, a uid range
// ("0-5"), a comma list, "~" (planes with no wing), or another wing
// letter (merges/renames that wing's planes into the new one).

use empire_db::planes;

use super::ctx::CmdCtx;
use crate::subs::plnsub::plane_spec_matches;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: wingadd WING-LETTER PLANE-SPEC\n".to_string();
    }

    let letter_arg = parts[0];
    let plane_spec = parts[1];

    let new_wing = if letter_arg == "~" {
        ' '
    } else if letter_arg.len() == 1 && letter_arg.chars().next().unwrap().is_ascii_alphabetic() {
        letter_arg.chars().next().unwrap()
    } else {
        return format!(
            "10 '{letter_arg}' is not a valid wing letter (use a-z, A-Z, or '~' to clear)\n"
        );
    };

    let all_planes = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let selected: Vec<_> = all_planes
        .into_iter()
        .filter(|p| p.own == ctx.cnum && plane_spec_matches(plane_spec, p))
        .collect();

    if selected.is_empty() {
        return "10 No planes match that specification.\n".to_string();
    }

    let mut n = 0u32;
    for mut plane in selected {
        plane.wing = new_wing;
        if let Err(e) = planes::put(ctx.db, &plane).await {
            return format!("10 DB error saving plane #{}: {e}\n", plane.uid);
        }
        n += 1;
    }

    let plural = if n == 1 { "" } else { "s" };
    let mut out = String::new();
    if new_wing == ' ' {
        out.push_str(&format!("1 {n} plane{plural} cleared from their wing\n"));
    } else {
        out.push_str(&format!("1 {n} plane{plural} added to wing `{new_wing}'\n"));
    }
    out.push_str("0 wingadd\n");
    out
}
