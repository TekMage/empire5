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
// Ported from: include/ship.h
// Known contributors to the original:
//    Dave Pare
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995

// ref: include/ship.h

use serde::{Deserialize, Serialize};
use crate::coords::{Coord, NatId};
use crate::commodity::Inventory;

pub const SHIP_MIN_EFF: i8 = 20;
pub const MAX_SHIP_NAME_LEN: usize = 24;
pub const RETREAT_PATH_LEN: usize = 16; // RET_LEN in retreat.h

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RetreatFlags: u32 {
        const INJURED  = 0x01;
        const TORPED   = 0x02;
        const BOMBED   = 0x04;
        const BOARDED  = 0x08;
        const SCARED   = 0x10;
        const HELPLESS = 0x20;
    }
}
impl serde::Serialize for RetreatFlags {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.bits())
    }
}
impl<'de> serde::Deserialize<'de> for RetreatFlags {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bits = u32::deserialize(d)?;
        Ok(Self::from_bits_truncate(bits))
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ShipFlags: u32 {
        const SUBMARINE  = 1 << 0;
        const SUPPLY     = 1 << 1;
        const MINE_LAYER = 1 << 2;
        const MINE_SWEEP = 1 << 3;
        const FERRY      = 1 << 4;
        const ANTI_SHIP  = 1 << 5;
        const ANTI_AIR   = 1 << 6;
        const RADAR      = 1 << 7;
        const NUCLEAR    = 1 << 8;
        const OILER      = 1 << 9;
        const REPAIR     = 1 << 10;
        const TORP       = 1 << 11;
        const CARRIER    = 1 << 12;
    }
}
impl serde::Serialize for ShipFlags {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.bits())
    }
}
impl<'de> serde::Deserialize<'de> for ShipFlags {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bits = u32::deserialize(d)?;
        Ok(Self::from_bits_truncate(bits))
    }
}

/// Full ship record.  ref: struct shpstr in ship.h
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ship {
    pub uid: i32,
    pub own: NatId,
    pub x: Coord,
    pub y: Coord,
    pub ship_type: i8,      // index into ship-type table
    pub effic: i8,          // 0–100%
    pub mobil: i8,
    pub off: bool,
    pub tech: i16,
    pub fleet: char,
    pub opx: Coord,
    pub opy: Coord,
    pub mission: i16,
    pub mission_radius: i16,

    pub items: Inventory,
    pub pstage: i16,
    pub ptime: i16,
    pub access: i16,
    pub name: String,
    pub orig_x: Coord,
    pub orig_y: Coord,
    pub orig_own: NatId,
    pub retreat_flags: RetreatFlags,
    pub retreat_path: String,
}
