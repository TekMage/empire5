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
// Ported from: src/lib/commands/info.c

// "info <topic>" command — display help text from info_dir files.
// Falls back to a short message when the directory or topic is absent.

use super::ctx::CmdCtx;

pub async fn run(topic: &str, ctx: &CmdCtx<'_>) -> String {
    let info_dir = &ctx.config.server.info_dir;

    if topic.is_empty() {
        // List available topics
        match std::fs::read_dir(info_dir) {
            Ok(entries) => {
                let mut topics: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter(|n| !n.starts_with('.'))
                    .collect();
                topics.sort();

                if topics.is_empty() {
                    return "2 No info topics installed.\n\
                            2 Place plain-text files in the info_dir directory.\n\
                            1 info\n"
                        .to_string();
                }

                let mut out = String::from("2 Available info topics:\n");
                for chunk in topics.chunks(6) {
                    out.push_str(&format!("2   {}\n", chunk.join("  ")));
                }
                out.push_str("1 info\n");
                out
            }
            Err(_) => {
                format!(
                    "2 Info directory not found: {}\n\
                     2 Set info_dir in empire.toml and populate it.\n\
                     1 info\n",
                    info_dir.display()
                )
            }
        }
    } else {
        // Sanitize topic: allow only alphanumeric, hyphen, underscore, dot
        let safe = topic
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .collect::<String>();

        if safe.is_empty() || safe.contains("..") {
            return "2 Invalid info topic name.\n1 info\n".to_string();
        }

        let path = info_dir.join(&safe);

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let mut out = String::new();
                for line in content.lines() {
                    out.push_str(&format!("2 {line}\n"));
                }
                out.push_str("1 info\n");
                out
            }
            Err(_) => {
                // Try case-insensitive match on the directory
                let matched = match_topic_insensitive(info_dir, &safe);
                match matched {
                    Some(content) => {
                        let mut out = String::new();
                        for line in content.lines() {
                            out.push_str(&format!("2 {line}\n"));
                        }
                        out.push_str("1 info\n");
                        out
                    }
                    None => format!(
                        "2 No info on '{topic}'.\n\
                         2 Type 'info' with no argument for a list of topics.\n\
                         1 info\n"
                    ),
                }
            }
        }
    }
}

fn match_topic_insensitive(info_dir: &std::path::Path, topic: &str) -> Option<String> {
    let topic_lc = topic.to_lowercase();
    let entries = std::fs::read_dir(info_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().into_string().ok()?;
        if name.to_lowercase() == topic_lc {
            return std::fs::read_to_string(entry.path()).ok();
        }
    }
    None
}
