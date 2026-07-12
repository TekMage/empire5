// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/expl.c

// "explore" command — move civilians into adjacent wilderness to claim it.
//
// Usage: explore <x,y> <path> <commodity> <amount>
//   e.g. explore 0,0 j civ 10
//        explore 0,0 nnb food 50
//
// The player specifies a starting sector they own, a direction path (u=UR,
// j=R, n=DR, b=DL, g=L, y=UL, h=stop), a commodity type (civ, food, mil,
// etc.) and an amount.  The commodity is moved step by step along the path.
// Any unowned wilderness or plain sector touched becomes the player's.
// Sea, mountain, and occupied enemy sectors cannot be entered.
//
// Mobility cost: each step costs the destination sector's terrain movement
// cost (SectorChr::mcost — scaled by the destination's efficiency, so
// highways/bridges at high efficiency are cheap or free to enter).  The walk
// stops as soon as the source sector can't afford the next step, mirroring
// 4.4.1's move_ground() — mobility never goes negative.
// The moved commodity arrives in the final (destination) sector.

use empire_db::sectors;
use empire_types::commodity::Item;
use empire_types::sector::SectorType;
use empire_types::sector_chr::SectorChr;
use crate::subs::geo;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    // Parse: <x,y> <path> <comm> <amount>
    let parts: Vec<&str> = args.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return "10 Usage: explore <x,y> <path> <comm> <amount>\n".to_string();
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

    // Load starting sector — must be owned by this player
    let mut src = match sectors::get_at(ctx.db, abs_x, abs_y).await {
        Ok(Some(s)) => s,
        Ok(None)    => return format!("10 Sector {rel_x},{rel_y} does not exist\n"),
        Err(e)      => return format!("10 DB error: {e}\n"),
    };
    if src.own != ctx.cnum {
        return format!("10 You don't own {rel_x},{rel_y}\n");
    }

    // Check source has enough of the commodity
    let have = src.items.get(item);
    if have < amount {
        return format!(
            "10 {rel_x},{rel_y} has only {have} {}, need {amount}\n",
            item.name()
        );
    }

    if src.mobil <= 0 {
        return format!("10 No mobility in {rel_x},{rel_y}\n");
    }

    // Parse path string and walk it
    let mut out = String::new();
    let mut cur_x = abs_x;
    let mut cur_y = abs_y;
    let mut steps = 0i8;
    let mut mobility_left = src.mobil as f64;

    for ch in path_str.chars() {
        if ch == 'h' { break; } // stop
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
                out.push_str(&format!("1 {dest_rel} is ocean — can't explore there\n"));
                break;
            }
            SectorType::Mountain => {
                out.push_str(&format!("1 {dest_rel} is a mountain — can't explore there\n"));
                break;
            }
            _ => {}
        }

        // Enemy territory?
        if dest.own != 0 && dest.own != ctx.cnum {
            out.push_str(&format!(
                "1 {dest_rel} is owned by country {} — can't explore enemy territory\n",
                dest.own
            ));
            break;
        }

        // Mobility check: cost is the destination's terrain movement cost at
        // its current efficiency (SectorChr::mcost). Stop before going
        // negative, same as 4.4.1's move_ground().
        let step_cost = SectorChr::for_type(dest.sector_type).mcost(dest.effic);
        if step_cost < 0.0 {
            out.push_str(&format!("1 {dest_rel} is impassable — can't explore there\n"));
            break;
        }
        if step_cost > mobility_left {
            out.push_str(&format!(
                "1 Not enough mobility to reach {dest_rel} — stopped at {}\n",
                ctx.format_xy(cur_x, cur_y)
            ));
            break;
        }

        // Claim unowned sector
        if dest.own == 0 {
            let mut claimed = dest.clone();
            claimed.own = ctx.cnum;
            claimed.old_own = ctx.cnum;
            if let Err(e) = sectors::put(ctx.db, &claimed).await {
                out.push_str(&format!("1 DB error claiming {dest_rel}: {e}\n"));
                break;
            }
            out.push_str(&format!("1 {dest_rel} claimed\n"));
        }

        mobility_left -= step_cost;
        cur_x = nx;
        cur_y = ny;
        steps += 1;
    }

    if steps == 0 {
        out.push_str("1 No sectors explored.\n");
        out.push_str("0 explore\n");
        return out;
    }

    // Deduct commodity from source, deposit in destination
    let final_rel = ctx.format_xy(cur_x, cur_y);
    src.items.add(item, -amount);
    src.mobil = mobility_left.round().clamp(0.0, 127.0) as i8;
    if let Err(e) = sectors::put(ctx.db, &src).await {
        out.push_str(&format!("10 DB error updating source: {e}\n"));
        out.push_str("0 explore\n");
        return out;
    }

    // Load the final destination (may have been just claimed) and add commodity
    match sectors::get_at(ctx.db, cur_x, cur_y).await {
        Ok(Some(mut dest)) => {
            dest.items.add(item, amount);
            if let Err(e) = sectors::put(ctx.db, &dest).await {
                out.push_str(&format!("10 DB error depositing at {final_rel}: {e}\n"));
            } else {
                out.push_str(&format!(
                    "1 {} {} moved to {}\n",
                    amount, item.name(), final_rel
                ));
            }
        }
        Ok(None) => out.push_str(&format!("10 Destination {final_rel} vanished\n")),
        Err(e)   => out.push_str(&format!("10 DB error at destination: {e}\n")),
    }

    out.push_str("0 explore\n");
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
