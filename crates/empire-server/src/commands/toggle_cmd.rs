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
// Ported from: src/lib/commands/togg.c

// "toggle" command — set or report per-nation behavior flags.
//
// Usage: toggle [<flag>] [on|off]
//   toggle                shows the status of every flag
//   toggle coastwatch      flips coastwatch (on->off, off->on)
//   toggle coastwatch on   forces coastwatch on
//   toggle c off           first-letter matching works, same as 4.4.1
//
// Flags: inform, flash, beep, coastwatch, sonar, techlists — matching
// 4.4.1's NF_INFORM/NF_FLASH/NF_BEEP/NF_COASTWATCH/NF_SONAR/NF_TECHLISTS.
// Of these, only coastwatch currently changes server behavior (see
// 'info coastwatch' / crate::subs::interdict) — the rest are stored for
// client-side use and forward-compatibility, matching their original
// mostly-client-hint role in 4.4.1.

use empire_db::nations;
use empire_types::nation::NatFlags;
use super::ctx::CmdCtx;

const ALL_FLAGS: &[(char, &str, NatFlags)] = &[
    ('i', "inform", NatFlags::INFORM),
    ('f', "flash", NatFlags::FLASH),
    ('b', "beep", NatFlags::BEEP),
    ('c', "coastwatch", NatFlags::COASTWATCH),
    ('s', "sonar", NatFlags::SONAR),
    ('t', "techlists", NatFlags::TECHLISTS),
];

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let mut nat = match nations::get_by_cnum(ctx.db, ctx.cnum).await {
        Ok(Some(n)) => n,
        Ok(None) => return "10 Nation not found\n".to_string(),
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        let mut out = String::new();
        for &(_, name, flag) in ALL_FLAGS {
            out.push_str(&format!("1 {name} flag {}\n", on_off(nat.flags.contains(flag))));
        }
        out.push_str("0 toggle\n");
        return out;
    }

    let key = parts[0].to_ascii_lowercase();
    let Some(&(_, name, flag)) = ALL_FLAGS.iter().find(|(c, n, _)| {
        key.chars().next() == Some(*c) || key == *n
    }) else {
        return format!(
            "10 Unknown flag '{}'. Try: inform, flash, beep, coastwatch, sonar, techlists\n",
            parts[0]
        );
    };

    let pos = match parts.get(1).map(|s| s.to_ascii_lowercase()) {
        Some(s) if s == "on" => true,
        Some(s) if s == "off" => false,
        Some(_) => return "10 Usage: toggle <flag> [on|off]\n".to_string(),
        None => !nat.flags.contains(flag),
    };

    nat.flags.set(flag, pos);
    if let Err(e) = nations::put(ctx.db, &nat).await {
        return format!("10 database error: {e}\n");
    }

    format!("1 {name} flag {}\n0 toggle\n", on_off(pos))
}

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}
