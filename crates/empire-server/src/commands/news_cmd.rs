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
// Ported from: src/lib/commands/news.c, src/lib/commands/head.c

// "news" command — show empire news since last visit (or last N days).
//
// Usage: news [days]
//   news       — show news since you last read it
//   news 3     — show news from the last 3 days
//
// News items are organized into sections (Foreign Affairs, Front Line, etc.).
// After the details, a "Bottom Line" shows net sectors captured between nations.

use empire_db::{nations, news};
use empire_types::news::{NewsVerb, NewsPage};
use super::ctx::CmdCtx;
use chrono::{Local, TimeZone};

/// All news pages in display order.
const PAGES: &[NewsPage] = &[
    NewsPage::Foreign,
    NewsPage::FrontLine,
    NewsPage::Sea,
    NewsPage::Sky,
    NewsPage::Telecom,
];

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Determine since_ts: from arg (days) or from nat.news_time
    let since_ts = if let Ok(days) = args.trim().parse::<i64>() {
        (now - days * 86400).max(0)
    } else {
        ctx.nat.news_time
    };

    // Update news_time to now so next call shows only new events
    let mut nat = match nations::get_by_cnum(ctx.db, ctx.cnum).await {
        Ok(Some(n)) => n,
        _ => return "10 Internal error: nation not found\n".to_string(),
    };
    nat.news_time = now;
    let _ = nations::put(ctx.db, &nat).await;

    // Fetch news items
    let items = match news::get_since(ctx.db, since_ts).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    // Load nation names for display
    let all_nations = match nations::get_all(ctx.db).await {
        Ok(v) => v,
        Err(_) => vec![],
    };
    let cname = |cnum: u8| -> String {
        all_nations.iter()
            .find(|n| n.cnum == cnum)
            .map(|n| n.name.clone())
            .unwrap_or_else(|| format!("country #{cnum}"))
    };

    let mut out = String::new();

    // Banner
    let now_dt = Local.timestamp_opt(now, 0).single();
    let now_str = now_dt.map(|dt| dt.format("%a %b %e %T %Y").to_string())
        .unwrap_or_else(|| "unknown time".to_string());
    let since_dt = Local.timestamp_opt(since_ts, 0).single();
    let since_str = since_dt.map(|dt| dt.format("%a %b %e %T %Y").to_string())
        .unwrap_or_else(|| "the beginning".to_string());

    out.push_str("1 \n");
    out.push_str("1         -=[  EMPIRE NEWS  ]=-\n");
    out.push_str("1 ::::::::::::::::::::::::::::::::::::::::::::::::::\n");
    out.push_str("1 !       \"All the news that fits, we print.\"      !\n");
    out.push_str("1 ::::::::::::::::::::::::::::::::::::::::::::::::::\n");
    out.push_str(&format!("1        {now_str}\n"));
    out.push_str("1 \n");
    out.push_str(&format!("1 The details of Empire news since {since_str}\n"));

    if items.is_empty() {
        out.push_str("1 \n");
        out.push_str("1 No news at the moment...\n");
        out.push_str("0 news\n");
        return out;
    }

    // Group by page, print each section that has items
    let mut printed_any = false;
    for &page in PAGES {
        let page_items: Vec<_> = items.iter()
            .filter(|item| {
                NewsVerb::from_u8(item.verb as u8)
                    .map(|v| v.page() == page)
                    .unwrap_or(false)
            })
            .collect();

        if page_items.is_empty() { continue; }

        out.push_str("1 \n");
        out.push_str(&format!("1 \t ===  {}  ===\n", page.heading()));
        printed_any = true;

        for item in &page_items {
            let actor_name = cname(item.actor as u8);
            let victim_name = cname(item.victim as u8);
            let ts_str = format_ts(item.when_ts);

            let story = if let Some(verb) = NewsVerb::from_u8(item.verb as u8) {
                let (a, b) = verb.stories();
                // Alternate between the two stories based on actor cnum
                let tmpl = if item.actor % 2 == 0 { a } else { b };
                tmpl.replace("%s", &victim_name)
            } else {
                format!("does something (verb {})", item.verb)
            };

            let times_str = if item.times > 1 {
                format!(" {} times", item.times)
            } else {
                String::new()
            };

            let line = format!("{ts_str}  {actor_name} {story}{times_str}");
            // Wrap at 80 chars like 4.4.1 preport()
            if line.len() > 80 {
                if let Some(pos) = line[60..80].rfind(' ') {
                    let split = 60 + pos;
                    out.push_str(&format!("1 {}\n", &line[..split]));
                    out.push_str(&format!("1 \t\t  {}\n", &line[split+1..]));
                    continue;
                }
            }
            out.push_str(&format!("1 {line}\n"));
        }
    }

    if !printed_any {
        out.push_str("1 \n");
        out.push_str("1 No news at the moment...\n");
        out.push_str("0 news\n");
        return out;
    }

    // Bottom Line: net sectors captured between each pair of nations
    let mut sectors_taken = [[0i32; 256]; 256];
    for item in &items {
        if let Some(verb) = NewsVerb::from_u8(item.verb as u8) {
            if verb.captures_sector() {
                let a = item.actor as usize;
                let v = item.victim as usize;
                sectors_taken[a][v] += item.times as i32;
            }
        }
    }

    // Collect pairs where net is non-zero
    let mut bottom_line: Vec<(u8, u8, i32)> = vec![];
    for i in 0..255usize {
        for j in 0..i {
            let diff = sectors_taken[i][j] - sectors_taken[j][i];
            if diff > 0 {
                bottom_line.push((i as u8, j as u8, diff));
            } else if diff < 0 {
                bottom_line.push((j as u8, i as u8, -diff));
            }
        }
    }
    bottom_line.sort_by(|a, b| b.2.cmp(&a.2));

    if !bottom_line.is_empty() {
        out.push_str("1 \n");
        out.push_str("1 \t ===  The Bottom Line  ===\n");
        for (actor, victim, count) in &bottom_line {
            let verb = match count {
                1    => "stole",
                2..=3 => "took",
                4..=7 => "captured",
                _    => "seized",
            };
            let plural = if *count == 1 { "" } else { "s" };
            out.push_str(&format!(
                "1 {} {} {} sector{} from {}\n",
                cname(*actor), verb, count, plural, cname(*victim)
            ));
        }
    }

    out.push_str("0 news\n");
    out
}

fn format_ts(ts: i64) -> String {
    Local.timestamp_opt(ts, 0).single()
        .map(|dt| dt.format("%b %e %H:%M").to_string())
        .unwrap_or_else(|| "???".to_string())
}
