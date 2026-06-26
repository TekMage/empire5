// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/move.c

// "move" command — move commodities between owned sectors.
//
// Usage: move <x,y> <path> <commodity> <amount>
//   e.g. move 0,0 j food 100
//        move 2,0 ggn iron 50
//
// Unlike explore, move only succeeds if EVERY sector along the path is
// already owned by the player.  It will not claim wilderness.
// Mobility cost: 1 mob per step, deducted from the source sector.
//
// TODO: warehouse discount — when a step passes through (or terminates at)
// a SectorType::Warehouse at >= 60% efficiency, apply ~1/3 normal mobility
// cost for that leg.  Same discount should apply in the distribution update
// tick (update.rs) when routing through a warehouse node.

use empire_db::sectors;
use empire_types::commodity::Item;
use empire_types::sector::SectorType;
use crate::subs::geo;
use super::ctx::CmdCtx;

// move and explore share identical logic; move just refuses to claim new sectors.
// We delegate to a shared helper rather than duplicating parse code.
pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return "10 Usage: move <x,y> <path> <comm> <amount>\n".to_string();
    }
    let xy_str   = parts[0];
    let path_str = parts[1];
    let comm_str = parts[2].trim();
    let amt_str  = parts[3].trim();

    let (rel_x, rel_y) = match parse_xy(xy_str) {
        Some(v) => v,
        None => return format!("10 Bad sector: {xy_str}\n"),
    };
    let abs_x = ctx.x_abs(rel_x);
    let abs_y = ctx.y_abs(rel_y);

    let item = match item_from_str(comm_str) {
        Some(i) => i,
        None => return format!("10 Unknown commodity: {comm_str}\n"),
    };

    let amount: i16 = match amt_str.parse() {
        Ok(n) if n > 0 => n,
        _ => return format!("10 Bad amount: {amt_str}\n"),
    };

    let mut src = match sectors::get_at(ctx.db, abs_x, abs_y).await {
        Ok(Some(s)) => s,
        Ok(None)    => return format!("10 Sector {rel_x},{rel_y} does not exist\n"),
        Err(e)      => return format!("10 DB error: {e}\n"),
    };
    if src.own != ctx.cnum {
        return format!("10 You don't own {rel_x},{rel_y}\n");
    }
    let have = src.items.get(item);
    if have < amount {
        return format!(
            "10 {rel_x},{rel_y} has only {have} {}, need {amount}\n",
            item.name()
        );
    }

    let mut out = String::new();
    let mut cur_x = abs_x;
    let mut cur_y = abs_y;
    let mut steps = 0i8;

    for ch in path_str.chars() {
        if ch == 'h' { break; }
        let dir = match geo::dir_from_char(ch) {
            Some(d) if d != geo::DIR_STOP => d,
            _ => {
                out.push_str(&format!("1 Unknown direction '{}'\n", ch));
                break;
            }
        };
        let (dx, dy) = geo::DIROFF[dir];
        let nx = geo::x_norm(cur_x + dx, ctx.world_x);
        let ny = geo::y_norm(cur_y + dy, ctx.world_y);

        let dest = match sectors::get_at(ctx.db, nx, ny).await {
            Ok(Some(s)) => s,
            Ok(None)    => { out.push_str("1 Off the world\n"); break; }
            Err(e)      => { out.push_str(&format!("10 DB error: {e}\n")); break; }
        };
        let dest_rel = ctx.format_xy(dest.x, dest.y);

        match dest.sector_type {
            SectorType::Sea => {
                out.push_str(&format!("1 {dest_rel} is ocean\n"));
                break;
            }
            SectorType::Mountain => {
                out.push_str(&format!("1 {dest_rel} is a mountain\n"));
                break;
            }
            _ => {}
        }
        if dest.own != ctx.cnum {
            out.push_str(&format!("1 {dest_rel} is not your territory\n"));
            break;
        }
        cur_x = nx;
        cur_y = ny;
        steps += 1;
    }

    if steps == 0 {
        out.push_str("1 Nothing moved.\n");
        out.push_str("0 move\n");
        return out;
    }

    let final_rel = ctx.format_xy(cur_x, cur_y);
    src.items.add(item, -amount);
    src.mobil = src.mobil.saturating_sub(steps);
    if let Err(e) = sectors::put(ctx.db, &src).await {
        out.push_str(&format!("10 DB error: {e}\n"));
        out.push_str("0 move\n");
        return out;
    }

    match sectors::get_at(ctx.db, cur_x, cur_y).await {
        Ok(Some(mut dest)) => {
            dest.items.add(item, amount);
            if let Err(e) = sectors::put(ctx.db, &dest).await {
                out.push_str(&format!("10 DB error at destination: {e}\n"));
            } else {
                out.push_str(&format!(
                    "1 {} {} moved to {}\n",
                    amount, item.name(), final_rel
                ));
            }
        }
        Ok(None) => out.push_str(&format!("10 Destination {final_rel} vanished\n")),
        Err(e)   => out.push_str(&format!("10 DB error: {e}\n")),
    }

    out.push_str("0 move\n");
    out
}

fn parse_xy(s: &str) -> Option<(i16, i16)> {
    let (xs, ys) = s.split_once(',')?;
    Some((xs.trim().parse().ok()?, ys.trim().parse().ok()?))
}

fn item_from_str(s: &str) -> Option<Item> {
    match s {
        "civ" | "civil" | "civilians" => Some(Item::Civil),
        "mil" | "milit" | "military"  => Some(Item::Milit),
        "food"                        => Some(Item::Food),
        "gun" | "guns"                => Some(Item::Gun),
        "shell" | "shells"            => Some(Item::Shell),
        "petrol" | "pet"              => Some(Item::Petrol),
        "iron"                        => Some(Item::Iron),
        "dust"                        => Some(Item::Dust),
        "bar" | "bars"                => Some(Item::Bar),
        "oil"                         => Some(Item::Oil),
        "lcm"                         => Some(Item::Lcm),
        "hcm"                         => Some(Item::Hcm),
        "uw"                          => Some(Item::Uw),
        "rad"                         => Some(Item::Rad),
        s if s.len() == 1 => Item::from_mnemonic(s.chars().next().unwrap()),
        _ => None,
    }
}
