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
// Ported from: include/plane.h
// Known contributors to the original:
//    Dave Pare, 1986
//    Ken Stevens, 1995
//    Steve McClure, 1998

// ref: include/plane.h

use serde::{Deserialize, Serialize};
use crate::coords::{Coord, NatId};

pub const PLANE_MIN_EFF: i8 = 10;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PlaneFlags: u32 {
        const LAUNCHED    = 1 << 0;
        const SYNCHRONOUS = 1 << 1;
        const AIRBURST    = 1 << 2;
    }
}
impl serde::Serialize for PlaneFlags {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.bits())
    }
}
impl<'de> serde::Deserialize<'de> for PlaneFlags {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bits = u32::deserialize(d)?;
        Ok(Self::from_bits_truncate(bits))
    }
}

/// Full plane record.  ref: struct plnstr in plane.h
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plane {
    pub uid: i32,
    pub own: NatId,
    pub x: Coord,
    pub y: Coord,
    pub plane_type: i8,     // index into plane-type table
    pub effic: i8,
    pub mobil: i8,
    pub off: bool,
    pub tech: i16,
    pub wing: char,
    pub opx: Coord,
    pub opy: Coord,
    pub mission: i16,
    pub mission_radius: i16,

    pub range: u8,          // total range in sectors
    pub harden: i8,         // missile hardening (0–100)
    pub ship: i32,          // carrier ship UID, or -1
    pub land: i32,          // carrying land unit UID, or -1
    pub flags: PlaneFlags,
    pub access: i16,
    pub theta: f32,         // orbital position (sine wave)
}
