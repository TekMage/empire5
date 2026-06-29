// Per-nation fog-of-war map.
// Mirrors 4.4.1's per-player bmap file (WORLD_X * WORLD_Y / 2 bytes).
//
// We store a flat byte array of world_x * world_y bytes, indexed by
// (y_norm * world_x + x_norm).  Only positions where (x+y) is even
// are valid Empire sectors; others are ignored.
//
// 0   = sector never seen
// any ASCII mnemonic = last-seen designation character

use crate::{Db, DbResult};

/// In-memory fog-of-war map for one nation.
pub struct Bmap {
    pub data: Vec<u8>,
    pub world_x: usize,
    pub world_y: usize,
}

impl Bmap {
    /// Blank map — all sectors unseen.
    pub fn new(world_x: usize, world_y: usize) -> Self {
        Self { data: vec![0u8; world_x * world_y], world_x, world_y }
    }

    /// Reconstruct from a stored BLOB.  If the blob is the wrong size,
    /// returns a blank map (triggers re-initialization on next radar sweep).
    pub fn from_blob(data: Vec<u8>, world_x: usize, world_y: usize) -> Self {
        let size = world_x * world_y;
        if data.len() == size {
            Self { data, world_x, world_y }
        } else {
            Self::new(world_x, world_y)
        }
    }

    fn idx(&self, x: i16, y: i16) -> usize {
        let nx = ((x as i32).rem_euclid(self.world_x as i32)) as usize;
        let ny = ((y as i32).rem_euclid(self.world_y as i32)) as usize;
        ny * self.world_x + nx
    }

    pub fn get(&self, x: i16, y: i16) -> u8 {
        self.data[self.idx(x, y)]
    }

    pub fn set(&mut self, x: i16, y: i16, ch: u8) {
        let i = self.idx(x, y);
        self.data[i] = ch;
    }

    pub fn is_seen(&self, x: i16, y: i16) -> bool {
        self.get(x, y) != 0
    }

    pub fn is_empty(&self) -> bool {
        self.data.iter().all(|&b| b == 0)
    }
}

/// Load the bmap for nation `cnum` from the database.
pub async fn get_bmap(db: &Db, cnum: u8, world_x: usize, world_y: usize) -> DbResult<Bmap> {
    let row: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT bmap FROM nations WHERE cnum = ?"
    )
    .bind(cnum as i64)
    .fetch_optional(db.pool())
    .await?;

    Ok(match row {
        Some(data) => Bmap::from_blob(data, world_x, world_y),
        None       => Bmap::new(world_x, world_y),
    })
}

/// Persist the bmap for nation `cnum`.
pub async fn put_bmap(db: &Db, cnum: u8, bmap: &Bmap) -> DbResult<()> {
    sqlx::query("UPDATE nations SET bmap = ? WHERE cnum = ?")
        .bind(&bmap.data)
        .bind(cnum as i64)
        .execute(db.pool())
        .await?;
    Ok(())
}
