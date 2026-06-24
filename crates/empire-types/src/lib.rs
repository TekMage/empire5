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

// Core game-object types for Empire 5.
// Each module maps 1-to-1 with the corresponding C header in reference/include/.
// C source cross-references are noted as: /* ref: <header.h> */

pub mod coords;
pub mod commodity;
pub mod nation;
pub mod sector;
pub mod ship;
pub mod plane;
pub mod land;
pub mod nuke;
pub mod unit;
pub mod selector;

pub use coords::{Coord, NatId, XyOffset, Range};
pub use commodity::{Item, Packaging};
pub use nation::{NatStatus, NatFlags, Nation, Realm};
pub use sector::{Sector, SectorType};
pub use ship::Ship;
pub use plane::Plane;
pub use land::LandUnit;
pub use nuke::Nuke;

pub const MAX_NATIONS: usize = 99;   /* MAXNOC in configure.ac */
pub const MAX_REALMS: usize = 50;    /* MAXNOR in nat.h */
pub const NAT_ID_BAD: NatId = 255;  /* NATID_BAD in nat.h */
