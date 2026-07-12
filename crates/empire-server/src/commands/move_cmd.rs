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
// Accepts two formats:
//
//   Classic ptkei format:  move <comm> <x,y> <amount> <x,y>
//     e.g.  move civ 9,-1 95 8,-2
//     The server auto-finds the shortest land path (BFS) and applies the move.
//
//   Path format:  move <x,y> <path> <comm> <amount>
//     e.g.  move 0,0 j food 100
//     The server walks the explicit path step by step.
//
// Unlike explore, move only succeeds if EVERY sector along the path is
// already owned by the player.  It will not claim wilderness.
// Mobility cost: 1 mob per step, deducted from the source sector.

use std::collections::HashMap;
use empire_db::sectors;
use empire_types::commodity::Item;
use empire_types::sector::SectorType;
use empire_types::sector_chr::SectorChr;
use empire_types::coords::Coord;
use crate::subs::geo::{self, DIRCH};
use crate::subs::pathfind;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    // Detect format by checking whether the first token contains a comma.
    // Classic format:  first token is a commodity name (no comma).
    // Path format:     first token is a coordinate like "9,-1" (has comma).
    let first = args.splitn(2, ' ').next().unwrap_or("");
    if !first.contains(',') && !first.is_empty() {
        return run_classic(args, ctx).await;
    }
    run_path(args, ctx).await
}

// Classic ptkei format: move <comm> <from-x,y> <amount> <to-x,y>
async fn run_classic(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return "10 Usage: move <comm> <x,y> <amount> <x,y>\n".to_string();
    }
    let comm_str = parts[0];
    let from_str = parts[1];
    let amt_str  = parts[2];
    let to_str   = parts[3].trim();

    let item = match item_from_str(comm_str) {
        Some(i) => i,
        None    => return format!("10 Unknown commodity: {comm_str}\n"),
    };
    let (from_rx, from_ry) = match parse_xy(from_str) {
        Some(v) => v,
        None    => return format!("10 Bad sector: {from_str}\n"),
    };
    let amount: i16 = match amt_str.parse() {
        Ok(n) if n > 0 => n,
        _ => return format!("10 Bad amount: {amt_str}\n"),
    };
    let (to_rx, to_ry) = match parse_xy(to_str) {
        Some(v) => v,
        None    => return format!("10 Bad sector: {to_str}\n"),
    };

    let from_ax = ctx.x_abs(from_rx);
    let from_ay = ctx.y_abs(from_ry);
    let to_ax   = ctx.x_abs(to_rx);
    let to_ay   = ctx.y_abs(to_ry);

    // Same sector — trivial no-op
    if from_ax == to_ax && from_ay == to_ay {
        let mut out = String::new();
        out.push_str("1 Nothing moved.\n");
        out.push_str("0 move\n");
        return out;
    }

    // Load all sectors to build an owned-land lookup for BFS
    let all = match sectors::get_all(ctx.db).await {
        Ok(v)  => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };
    let sec_map: HashMap<(Coord, Coord), _> = all.iter()
        .map(|s| ((s.x, s.y), s))
        .collect();

    let cnum = ctx.cnum;
    let path_dirs = pathfind::find_path(
        from_ax, from_ay, to_ax, to_ay,
        ctx.world_x, ctx.world_y,
        |nx, ny| {
            sec_map.get(&(nx, ny)).map_or(false, |s| {
                s.own == cnum
                    && s.sector_type != SectorType::Sea
                    && s.sector_type != SectorType::Mountain
            })
        },
    );

    if path_dirs.is_empty() {
        return format!(
            "10 No path from {from_rx},{from_ry} to {to_rx},{to_ry}\n"
        );
    }

    // Convert direction indices to a path string and delegate to path executor
    let path_str: String = path_dirs.iter().map(|&d| DIRCH[d as usize]).collect();
    execute_move(from_ax, from_ay, &path_str, item, amount, ctx).await
}

// Path format: move <x,y> <path> <comm> <amount>
async fn run_path(args: &str, ctx: &CmdCtx<'_>) -> String {
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
        None    => return format!("10 Bad sector: {xy_str}\n"),
    };
    let item = match item_from_str(comm_str) {
        Some(i) => i,
        None    => return format!("10 Unknown commodity: {comm_str}\n"),
    };
    let amount: i16 = match amt_str.parse() {
        Ok(n) if n > 0 => n,
        _ => return format!("10 Bad amount: {amt_str}\n"),
    };

    execute_move(ctx.x_abs(rel_x), ctx.y_abs(rel_y), path_str, item, amount, ctx).await
}

// Shared move executor: walks <path_str> from (src_ax, src_ay), moves <amount> of <item>.
async fn execute_move(
    src_ax: Coord,
    src_ay: Coord,
    path_str: &str,
    item: Item,
    amount: i16,
    ctx: &CmdCtx<'_>,
) -> String {
    let rel_src = ctx.format_xy(src_ax, src_ay);

    let mut src = match sectors::get_at(ctx.db, src_ax, src_ay).await {
        Ok(Some(s)) => s,
        Ok(None)    => return format!("10 Sector {rel_src} does not exist\n"),
        Err(e)      => return format!("10 DB error: {e}\n"),
    };
    if src.own != ctx.cnum {
        return format!("10 You don't own {rel_src}\n");
    }
    let have = src.items.get(item);
    if have < amount {
        return format!(
            "10 {rel_src} has only {have} {}, need {amount}\n",
            item.name()
        );
    }

    if src.mobil <= 0 {
        return format!("10 No mobility in {rel_src}\n");
    }

    let mut out = String::new();
    let mut cur_x = src_ax;
    let mut cur_y = src_ay;
    let mut steps = 0i8;
    let mut mobility_left = src.mobil as f64;

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

        // Mobility check: destination's terrain movement cost at its
        // current efficiency (highways/bridges are cheap-to-free at high
        // eff). Stop before going negative, same as 4.4.1's move_ground().
        let step_cost = SectorChr::for_type(dest.sector_type).mcost(dest.effic);
        if step_cost < 0.0 {
            out.push_str(&format!("1 {dest_rel} is impassable\n"));
            break;
        }
        if step_cost > mobility_left {
            out.push_str(&format!(
                "1 Not enough mobility to reach {dest_rel} — stopped at {}\n",
                ctx.format_xy(cur_x, cur_y)
            ));
            break;
        }

        mobility_left -= step_cost;
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
    src.mobil = mobility_left.round().clamp(0.0, 127.0) as i8;
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
