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

// Game subsystems (src/lib/subs/ from empire4.x).
// Pure, async-free functions that operate on game objects and are
// called by command handlers (src/lib/commands/) and the update engine.

pub mod geo;
pub mod damage;
pub mod control;
pub mod takeover;
pub mod nat_util;
pub mod pathfind;
pub mod tech;
pub mod shpsub;
pub mod lndsub;
pub mod fortsub;
pub mod plnsub;
pub mod satsub;
pub mod aircombat;
pub mod attsub;
pub mod shipcarry;
