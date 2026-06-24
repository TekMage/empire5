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
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/client/play.c
// Known contributors to the original:
//    Markus Armbruster, 2007-2017
//    Ron Koenderink, 2007-2009
//    Martin Haukeli, 2015

// empire-client: Terminal client (Phase 7 — optional Rust port of src/client/)
//
// The original C client (src/client/play.c) remains fully compatible with
// the Rust server.  This crate is a placeholder for a future Rust client.
//
// Phase 0 stub.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "empire-client", about = "Empire 5 terminal client (Phase 7 stub)")]
struct Args {
    /// Server hostname
    #[arg(short, long, default_value = "localhost")]
    server: String,
    /// Server port
    #[arg(short, long, default_value_t = 6665)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!(
        "empire-client: Phase 7 stub — use the C client from empire4.4.1/src/client/"
    );
    println!("Would connect to {}:{}", args.server, args.port);
    println!("Build the C client: cd ../empire4.4.1 && make");
    Ok(())
}
