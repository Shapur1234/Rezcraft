use std::{iter, num::NonZeroI32};

use block_mesh::ndshape::ConstShape;
use cgmath::Vector3;
use noise::{Cache, NoiseFn, Perlin};
use rand::prelude::*;

use crate::{
    game::world::{Block, BlockBuffer, BlockManager, ChunkShape, CHUNK_SIZE},
    misc::pos::Pos,
};

const BASE_GROUND_LEVEL: f64 = -10.0;
const SEA_LEVEL: i32 = 0;
const HILLINESS: f64 = 20.0;
const LEVELS_OF_DIRT: u32 = 5;

#[derive(Clone, Debug)]
pub struct TerrainGenerator {
    #[allow(dead_code)]
    seed: u32,
    noise: Cache<Perlin>,
    block_manager: BlockManager,
}

impl TerrainGenerator {
    pub fn new(seed: u32, block_manager: BlockManager) -> Self {
        Self {
            seed,
            block_manager,
            noise: Cache::new(Perlin::new(seed)),
        }
    }

    pub fn generate_blocks(&mut self, chunk_pos: &Vector3<NonZeroI32>) -> BlockBuffer {
        let mut blocks = Vec::from_iter(iter::repeat(Block::default()).take((CHUNK_SIZE as usize).pow(3)));

        for x in 0..CHUNK_SIZE as usize {
            for y in 0..CHUNK_SIZE as usize {
                for z in 0..CHUNK_SIZE as usize {
                    let block_pos = Pos::new(*chunk_pos, Vector3::new(x as f32, y as f32, z as f32));

                    let index = block_pos.in_chunk_pos_i32();
                    blocks[ChunkShape::linearize([index.x as u32, index.y as u32, index.z as u32]) as usize] = self
                        .generate_block(&{
                            let abs_pos = block_pos.abs_pos();
                            Vector3::new(abs_pos.x as i32, abs_pos.y as i32, abs_pos.z as i32)
                        });
                }
            }
        }

        BlockBuffer::new(blocks)
    }

    fn generate_block(&mut self, abs_pos: &Vector3<i32>) -> Block {
        let xy = [abs_pos.x as f64 / 100.0, abs_pos.z as f64 / 100.0];
        let ground_y = (BASE_GROUND_LEVEL - ((self.noise.get(xy) - 0.5) * HILLINESS)) as i32;

        let block_name = if abs_pos.y > ground_y {
            if abs_pos.y <= SEA_LEVEL {
                "Water"
            } else {
                "Air"
            }
        } else {
            if abs_pos.y == ground_y {
                if ground_y < SEA_LEVEL {
                    "Sand"
                } else {
                    "Grass"
                }
            } else if abs_pos.y > ground_y - LEVELS_OF_DIRT as i32 {
                if ground_y < SEA_LEVEL {
                    "Sand"
                } else {
                    "Dirt"
                }
            } else {
                "Stone"
            }
        };

        Block::new_with_default(block_name, &self.block_manager)
    }

    pub fn generate_seed() -> u32 {
        let mut rng = rand::thread_rng();
        rng.gen()
    }
}
