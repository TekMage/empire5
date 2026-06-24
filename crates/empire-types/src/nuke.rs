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
// Ported from: include/nuke.h
// Known contributors to the original:
//    Dave Pare, 1986
//    Markus Armbruster, 2004-2020

// ref: include/nuke.h

use serde::{Deserialize, Serialize};
use crate::coords::{Coord, NatId};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NukeFlags: u32 {
        const NEUTRON = 1 << 0;
    }
}
impl serde::Serialize for NukeFlags {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.bits())
    }
}
impl<'de> serde::Deserialize<'de> for NukeFlags {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bits = u32::deserialize(d)?;
        Ok(Self::from_bits_truncate(bits))
    }
}

/// Full nuclear weapon record.  ref: struct nukstr in nuke.h
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nuke {
    pub uid: i32,
    pub own: NatId,
    pub x: Coord,
    pub y: Coord,
    pub nuke_type: i8,    // index into nuke-type table
    pub effic: i8,        // always 100 in practice
    pub tech: i16,
    pub stockpile: char,  // group membership letter

    pub plane: i32,       // transporting plane UID, or -1
}
