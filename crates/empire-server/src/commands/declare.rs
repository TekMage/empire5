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
// Ported from: src/lib/commands/decl.c

// "declare" command — set diplomatic stance toward other nations.
// Usage: declare <alliance|friendly|neutrality|hostility|war> <country-spec>
//   e.g. declare war russia
//   e.g. declare allied 2
//   e.g. declare neutral *

use empire_db::{nations, relations, news};
use empire_types::nation::NatStatus;
use empire_types::news::NewsVerb;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return "10 Usage: declare <alliance|friendly|neutral|hostile|war> <country>\n".to_string();
    }

    let rel_word = parts[0].trim();
    let rel = match relations::Relation::from_char(rel_word.chars().next().unwrap_or(' ')) {
        Some(r) => r,
        None => return format!(
            "10 Unknown declaration: '{}' (use alliance/friendly/neutral/hostile/war)\n",
            rel_word
        ),
    };

    let target_spec = parts[1].trim();
    let as_cnum = if ctx.is_deity && parts.len() >= 3 {
        match parts[2].trim().parse::<u8>() {
            Ok(n) => n,
            Err(_) => return "10 Invalid deity cnum argument.\n".to_string(),
        }
    } else {
        ctx.cnum
    };

    let all_nations = match nations::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let targets: Vec<u8> = if target_spec == "*" {
        all_nations.iter()
            .filter(|n| n.cnum != as_cnum && n.status >= NatStatus::Sanct)
            .map(|n| n.cnum)
            .collect()
    } else {
        // Try number, then name prefix
        if let Ok(n) = target_spec.parse::<u8>() {
            vec![n]
        } else {
            let tgt_lc = target_spec.to_lowercase();
            all_nations.iter()
                .filter(|n| n.name.to_lowercase().starts_with(&tgt_lc))
                .map(|n| n.cnum)
                .collect()
        }
    };

    if targets.is_empty() {
        return format!("10 No such country: '{}'\n", target_spec);
    }

    let mut out = String::new();
    for target_cnum in targets {
        if target_cnum == as_cnum { continue; }

        let current = relations::get(ctx.db, as_cnum, target_cnum).await
            .unwrap_or(relations::Relation::Neutral);

        if current == rel {
            continue; // already at this relation
        }

        match relations::set(ctx.db, as_cnum, target_cnum, rel).await {
            Ok(_) => {
                let tgt_name = all_nations.iter()
                    .find(|n| n.cnum == target_cnum)
                    .map(|n| n.name.as_str())
                    .unwrap_or("unknown");
                out.push_str(&format!(
                    "1 Declared {} toward {} (#{target_cnum})\n",
                    rel.name(), tgt_name
                ));
                // File news event for this relations change
                let verb = relation_news_verb(current, rel);
                let _ = news::add_news(ctx.db, as_cnum, verb as u8, target_cnum, 1).await;
            }
            Err(e) => {
                out.push_str(&format!("1 Error updating relation: {e}\n"));
            }
        }
    }

    if out.is_empty() {
        out.push_str("1 No relations changed.\n");
    }
    out.push_str("0 declare\n");
    out
}

/// Map old→new relation change to the appropriate news verb.
fn relation_news_verb(old: relations::Relation, new: relations::Relation) -> NewsVerb {
    use relations::Relation::*;
    match new {
        AtWar    => NewsVerb::DeclWar,
        Allied   => NewsVerb::DeclAlly,
        Neutral  => {
            if old > Neutral { NewsVerb::DownNeutral } else { NewsVerb::UpNeutral }
        }
        Hostile  => {
            if old > Hostile { NewsVerb::DownHostile } else { NewsVerb::UpHostile }
        }
        Friendly => {
            if old > Friendly { NewsVerb::DownFriendly } else { NewsVerb::UpFriendly }
        }
    }
}
