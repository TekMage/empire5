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
// Ported from: include/types.h, include/xy.h
// Known contributors to the original:
//    Markus Armbruster, 2006-2014 (types.h)
//    Steve McClure, 1998 (xy.h)

// ref: include/types.h, include/xy.h
//
// Empire uses a 2D toroidal grid where only sectors at positions where
// (x + y) is even are valid ("even parity" hex-like grid).  Coordinates
// wrap around at WORLD_X and WORLD_Y boundaries.

use serde::{Deserialize, Serialize};

/// Signed sector coordinate.  Equivalent to C `coord` (short).
pub type Coord = i16;

/// Nation identifier.  0..MAX_NATIONS-1, NAT_ID_BAD = 255.  Equivalent to C `natid` (u8).
pub type NatId = u8;

/// Linear index into the sector array.  Computed from (x, y) via [`xy_offset`].
pub type XyOffset = i32;

/// Axis-aligned rectangle in map coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub lx: Coord,
    pub ly: Coord,
    pub hx: Coord,
    pub hy: Coord,
}

impl Range {
    pub fn width(&self) -> i32 {
        (self.hx as i32 - self.lx as i32).abs() + 1
    }
    pub fn height(&self) -> i32 {
        (self.hy as i32 - self.ly as i32).abs() + 1
    }
}

/// Return the number of valid sectors for a world of the given dimensions.
/// Only sectors where (x + y) is even are valid.
pub fn world_size(world_x: i32, world_y: i32) -> i32 {
    world_x * world_y / 2
}

/// Compute the linear sector index for normalized coordinates (x, y).
/// Equivalent to C macro `XYOFFSET(x, y)`.
pub fn xy_offset(x: Coord, y: Coord, world_x: i32) -> XyOffset {
    ((y as i32 * world_x) + x as i32) / 2
}

/// Normalize x coordinate to [0, world_x).
/// Equivalent to C macro `XNORM(x)`.
pub fn x_norm(x: Coord, world_x: i32) -> Coord {
    let wx = world_x as i32;
    if x < 0 {
        (wx - 1 - ((-x as i32 - 1) % wx)) as Coord
    } else {
        (x as i32 % wx) as Coord
    }
}

/// Normalize y coordinate to [0, world_y).
/// Equivalent to C macro `YNORM(y)`.
pub fn y_norm(y: Coord, world_y: i32) -> Coord {
    let wy = world_y as i32;
    if y < 0 {
        (wy - 1 - ((-y as i32 - 1) % wy)) as Coord
    } else {
        (y as i32 % wy) as Coord
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_size_standard() {
        // Standard small game: 32x16 = 256 sectors
        assert_eq!(world_size(32, 16), 256);
        // Standard large game: 64x32 = 1024 sectors
        assert_eq!(world_size(64, 32), 1024);
    }

    #[test]
    fn xy_offset_origin() {
        assert_eq!(xy_offset(0, 0, 32), 0);
    }

    #[test]
    fn x_norm_wraps_negative() {
        assert_eq!(x_norm(-1, 32), 31);
        assert_eq!(x_norm(-32, 32), 0);
    }

    #[test]
    fn y_norm_wraps_positive() {
        assert_eq!(y_norm(16, 16), 0);
        assert_eq!(y_norm(17, 16), 1);
    }
}
