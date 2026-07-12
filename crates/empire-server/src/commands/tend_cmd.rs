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
// Ported from: src/lib/commands/tend.c

// "tend" command — transfer a commodity directly between two ships in the
// same sector.  Unlike load/unload, no harbor is required: this is how an
// oil derrick parked out at sea hands its oil off to a tanker without
// either ship needing to be in owned territory.
//
// Usage: tend <commodity> <tender-spec> <amount> <target-spec>
//   amount > 0: move FROM tender(s) TO target(s)
//   amount < 0: move FROM target(s) TO tender(s) (tender pulls in cargo)
//
// Both ships must be in the same sector.  The target must be yours, or
// owned by a nation at Friendly relations or better (mirrors 4.4.1's
// can_tend_to()).  Capacity is enforced per ship type (ShipChr::cargo_cap).

use empire_db::{relations, ships};
use empire_types::commodity::Item;
use empire_types::ship_chr::ShipChr;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 4 {
        return "10 Usage: tend <commodity> <tender-spec> <amount> <target-spec>\n".to_string();
    }

    let item = match parse_item(parts[0]) {
        Some(i) => i,
        None => return format!("10 Unknown commodity '{}'\n", parts[0]),
    };
    let tender_spec = parts[1];
    let amount: i16 = match parts[2].parse() {
        Ok(n) => n,
        Err(_) => return format!("10 Invalid amount '{}'\n", parts[2]),
    };
    if amount == 0 {
        return "10 Amount must be non-zero\n".to_string();
    }
    let target_spec = parts[3];

    let mut all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let tender_uids: Vec<i32> = all_ships.iter()
        .filter(|s| (s.own == ctx.cnum || ctx.is_deity) && matches_ship(s.uid, tender_spec))
        .map(|s| s.uid)
        .collect();
    if tender_uids.is_empty() {
        return format!("1 No ships match '{tender_spec}'\n0 tend\n");
    }
    let target_uids: Vec<i32> = all_ships.iter()
        .filter(|s| matches_ship(s.uid, target_spec))
        .map(|s| s.uid)
        .collect();
    if target_uids.is_empty() {
        return format!("1 No ships match '{target_spec}'\n0 tend\n");
    }

    let mut out = String::new();

    for &tender_uid in &tender_uids {
        let tender_idx = match all_ships.iter().position(|s| s.uid == tender_uid) {
            Some(i) => i,
            None => continue,
        };
        let (tx, ty) = (all_ships[tender_idx].x, all_ships[tender_idx].y);

        let mut total = 0i32;
        for &target_uid in &target_uids {
            if target_uid == tender_uid { continue; }
            let Some(target_idx) = all_ships.iter().position(|s| s.uid == target_uid) else { continue };

            // Same sector required — no harbor, no ownership needed for the
            // sector itself, just proximity.
            if all_ships[target_idx].x != tx || all_ships[target_idx].y != ty {
                continue;
            }
            if all_ships[target_idx].own == 0 { continue; }

            // Ownership / relations check on whichever side is receiving.
            let (giver_idx, receiver_idx) = if amount > 0 {
                (tender_idx, target_idx)
            } else {
                (target_idx, tender_idx)
            };
            if !ctx.is_deity {
                let receiver_own = all_ships[receiver_idx].own;
                if receiver_own != ctx.cnum {
                    let rel = relations::get(ctx.db, ctx.cnum, receiver_own).await
                        .unwrap_or(relations::Relation::Neutral);
                    if rel < relations::Relation::Friendly {
                        out.push_str(&format!(
                            "1 Not on friendly terms with the owner of ship {}\n",
                            all_ships[receiver_idx].uid
                        ));
                        continue;
                    }
                }
            }

            let give_have = all_ships[giver_idx].items.get(item);
            if give_have <= 0 {
                continue;
            }
            let Some(recv_chr) = ShipChr::for_type(all_ships[receiver_idx].ship_type as usize) else { continue };
            let recv_cap = recv_chr.cargo_cap(item);
            if recv_cap == 0 {
                out.push_str(&format!(
                    "1 Ship {} ({}) cannot hold any {}\n",
                    all_ships[receiver_idx].uid, recv_chr.name, item.name()
                ));
                continue;
            }
            let recv_have = all_ships[receiver_idx].items.get(item);
            let room = (recv_cap - recv_have).max(0);
            if room == 0 {
                out.push_str(&format!(
                    "1 Ship {} can't hold more {}\n",
                    all_ships[receiver_idx].uid, item.name()
                ));
                continue;
            }

            let transfer = give_have.min(amount.abs()).min(room);
            if transfer <= 0 { continue; }

            all_ships[giver_idx].items.add(item, -transfer);
            all_ships[receiver_idx].items.add(item, transfer);
            total += transfer as i32;

            // Ran the tender dry, or filled it up (when pulling in) — stop.
            if amount > 0 && all_ships[tender_idx].items.get(item) <= 0 { break; }
            if amount < 0 {
                let tender_have = all_ships[tender_idx].items.get(item);
                if tender_have >= ShipChr::for_type(all_ships[tender_idx].ship_type as usize)
                    .map(|c| c.cargo_cap(item)).unwrap_or(0)
                { break; }
            }
        }

        if total > 0 {
            out.push_str(&format!(
                "1 {} total {} transferred {} ship {}\n",
                total, item.name(),
                if amount > 0 { "off of" } else { "to" },
                tender_uid,
            ));
        }
    }

    for &uid in tender_uids.iter().chain(target_uids.iter()) {
        if let Some(s) = all_ships.iter().find(|s| s.uid == uid) {
            if let Err(e) = ships::put(ctx.db, s).await {
                out.push_str(&format!("1 Ship {uid} save error: {e}\n"));
            }
        }
    }

    if out.is_empty() {
        out.push_str("1 Nothing transferred\n");
    }
    out.push_str("0 tend\n");
    out
}

fn matches_ship(uid: i32, spec: &str) -> bool {
    if spec.is_empty() || spec == "*" { return true; }
    if let Ok(n) = spec.parse::<i32>() { return uid == n; }
    if let Some((lo, hi)) = spec.split_once('-') {
        if let (Ok(lo), Ok(hi)) = (lo.trim().parse::<i32>(), hi.trim().parse::<i32>()) {
            return uid >= lo && uid <= hi;
        }
    }
    false
}

fn parse_item(s: &str) -> Option<Item> {
    if let Some(i) = Item::from_mnemonic(s.chars().next().unwrap_or(' ')) {
        return Some(i);
    }
    let s_lc = s.to_lowercase();
    let all_items = [
        Item::Civil, Item::Milit, Item::Shell, Item::Gun, Item::Petrol,
        Item::Iron, Item::Dust, Item::Bar, Item::Food, Item::Oil,
        Item::Lcm, Item::Hcm, Item::Uw, Item::Rad,
    ];
    match s_lc.as_str() {
        "dust" | "gold dust" => return Some(Item::Dust),
        "bar" | "bars" | "gold bars" => return Some(Item::Bar),
        "lcm" | "light" => return Some(Item::Lcm),
        "hcm" | "heavy" => return Some(Item::Hcm),
        "uw" | "undesirable" | "undesirables" => return Some(Item::Uw),
        _ => {}
    }
    all_items.into_iter().find(|item| item.name().starts_with(s_lc.as_str()))
}
