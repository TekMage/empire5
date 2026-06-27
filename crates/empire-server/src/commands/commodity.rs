// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/comm.c

// "commodity" command — show deliver directions, distribution thresholds, and
// current inventory for the 10 deliverable commodities (shell through rad).
//
// Usage: commodity <sector-spec>
//   e.g. commodity *       (all owned sectors)

use empire_db::sectors;
use empire_types::commodity::Item;
use super::ctx::CmdCtx;
use super::sector_sel::matches_area;

// The 10 commodities shown in the commodity report (matching C comm.c order).
// Civilians, military, food, and UW are omitted — they are not deliverable.
const COMM_ITEMS: [Item; 10] = [
    Item::Shell, Item::Gun, Item::Petrol, Item::Iron, Item::Dust,
    Item::Bar, Item::Oil, Item::Lcm, Item::Hcm, Item::Rad,
];

// Encode delivery direction as the classic one-char mnemonic.
fn dir_char(d: u8) -> char {
    match d {
        0 => '.', 1 => 'u', 2 => 'j', 3 => 'n',
        4 => 'b', 5 => 'g', 6 => 'y', 7 => '$',
        _ => '?',
    }
}

// Encode distribution threshold as a single char: '.' = 0, '0'–'9' = 0–900, 'a' = 1000+.
fn thresh_char(val: i16) -> char {
    if val >= 1000 {
        'a'
    } else if val > 0 {
        char::from_digit((val / 100) as u32, 10).unwrap_or('?')
    } else {
        '.'
    }
}

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let spec = args.trim();

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let mut out = String::new();
    let mut nsect = 0u32;

    for s in &all_sectors {
        if s.own != ctx.cnum && !ctx.is_deity { continue; }
        if s.own == 0 { continue; }
        if !spec.is_empty() && !matches_area(s, spec, ctx) { continue; }

        if nsect == 0 {
            let pfx = if ctx.is_deity { "1    " } else { "1 " };
            out.push_str(&format!("{pfx}COMMODITIES deliver--  distribute\n"));
            let pfx2 = if ctx.is_deity { "1    " } else { "1 " };
            out.push_str(&format!(
                "{pfx2}  sect      sgpidbolhr sgpidbolhr  sh gun  pet iron dust bar  oil  lcm  hcm rad\n"
            ));
        }
        nsect += 1;

        if ctx.is_deity {
            out.push_str(&format!("1 {:3}", s.own));
        } else {
            out.push_str("1 ");
        }

        let type_ch = s.sector_type.mnemonic();
        out.push_str(&format!("{:>4},{:<4} {} ", ctx.x_rel(s.x), ctx.y_rel(s.y), type_ch));

        // Deliver direction column (10 chars, one per commodity)
        for &item in &COMM_ITEMS {
            out.push(dir_char(s.del[item as usize].path));
        }
        out.push(' ');

        // Distribution threshold column (10 chars, one per commodity)
        for &item in &COMM_ITEMS {
            out.push(thresh_char(s.del[item as usize].threshold));
        }
        out.push(' ');

        // Inventory amounts — widths match C: sh=4, gun=4, pet=5, iron=5, dust=5,
        //                                     bar=4, oil=5, lcm=5, hcm=5, rad=4
        let widths = [4usize, 4, 5, 5, 5, 4, 5, 5, 5, 4];
        for (&item, &w) in COMM_ITEMS.iter().zip(widths.iter()) {
            out.push_str(&format!("{:>w$}", s.items.get(item), w = w));
        }
        out.push('\n');
    }

    if nsect == 0 {
        let label = if spec.is_empty() { "*" } else { spec };
        out.push_str(&format!("1 {label}: No sector(s)\n"));
    } else {
        out.push_str(&format!("1 {} sector{}\n", nsect, if nsect == 1 { "" } else { "s" }));
    }
    out.push_str("0 commodity\n");
    out
}
