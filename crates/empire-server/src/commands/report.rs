// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/repo.c
//   Known contributors to the original: Keith Muller (1983), Dave Pare (1986),
//   Steve McClure (2000)

// "report" command — show tech/research/education/happiness levels of nations.
//
// Usage: report
//        report *
//        report <n>
//
// Non-deities see fuzzy ranges rather than exact values for other nations.
// Own nation always shows exact values.  Deities see exact values + capital.
//
// Fuzzy range logic: divide levels into brackets relative to the viewer's own
// level (same algorithm as the C server's printdiff / tryprdiff).

use empire_db::nations;
use empire_types::nation::{Nation, NatStatus};
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let arg = args.trim();

    // Determine which nations to show.
    let all_nations = match nations::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let targets: Vec<Nation> = if arg.is_empty() || arg == "*" {
        all_nations.into_iter().filter(|n| {
            if n.status == NatStatus::Unused { return false; }
            if ctx.is_deity { true } else { n.status >= NatStatus::Active }
        }).collect()
    } else if let Ok(cnum) = arg.parse::<u8>() {
        all_nations.into_iter().filter(|n| n.cnum == cnum).collect()
    } else {
        return "10 Usage: report [*|<nation>]\n".to_string();
    };

    let mut out = String::new();
    out.push_str(&repo_header(ctx.is_deity));
    for nat in &targets {
        out.push_str(&repo_line(ctx, nat));
    }
    out.push_str("0 report\n");
    out
}

fn repo_header(is_deity: bool) -> String {
    let last_col = if is_deity { "capital  " } else { " status" };
    format!(
        "1  #    name                tech      research   education   happiness  {last_col}\n"
    )
}

fn repo_line(ctx: &CmdCtx, nat: &Nation) -> String {
    let prefix = format!("1  {:<3}   {:<14} ", nat.cnum, &nat.name[..nat.name.len().min(14)]);

    let levels = if ctx.is_deity || ctx.cnum == nat.cnum {
        // Exact values
        let suffix = if ctx.is_deity {
            let xcap = ctx.x_rel(nat.xcap);
            let ycap = ctx.y_rel(nat.ycap);
            format!("  {xcap:>4},{ycap:<4}")
        } else {
            "    ".to_string()
        };
        format!(
            " {:>7.2}     {:>7.2}     {:>7.2}     {:>7.2}{suffix}",
            nat.tech, nat.research, nat.education, nat.happiness
        )
    } else {
        // Fuzzy ranges — same bracket algorithm as C printdiff/tryprdiff.
        let us = &ctx.nat;
        let show = us.status >= NatStatus::Active && nat.status >= NatStatus::Active;
        format!(
            "{}{}{}{}",
            if show { fmt_level(us.tech,       nat.tech,       20) } else { "    n/a     ".to_string() },
            if show { fmt_level(us.research,    nat.research,   10) } else { "    n/a     ".to_string() },
            if show { fmt_level(us.education,   nat.education,   5) } else { "    n/a     ".to_string() },
            if show { fmt_level(us.happiness,   nat.happiness,   5) } else { "    n/a     ".to_string() },
        )
    };

    let status = if !ctx.is_deity {
        match nat.status {
            NatStatus::Active  => if nat.money < 0 { "Broke" } else { "Active" },
            NatStatus::Sanct   => "Sanctuary",
            NatStatus::Visitor => "Visitor",
            NatStatus::Deity   => "Deity",
            _                  => "Unknown",
        }
    } else {
        ""
    };

    format!("{prefix}{levels}{status}\n")
}

// Returns a 12-char fuzzy range string for one level, relative to `ours`.
// Matches the C tryprdiff / printdiff bracket algorithm exactly.
fn fmt_level(ours: f64, theirs: f64, tol: i32) -> String {
    if ours == 0.0 {
        return "    n/a     ".to_string();
    }

    // Subtract a common base so numbers stay small.
    let shift = ((theirs.min(ours)) as i32 - 100).max(0);
    let o = ours  - shift as f64;
    let t = theirs - shift as f64;
    let tol = tol.min((2.0 * o) as i32);

    // Brackets from highest to lowest, matching the C if-chain order.
    // lo < 0 means "≥ lo (lower unbounded)"; hi < 0 means "≤ hi (upper unbounded)".
    let brackets: &[(f64, f64)] = &[
        (2.0 * o,  -1.0     ),  // theirs >= 2×ours
        (1.5 * o,  2.0 * o  ),
        (1.2 * o,  1.5 * o  ),
        (1.1 * o,  1.2 * o  ),
        (o / 1.1,  1.1 * o  ),
        (o / 1.2,  o / 1.1  ),
        (o / 1.5,  o / 1.2  ),
        (o / 2.0,  o / 1.5  ),
        (-1.0,     o / 2.0  ),  // theirs <= ours/2
    ];

    for &(lo, hi) in brackets {
        if let Some(s) = try_bracket(t, lo, hi, shift, tol) {
            return s;
        }
    }
    "    n/a     ".to_string()
}

// Try one bracket.  Returns Some(12-char string) if theirs falls in range.
fn try_bracket(theirs: f64, lo: f64, hi: f64, shift: i32, tol: i32) -> Option<String> {
    if lo < 0.0 {
        // Upper-bounded: print "0 - hi"
        if theirs <= hi {
            let max = (hi as i32).max(tol);
            return Some(format!("   0 - {:<4} ", max + shift));
        }
    } else if hi < 0.0 {
        // Lower-bounded: print ">= lo"
        if theirs >= lo {
            return Some(format!("    >= {:<4} ", lo as i32 + shift));
        }
    } else if theirs >= lo && theirs <= hi {
        let (mut rlo, mut rhi) = (lo, hi);
        if rhi - rlo < tol as f64 {
            let shove = (tol as f64 - (rhi - rlo)) / 2.0;
            if rlo + shift as f64 - shove >= 0.0 {
                rlo -= shove;
                rhi += shove;
            } else {
                rlo = 0.0;
                rhi = tol as f64;
            }
        }
        return Some(format!("{:>4} - {:<4} ", rlo as i32 + shift, rhi as i32 + shift));
    }
    None
}
