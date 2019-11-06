use crate::block::BlockId;

/// Number of blocks along an axis of the chunk
pub const CHUNK_SIZE: u32 = 32;

/// Position of a chunk in the world
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub px: i64,
    pub py: i64,
    pub pz: i64,
}

impl ChunkPos {
    /// Offset the current chunk position by some amount of chunks
    pub fn offset(self, dx: i64, dy: i64, dz: i64) -> Self {
        Self {
            px: self.px + dx,
            py: self.py + dy,
            pz: self.pz + dz,
        }
    }
}

impl From<(i64, i64, i64)> for ChunkPos {
    fn from((px, py, pz): (i64, i64, i64)) -> Self {
        Self { px, py, pz }
    }
}

/// An RLE-compressed chunk
#[derive(Debug, Clone)]
pub struct CompressedChunk {
    pub pos: ChunkPos,
    pub data: Vec<(u16, BlockId)>,
}

impl CompressedChunk {
    /// Compress `chunk` using RLE
    pub fn from_chunk(chunk: &Chunk) -> Self {
        let mut compressed_data = Vec::new();
        let mut current_block = chunk.data[0];
        let mut current_block_count = 0;
        for i in 0..(CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize {
            if chunk.data[i] != current_block {
                compressed_data.push((current_block_count, current_block));
                current_block = chunk.data[i];
                current_block_count = 0;
            }
            current_block_count += 1;
        }

        compressed_data.push((current_block_count, current_block));

        Self {
            pos: chunk.pos,
            data: compressed_data,
        }
    }

    /// Recover original chunk
    pub fn to_chunk(&self) -> Chunk {
        let mut data = Vec::new();
        data.resize((CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize, 0);

        let mut i = 0;
        for &(len, block) in self.data.iter() {
            for j in 0..len {
                data[(i + j) as usize] = block;
            }
            i += len;
        }

        Chunk {
            pos: self.pos,
            data,
        }
    }
}

/// A chunk
#[derive(Debug, Clone)]
pub struct Chunk {
    pub pos: ChunkPos,
    pub(super) data: Vec<BlockId>,
}

impl Chunk {
    /// Create a new empty chunk
    pub fn new(pos: ChunkPos) -> Self {
        let data: Vec<BlockId> = unsafe {
            crate::collections::zero_initialized_vec(
                (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize,
            )
        };
        Self { pos, data }
    }

    /// Get block at some position
    #[inline]
    pub fn get_block_at(&self, (px, py, pz): (u32, u32, u32)) -> BlockId {
        self.data[(px * CHUNK_SIZE * CHUNK_SIZE + py * CHUNK_SIZE + pz) as usize]
    }

    /// Set block at some position
    #[inline]
    pub fn set_block_at(&mut self, (px, py, pz): (u32, u32, u32), block: BlockId) {
        self.data[(px * CHUNK_SIZE * CHUNK_SIZE + py * CHUNK_SIZE + pz) as usize] = block;
    }
}
