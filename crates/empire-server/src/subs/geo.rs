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
// Ported from: src/lib/global/dir.c, src/lib/common/xy.c,
//              src/lib/common/mapdist.c
// Known contributors to the original:
//    Dave Pare, 1989

// Geographic utilities: directions, distance, coordinate formatting.
// These are pure functions; callers pass world dimensions from Config.

use empire_types::coords::{Coord, NatId, Range};
use empire_types::nation::Nation;

// ── Direction constants (must agree with path.h and diroff[]) ─────────────────

/// Directional stop / halt.
pub const DIR_STOP: usize = 0;
/// Up-right.
pub const DIR_UR:   usize = 1;
/// Right.
pub const DIR_R:    usize = 2;
/// Down-right.
pub const DIR_DR:   usize = 3;
/// Down-left.
pub const DIR_DL:   usize = 4;
/// Left.
pub const DIR_L:    usize = 5;
/// Up-left.
pub const DIR_UL:   usize = 6;

/// First compass direction index.
pub const DIR_FIRST: usize = 1;
/// Last compass direction index.
pub const DIR_LAST:  usize = 6;

/// Direction as a newtype for clarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dir(pub usize);

impl Dir {
    /// Opposite direction: UR↔DL, R↔L, DR↔UL.
    pub fn back(self) -> Dir {
        let d = self.0;
        if d >= DIR_FIRST + 3 { Dir(d - 3) } else { Dir(d + 3) }
    }
}

/// (dx, dy) offsets for each direction.  Index 0 = stop, indices 1-6 = compass.
/// ref: dir.c diroff[][]
pub const DIROFF: [(i16, i16); 7] = [
    (0, 0),    // DIR_STOP
    (1, -1),   // DIR_UR
    (2, 0),    // DIR_R
    (1, 1),    // DIR_DR
    (-1, 1),   // DIR_DL
    (-2, 0),   // DIR_L
    (-1, -1),  // DIR_UL
];

/// Key characters for each direction (matching dirch[] in dir.c).
pub const DIRCH: [char; 7] = ['h', 'u', 'j', 'n', 'b', 'g', 'y'];

/// Map a direction character (from player input) to a DIR_* index.
/// Returns None for unrecognized characters.
pub fn dir_from_char(c: char) -> Option<usize> {
    DIRCH.iter().position(|&d| d == c)
}

// ── Coordinate utilities (xy.c) ──────────────────────────────────────────────

/// Normalize x coordinate to [0, world_x).  Equivalent to XNORM(x).
pub fn x_norm(x: Coord, world_x: i32) -> Coord {
    let wx = world_x as i32;
    ((x as i32 % wx + wx) % wx) as Coord
}

/// Normalize y coordinate to [0, world_y).  Equivalent to YNORM(y).
pub fn y_norm(y: Coord, world_y: i32) -> Coord {
    let wy = world_y as i32;
    ((y as i32 % wy + wy) % wy) as Coord
}

/// Convert absolute x to player-relative x.  Equivalent to xrel().
pub fn x_rel(nat: &Nation, abs_x: Coord, world_x: i32) -> Coord {
    let x = x_norm(abs_x.wrapping_sub(nat.xorg), world_x) as i32;
    if x >= world_x / 2 { (x - world_x) as Coord } else { x as Coord }
}

/// Convert absolute y to player-relative y.  Equivalent to yrel().
pub fn y_rel(nat: &Nation, abs_y: Coord, world_y: i32) -> Coord {
    let y = y_norm(abs_y.wrapping_sub(nat.yorg), world_y) as i32;
    if y >= world_y / 2 { (y - world_y) as Coord } else { y as Coord }
}

/// Convert player-relative x back to absolute.  Equivalent to xabs().
pub fn x_abs(nat: &Nation, rel_x: Coord, world_x: i32) -> Coord {
    x_norm(rel_x.wrapping_add(nat.xorg), world_x)
}

/// Convert player-relative y back to absolute.  Equivalent to yabs().
pub fn y_abs(nat: &Nation, rel_y: Coord, world_y: i32) -> Coord {
    y_norm(rel_y.wrapping_add(nat.yorg), world_y)
}

/// Format absolute coordinates as "x,y" relative to `nation`'s origin.
/// Equivalent to C's `xyas(x, y, cnum)`.
pub fn format_xy(nat: &Nation, x: Coord, y: Coord, world_x: i32, world_y: i32) -> String {
    format!("{},{}", x_rel(nat, x, world_x), y_rel(nat, y, world_y))
}

/// Format absolute coordinates of `sector`'s owner relative to their own origin.
pub fn format_own_xy(nat: &Nation, x: Coord, y: Coord, world_x: i32, world_y: i32) -> String {
    format_xy(nat, x, y, world_x, world_y)
}

// ── Distance (mapdist.c) ──────────────────────────────────────────────────────

fn delta_x(x1: Coord, x2: Coord, world_x: i32) -> i32 {
    let mut dx = (x1 as i32 - x2 as i32).abs() % world_x;
    if dx > world_x / 2 { dx = world_x - dx; }
    dx
}

fn delta_y(y1: Coord, y2: Coord, world_y: i32) -> i32 {
    let mut dy = (y1 as i32 - y2 as i32).abs() % world_y;
    if dy > world_y / 2 { dy = world_y - dy; }
    dy
}

/// Chebyshev-like distance on Empire's toroidal hex grid.
/// Equivalent to C's `mapdist(x1, y1, x2, y2)`.
pub fn map_dist(x1: Coord, y1: Coord, x2: Coord, y2: Coord, world_x: i32, world_y: i32) -> i32 {
    let dx = delta_x(x1, x2, world_x);
    let dy = delta_y(y1, y2, world_y);
    if dx > dy { (dx - dy) / 2 + dy } else { dy }
}

// ── Range helpers (xy.c) ─────────────────────────────────────────────────────

/// Build a bounding Range for all sectors within `dist` of (cx, cy).
/// Equivalent to C's `xydist_range`.
pub fn xydist_range(cx: Coord, cy: Coord, dist: i32, world_x: i32, world_y: i32) -> Range {
    let (lx, hx, _width) = if 4 * dist + 1 < world_x {
        let lx = x_norm(cx - (2 * dist) as Coord, world_x);
        let hx = x_norm(cx + (2 * dist) as Coord, world_x);
        (lx, hx, 4 * dist + 1)
    } else {
        let lx = x_norm(cx - (world_x / 2) as Coord, world_x);
        let hx = x_norm(lx + (world_x - 1) as Coord, world_x);
        (lx, hx, world_x)
    };
    let (ly, hy, _height) = if 2 * dist + 1 < world_y {
        let ly = y_norm(cy - dist as Coord, world_y);
        let hy = y_norm(cy + dist as Coord, world_y);
        (ly, hy, 2 * dist + 1)
    } else {
        let ly = y_norm(cy - (world_y / 2) as Coord, world_y);
        let hy = y_norm(ly + (world_y - 1) as Coord, world_y);
        (ly, hy, world_y)
    };
    Range { lx, hx, ly, hy }
}

/// True if (x, y) is within the closed range `r`.  Handles wrap-around.
pub fn xy_in_range(x: Coord, y: Coord, r: &Range) -> bool {
    let x_ok = if r.lx <= r.hx {
        x >= r.lx && x <= r.hx
    } else {
        x >= r.lx || x <= r.hx
    };
    if !x_ok { return false; }
    if r.ly <= r.hy {
        y >= r.ly && y <= r.hy
    } else {
        y >= r.ly || y <= r.hy
    }
}

// ── Neighbor iteration ────────────────────────────────────────────────────────

/// Return the 6 neighboring sector coordinates of (x, y), normalized.
/// Equivalent to iterating `for i in DIR_FIRST..=DIR_LAST { getsect(x+diroff[i][0], ...) }`.
pub fn neighbors(x: Coord, y: Coord, world_x: i32, world_y: i32) -> [(Coord, Coord); 6] {
    let mut result = [(0i16, 0i16); 6];
    for (i, &(dx, dy)) in DIROFF[DIR_FIRST..=DIR_LAST].iter().enumerate() {
        result[i] = (x_norm(x + dx, world_x), y_norm(y + dy, world_y));
    }
    result
}

/// True if any of the 6 neighbors of (x, y) is owned by `own`.
/// Equivalent to C's `neigh(x, y, own)`.
pub fn is_neighbor_of(
    x: Coord, y: Coord,
    own: NatId,
    world_x: i32, world_y: i32,
    get_sector_own: impl Fn(Coord, Coord) -> NatId,
) -> bool {
    neighbors(x, y, world_x, world_y)
        .iter()
        .any(|&(nx, ny)| get_sector_own(nx, ny) == own)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diroff_stop_is_zero() {
        assert_eq!(DIROFF[DIR_STOP], (0, 0));
    }

    #[test]
    fn dir_back_roundtrip() {
        for d in DIR_FIRST..=DIR_LAST {
            assert_eq!(Dir(d).back().back().0, d);
        }
    }

    #[test]
    fn x_norm_negative() {
        assert_eq!(x_norm(-1, 32), 31);
        assert_eq!(x_norm(-32, 32), 0);
    }

    #[test]
    fn map_dist_same_sector() {
        assert_eq!(map_dist(0, 0, 0, 0, 64, 32), 0);
    }

    #[test]
    fn map_dist_adjacent_ur() {
        // DIR_UR: dx=1, dy=-1 in absolute; map distance should be 1
        assert_eq!(map_dist(0, 0, 1, 31, 64, 32), 1);
    }

    #[test]
    fn neighbors_count() {
        let n = neighbors(4, 4, 64, 32);
        assert_eq!(n.len(), 6);
    }

    #[test]
    fn xy_in_range_simple() {
        let r = Range { lx: 0, hx: 10, ly: 0, hy: 10 };
        assert!(xy_in_range(5, 5, &r));
        assert!(!xy_in_range(15, 5, &r));
    }

    #[test]
    fn xydist_range_small_dist() {
        let r = xydist_range(10, 10, 2, 64, 32);
        // dist=2: lx = 10-4=6, hx=10+4=14 — width 9
        assert_eq!(r.lx, 6);
        assert_eq!(r.hx, 14);
        assert_eq!(r.ly, 8);
        assert_eq!(r.hy, 12);
    }
}
