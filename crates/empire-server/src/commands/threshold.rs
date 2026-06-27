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
// Ported from: src/lib/commands/thre.c

// "threshold" command — set distribution thresholds per commodity per sector.
// Usage: threshold <commodity> <sector-spec> [value]
//   e.g. threshold food 0,0 500   (keep 500 food in sector 0,0 before distributing)
//   e.g. threshold c *            (show current civilian threshold for all sectors)

use empire_db::sectors;
use empire_types::commodity::Item;
use empire_types::sector_chr::SectorChr;
use super::ctx::CmdCtx;

const ALL_ITEMS: [Item; 14] = [
    Item::Civil, Item::Milit, Item::Shell, Item::Gun, Item::Petrol,
    Item::Iron, Item::Dust, Item::Bar, Item::Food, Item::Oil,
    Item::Lcm, Item::Hcm, Item::Uw, Item::Rad,
];
const ITEM_MAX: i16 = i16::MAX;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(4, ' ').collect();
    if parts.is_empty() {
        return "10 Usage: threshold <commodity> <sector-spec> [value]\n".to_string();
    }

    let item = match parse_item(parts[0].trim()) {
        Some(i) => i,
        None => return format!("10 Unknown commodity: '{}'\n", parts[0]),
    };

    let area_spec = parts.get(1).copied().unwrap_or("*");
    let new_thresh: Option<i16> = parts.get(2).and_then(|s| s.trim().parse().ok());

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let item_idx = item as usize;

    for mut s in all_sectors {
        if s.own != ctx.cnum && !ctx.is_deity { continue; }
        if s.own == 0 { continue; }
        if !matches_area(&s, area_spec, ctx) { continue; }

        let xy = ctx.format_xy(s.x, s.y);
        let dchr = SectorChr::for_type(s.sector_type);
        let old = s.del[item_idx].threshold;

        if let Some(thresh) = new_thresh {
            let thresh = thresh.max(0).min(ITEM_MAX);
            if old == thresh {
                out.push_str(&format!(
                    "1 {xy} {} threshold unchanged (left at {})\n",
                    dchr.name, old
                ));
                continue;
            }
            if old > 0 {
                out.push_str(&format!(
                    "1 {xy} {} old threshold {old}\n",
                    dchr.name
                ));
            }
            s.del[item_idx].threshold = thresh;
            match sectors::put(ctx.db, &s).await {
                Ok(_) => {}
                Err(e) => {
                    out.push_str(&format!("1 {xy}: database error: {e}\n"));
                    continue;
                }
            }
        } else {
            // Display-only mode (no value given)
            let disp = if old == 0 {
                format!("{xy} {} threshold not set", dchr.name)
            } else {
                format!("{xy} {} threshold {old}", dchr.name)
            };
            out.push_str(&format!("1 {disp}\n"));
        }
    }

    if out.is_empty() {
        let spec = if area_spec.is_empty() { "*" } else { area_spec };
        out.push_str(&format!("1 {spec}: No sector(s)\n"));
    }
    out.push_str("0 threshold\n");
    out
}

fn parse_item(s: &str) -> Option<Item> {
    if s.is_empty() { return None; }
    if s.len() == 1 {
        return Item::from_mnemonic(s.chars().next()?);
    }
    let s_lc = s.to_lowercase();
    // Short aliases that don't prefix-match item.name()
    match s_lc.as_str() {
        "dust" | "gold dust"                    => return Some(Item::Dust),
        "bar"  | "bars" | "gold bars"           => return Some(Item::Bar),
        "lcm"  | "light"                        => return Some(Item::Lcm),
        "hcm"  | "heavy"                        => return Some(Item::Hcm),
        "uw"   | "undesirable" | "undesirables" => return Some(Item::Uw),
        _ => {}
    }
    for &item in &ALL_ITEMS {
        if item.name().starts_with(s_lc.as_str()) {
            return Some(item);
        }
    }
    None
}

fn matches_area(s: &empire_types::sector::Sector, spec: &str, ctx: &CmdCtx) -> bool {
    if spec.is_empty() || spec == "*" { return true; }
    if let Some((rx, ry)) = parse_rel_xy(spec) {
        return s.x == ctx.x_abs(rx) && s.y == ctx.y_abs(ry);
    }
    if let Some(pos) = spec.find(':') {
        let (coord_part, dist_part) = spec.split_at(pos);
        let dist_part = &dist_part[1..];
        if let (Some((rx, ry)), Ok(dist)) = (parse_rel_xy(coord_part), dist_part.trim().parse::<i32>()) {
            let ax = ctx.x_abs(rx);
            let ay = ctx.y_abs(ry);
            let d = crate::subs::geo::map_dist(s.x, s.y, ax, ay, ctx.world_x, ctx.world_y);
            return d <= dist;
        }
    }
    true
}

fn parse_rel_xy(s: &str) -> Option<(i16, i16)> {
    let (xs, ys) = s.split_once(',')?;
    Some((xs.trim().parse().ok()?, ys.trim().parse().ok()?))
}
