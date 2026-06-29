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

// Command dispatch table.
// Replaces src/lib/commands/ (151 .c files) and the C cmndstr dispatch table.
//
// Phase 7: add, capital, newcap, enable, disable, shutdown, distribute,
//           deliver, show, power commands.

pub mod ctx;
mod version;
mod info;
mod xdump;
mod dump;
mod census;
mod nation_cmd;
mod map_cmd;
mod designate;
mod threshold;
mod relations_cmd;
mod declare;
mod sector_sel;
mod add;
mod capital;
mod enable;
mod shutdown_cmd;
mod distribute;
mod deliver;
mod show;
mod power;
mod build;
mod march;
mod navigate;
mod attack;
mod bomb;
mod fly;
mod launch;
mod mission;
mod sell;
mod buy;
mod trade;
mod loan;
mod explore;
mod move_cmd;
mod realm;
mod report;
mod resource;
mod commodity;
mod change;
mod force;
mod update_cmd;
mod telegram_cmd;
mod read_cmd;
mod announce_cmd;
mod ship_cmd;
mod load_cmd;
mod enlist_cmd;
mod demobilize_cmd;
mod assault_cmd;
mod execute_cmd;
mod news_cmd;

use crate::state::GameState;
use crate::protocol::{code, response};
use empire_config::Config;

/// Dispatch a command line to the appropriate handler.
/// Loads the issuing nation from DB and builds a `CmdCtx` once per command.
pub async fn dispatch(line: &str, cnum: u8, state: &GameState, cfg: &Config) -> String {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let cmd  = parts[0].to_lowercase();
    let args = parts.get(1).copied().unwrap_or("");

    // Load nation record — required for coordinate transforms and identity checks
    let nat = match empire_db::nations::get_by_cnum(&state.db, cnum).await {
        Ok(Some(n)) => n,
        Ok(None)    => return response(code::CMDERR, "Internal error: nation not found"),
        Err(e)      => return response(code::CMDERR, &format!("DB error: {e}")),
    };

    let is_deity = nat.is_deity();

    let ctx = ctx::CmdCtx {
        cnum,
        nat,
        is_deity,
        db: &state.db,
        state,
        config: cfg,
        world_x: cfg.game.world_x,
        world_y: cfg.game.world_y,
        etu: cfg.game.etu_per_update,
    };

    match cmd.as_str() {
        "version" | "vers"          => version::run(args, &ctx).await,
        "info"                      => info::run(args, &ctx).await,
        "echo"                      => echo_cmd(args),
        "xdump"                     => xdump::run(args, &ctx).await,
        "dump"                      => dump::run("dump",  &ctx).await,
        "sdump"                     => dump::run("sdump", &ctx).await,
        "ldump"                     => dump::run("ldump", &ctx).await,
        "pdump"                     => dump::run("pdump", &ctx).await,

        "census" | "cens"           => census::run(args, &ctx).await,
        "nation" | "nati"           => nation_cmd::run(args, &ctx).await,
        "map"                       => map_cmd::run(args, &ctx).await,
        "bmap"                      => map_cmd::run(args, &ctx).await,
        "smap" | "sector" | "sect"  => map_cmd::run(args, &ctx).await,
        "designate" | "desi" | "des" => designate::run(args, &ctx).await,
        "threshold" | "thre" | "thresh" => threshold::run(args, &ctx).await,
        "relations" | "rela"        => relations_cmd::run(args, &ctx).await,
        "declare"   | "decl"        => declare::run(args, &ctx).await,

        "add"                       => add::run(args, &ctx).await,
        "capital"   | "capi"        => capital::run(args, &ctx).await,
        "newcap"                    => capital::run_newcap(args, &ctx).await,
        "enable"    | "enab"        => enable::run_enable(&ctx).await,
        "disable"   | "disa"        => enable::run_disable(&ctx).await,
        "shutdown"  | "shut"        => shutdown_cmd::run(args, &ctx).await,
        "distribute"| "dist"        => distribute::run(args, &ctx).await,
        "deliver"   | "deli"        => deliver::run(args, &ctx).await,
        "show"                      => show::run(args, &ctx).await,
        "power"     | "powe"        => power::run(args, &ctx).await,
        "build"     | "buil"        => build::run(args, &ctx).await,
        "march"                     => march::run(args, &ctx).await,
        "navigate"  | "nav"         => navigate::run(args, &ctx).await,
        "attack"    | "atta"        => attack::run(args, &ctx).await,
        "bomb"                      => bomb::run(args, &ctx).await,
        "fly"                       => fly::run(args, &ctx).await,
        "launch"    | "lnch"        => launch::run(args, &ctx).await,
        "mission"   | "miss"        => mission::run(args, &ctx).await,

        "sell"              => sell::run(args, &ctx).await,
        "buy"               => buy::run(args, &ctx).await,
        "trade" | "trad"    => trade::run(args, &ctx).await,
        "loan"              => loan::run(args, &ctx).await,

        "explore" | "expl"  => explore::run(args, &ctx).await,
        "move"    | "mov"   => move_cmd::run(args, &ctx).await,

        "realm"   | "real"  => realm::run(args, &ctx).await,
        "report"  | "repo"  => report::run(args, &ctx).await,
        "resource"| "reso"  => resource::run(args, &ctx).await,
        "commodity"| "comm" => commodity::run(args, &ctx).await,
        "change"  | "chan"  => change::run(args, &ctx).await,
        "force"   | "forc"  => force::run(args, &ctx).await,
        "update"  | "upda"  => update_cmd::run(args, &ctx).await,

        "telegram"| "tele"  => telegram_cmd::run(args, &ctx).await,
        "read"    | "rea"   => read_cmd::run(args, &ctx).await,
        "announce"| "anno"  => announce_cmd::run(args, &ctx).await,
        "pray"              => telegram_cmd::run(&format!("0 {args}"), &ctx).await,

        "ship"    | "shp"   => ship_cmd::run(args, &ctx).await,
        "load"              => load_cmd::run(args, &ctx).await,
        "unload"  | "unlo"  => load_cmd::run_unload(args, &ctx).await,
        "enlist"  | "enli"  => enlist_cmd::run(args, &ctx).await,
        "demobilize" | "demo" => demobilize_cmd::run(args, &ctx).await,
        "assault" | "assa"  => assault_cmd::run(args, &ctx).await,
        "execute" | "exec"  => execute_cmd::run(args, &ctx).await,
        "news"              => news_cmd::run(args, &ctx).await,

        _ => response(code::BADCMD, &format!("Unknown command: {cmd}")),
    }
}

fn echo_cmd(args: &str) -> String {
    format!("{} {args}\n{} echo\n", code::INIT, code::DATA)
}
