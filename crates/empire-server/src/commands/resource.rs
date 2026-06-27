// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/reso.c

// "resource" command — show natural resource values for owned sectors.
//
// Usage: resource <sector-spec>
//   e.g. resource *        (all owned sectors)
//        resource 0,0:3    (sectors within distance 3 of origin)

use empire_db::sectors;
use super::ctx::CmdCtx;
use super::sector_sel::matches_area;

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
            if ctx.is_deity { out.push_str("1 "); } else { out.push_str("1 "); }
            out.push_str("RESOURCE\n");
            out.push_str("1 ");
            if ctx.is_deity { out.push_str("own "); }
            out.push_str("  sect        eff  min gold fert oil uran");
            if !ctx.is_deity { out.push_str(" ter"); }
            out.push('\n');
        }
        nsect += 1;

        let xy = ctx.format_xy(s.x, s.y);
        let type_ch = s.sector_type.mnemonic();
        let new_ch = if s.new_type != s.sector_type {
            s.new_type.mnemonic()
        } else {
            ' '
        };

        if ctx.is_deity {
            out.push_str(&format!("1  {:3} ", s.own));
        } else {
            out.push_str("1 ");
        }

        out.push_str(&format!(
            "{:>4},{:<4} {}{}{:4}%{:5}{:5}{:5}{:4}{:5}",
            // prxy format: x right-4, y left-4
            ctx.x_rel(s.x), ctx.y_rel(s.y),
            type_ch, new_ch,
            s.effic,
            s.min,
            s.gmin,
            s.fertil,
            s.oil,
            s.uran,
        ));
        if !ctx.is_deity {
            out.push_str(&format!("{:4}", s.terr[0]));
        }
        out.push('\n');
    }

    if nsect == 0 {
        let label = if spec.is_empty() { "*" } else { spec };
        out.push_str(&format!("1 {label}: No sector(s)\n"));
    } else {
        out.push_str(&format!("1 {} sector{}\n", nsect, if nsect == 1 { "" } else { "s" }));
    }
    out.push_str("0 resource\n");
    out
}
