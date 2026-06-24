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
// Ported from: include/land.h
// Known contributors to the original:
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995
//    Steve McClure, 1998

// ref: include/land.h

use serde::{Deserialize, Serialize};
use crate::coords::{Coord, NatId};
use crate::commodity::Inventory;
use crate::ship::RetreatFlags;

pub const LAND_MIN_EFF: i8 = 10;
pub const LAND_MIN_FIRE_EFF: i8 = 40;

/// Full land unit record.  ref: struct lndstr in land.h
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandUnit {
    pub uid: i32,
    pub own: NatId,
    pub x: Coord,
    pub y: Coord,
    pub land_type: i8,       // index into land-type table
    pub effic: i8,
    pub mobil: i8,
    pub off: bool,
    pub tech: i16,
    pub army: char,
    pub opx: Coord,
    pub opy: Coord,
    pub mission: i16,
    pub mission_radius: i16,

    pub ship: i32,           // transporting ship UID, or -1
    pub harden: i8,          // fortification level 0–100
    pub retreat: i16,        // retreat percentage
    pub retreat_flags: RetreatFlags,
    pub retreat_path: String,
    pub scar: u8,            // experience (unused in current rules)
    pub items: Inventory,
    pub pstage: i16,
    pub ptime: i16,
    pub carried_by_land: i32, // transporting land unit UID, or -1
    pub access: i16,
}
