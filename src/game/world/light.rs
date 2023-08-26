use std::{cmp::max, iter, ops::Index, sync::Arc};

use block_mesh::ndshape::ConstShape;
use cgmath::Vector3;
use rle_vec::RleVec;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::{
    engine::face::FaceDirection,
    game::world::{coordinate_in_surrounding_buffers_cube, BlockBuffer, ChunkShape, CHUNK_SIZE},
    misc::index::{index_from_relative_pos_surrounding_cubes, relative_pos_surrounding_cubes_from_index},
};

pub const MAX_LIGHT_VAL: u8 = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LightSource {
    pub red: bool,
    pub green: bool,
    pub blue: bool,
    pub strength: u8,
}

impl LightSource {
    pub fn new(red: bool, green: bool, blue: bool, strength: u8) -> Self {
        debug_assert!(strength <= MAX_LIGHT_VAL);
        debug_assert!(red || green || blue);

        Self {
            red,
            green,
            blue,
            strength,
        }
    }

    pub fn is_valid(&self) -> bool {
        debug_assert!(self.strength <= MAX_LIGHT_VAL);

        self.red || self.green || self.blue
    }

    pub fn light_raw(&self) -> [u8; 4] {
        [
            if self.red { self.strength } else { 0 },
            if self.green { self.strength } else { 0 },
            if self.blue { self.strength } else { 0 },
            0,
        ]
    }
}

impl Default for LightSource {
    fn default() -> Self {
        Self::new(true, true, true, MAX_LIGHT_VAL)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LightVal {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub sun: u8,
}

impl LightVal {
    pub fn new(red: u8, green: u8, blue: u8, sun: u8) -> Self {
        Self { red, green, blue, sun }
    }

    pub fn light_raw(&self) -> [u8; 4] {
        [self.red, self.green, self.blue, self.sun]
    }
}

impl From<LightSource> for LightVal {
    fn from(value: LightSource) -> Self {
        Self::new(
            if value.red { value.strength } else { 0 },
            if value.green { value.strength } else { 0 },
            if value.blue { value.strength } else { 0 },
            0,
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Hash)]
pub struct LightBuffer {
    buffer: RleVec<LightVal>,
}

impl LightBuffer {
    pub fn new<'a>(surrounding_blocks: [Arc<BlockBuffer>; 27]) -> Option<Self> {
        let (light_sources, sunlight_sources) = {
            let (mut light_sources_temp, mut sunlight_sources_temp) = (Vec::new(), Vec::new());

            for (index, blocks) in surrounding_blocks.iter().enumerate() {
                let chunk_offset = relative_pos_surrounding_cubes_from_index(index as u8);

                light_sources_temp.extend(blocks.light_sources()?.cache().iter().map(|in_chunk_pos| {
                    (
                        in_chunk_pos + chunk_offset * CHUNK_SIZE as i32,
                        blocks[&in_chunk_pos].light_source().unwrap(),
                    )
                }));
                sunlight_sources_temp.extend(
                    blocks
                        .sunlight_sources()?
                        .cache()
                        .iter()
                        .map(|in_chunk_pos| in_chunk_pos + chunk_offset * CHUNK_SIZE as i32),
                );
            }

            (light_sources_temp, sunlight_sources_temp)
        };

        let mut lights = Self::new_unlit();

        lights.handle_sunlight(&surrounding_blocks);
        for in_chunk_pos in sunlight_sources {
            lights.spread_sunlight_from_source(in_chunk_pos, &surrounding_blocks)
        }

        for (in_chunk_pos, source) in light_sources {
            lights.spread_light_from_source(
                in_chunk_pos,
                source.red,
                source.green,
                source.blue,
                source.strength,
                &surrounding_blocks,
            )
        }

        Some(lights)
    }

    pub fn set(&mut self, index: &Vector3<i32>, val: LightVal) {
        self.buffer.set(
            ChunkShape::linearize([index.x as u32, index.y as u32, index.z as u32]) as usize,
            val,
        )
    }

    fn new_unlit() -> Self {
        Self {
            buffer: iter::repeat(LightVal::default())
                .take((CHUNK_SIZE as usize).pow(3))
                .collect(),
        }
    }

    fn handle_sunlight(&mut self, surrounding_blocks: &[Arc<BlockBuffer>; 27]) {
        for x in 0..CHUNK_SIZE as i32 {
            for z in 0..CHUNK_SIZE as i32 {
                for y in 0..CHUNK_SIZE as i32 {
                    if surrounding_blocks[index_from_relative_pos_surrounding_cubes(&Vector3::new(0, 0, 0)) as usize]
                        [&Vector3::new(x, y, z)]
                        .is_sunlit()
                    {
                        self.set(&Vector3::new(x, y, z), LightVal::new(0, 0, 0, MAX_LIGHT_VAL))
                    }
                }
            }
        }
    }

    fn spread_light_from_source(
        &mut self,
        source_in_chunk_pos: Vector3<i32>,
        source_red: bool,
        source_green: bool,
        source_blue: bool,
        source_strength: u8,
        surrounding_blocks: &[Arc<BlockBuffer>; 27],
    ) {
        if source_in_chunk_pos.x > -(MAX_LIGHT_VAL as i32)
            && source_in_chunk_pos.x < CHUNK_SIZE as i32 + (MAX_LIGHT_VAL as i32 - 1)
            && source_in_chunk_pos.y > -(MAX_LIGHT_VAL as i32)
            && source_in_chunk_pos.y < CHUNK_SIZE as i32 + (MAX_LIGHT_VAL as i32 - 1)
            && source_in_chunk_pos.z > -(MAX_LIGHT_VAL as i32)
            && source_in_chunk_pos.z < CHUNK_SIZE as i32 + (MAX_LIGHT_VAL as i32 - 1)
        {
            if source_strength > 1 && (source_red || source_blue || source_green) {
                let mut to_process = Vec::new();
                let mut processed = FxHashSet::default();

                {
                    let (chunk_pos, in_chunk_pos) = coordinate_in_surrounding_buffers_cube(source_in_chunk_pos);

                    if chunk_pos == Vector3::new(0, 0, 0) {
                        self.set(&in_chunk_pos, {
                            let mut light_val = self[&in_chunk_pos].clone();

                            if source_red {
                                light_val.red = source_strength;
                            }
                            if source_green {
                                light_val.green = source_strength;
                            }
                            if source_blue {
                                light_val.blue = source_strength;
                            }

                            light_val
                        });
                    }

                    for face in FaceDirection::iter() {
                        let dir = face.as_dir();
                        to_process.push((source_in_chunk_pos + dir, source_strength - 1))
                    }
                }

                while !to_process.is_empty() {
                    let mut to_process_next = Vec::new();

                    for (pos, strength) in to_process.into_iter() {
                        if !processed.contains(&pos) {
                            let (chunk_pos, in_chunk_pos) = coordinate_in_surrounding_buffers_cube(pos);
                            let block = &surrounding_blocks
                                [index_from_relative_pos_surrounding_cubes(&chunk_pos) as usize][&in_chunk_pos];

                            if !block.is_opaque() {
                                if chunk_pos == Vector3::new(0, 0, 0) {
                                    self.set(&in_chunk_pos, {
                                        let mut light_val = self[&in_chunk_pos].clone();

                                        if source_red {
                                            light_val.red = max(light_val.red, strength);
                                        }
                                        if source_green {
                                            light_val.green = max(light_val.green, strength);
                                        }
                                        if source_blue {
                                            light_val.blue = max(light_val.blue, strength);
                                        }

                                        light_val
                                    });
                                }

                                if strength > 1 {
                                    for face in FaceDirection::iter() {
                                        let dir = face.as_dir();
                                        to_process_next.push((pos + dir, strength - 1))
                                    }
                                }
                            }

                            processed.insert(pos);
                        }
                    }

                    to_process = to_process_next;
                }
            }
        }
    }

    fn spread_sunlight_from_source(
        &mut self,
        source_in_chunk_pos: Vector3<i32>,
        surrounding_blocks: &[Arc<BlockBuffer>; 27],
    ) {
        if source_in_chunk_pos.x > -(MAX_LIGHT_VAL as i32)
            && source_in_chunk_pos.x < CHUNK_SIZE as i32 + (MAX_LIGHT_VAL as i32 - 1)
            && source_in_chunk_pos.y > -(MAX_LIGHT_VAL as i32)
            && source_in_chunk_pos.y < CHUNK_SIZE as i32 + (MAX_LIGHT_VAL as i32 - 1)
            && source_in_chunk_pos.z > -(MAX_LIGHT_VAL as i32)
            && source_in_chunk_pos.z < CHUNK_SIZE as i32 + (MAX_LIGHT_VAL as i32 - 1)
        {
            let mut to_process = Vec::new();
            let mut processed = FxHashSet::default();

            {
                let (chunk_pos, in_chunk_pos) = coordinate_in_surrounding_buffers_cube(source_in_chunk_pos);

                if chunk_pos == Vector3::new(0, 0, 0) {
                    self.set(&in_chunk_pos, {
                        let mut light_val = self[&in_chunk_pos].clone();
                        light_val.sun = MAX_LIGHT_VAL;
                        light_val
                    });
                }

                for face in FaceDirection::iter() {
                    let dir = face.as_dir();
                    to_process.push((source_in_chunk_pos + dir, MAX_LIGHT_VAL - 1))
                }
            }

            while !to_process.is_empty() {
                let mut to_process_next = Vec::new();

                for (pos, strength) in to_process.into_iter() {
                    if !processed.contains(&pos) {
                        let (chunk_pos, in_chunk_pos) = coordinate_in_surrounding_buffers_cube(pos);
                        let block = &surrounding_blocks[index_from_relative_pos_surrounding_cubes(&chunk_pos) as usize]
                            [&in_chunk_pos];

                        if !block.is_sunlit() && !block.is_opaque() {
                            if chunk_pos == Vector3::new(0, 0, 0) {
                                self.set(&in_chunk_pos, {
                                    let mut light_val = self[&in_chunk_pos].clone();

                                    light_val.sun = max(light_val.sun, strength);

                                    light_val
                                });
                            }

                            if strength > 1 {
                                for face in FaceDirection::iter() {
                                    let dir = face.as_dir();
                                    to_process_next.push((pos + dir, strength - 1))
                                }
                            }
                        }

                        processed.insert(pos);
                    }
                }

                to_process = to_process_next;
            }
        }
    }
}

impl Index<&Vector3<i32>> for LightBuffer {
    type Output = LightVal;

    fn index(&self, index: &Vector3<i32>) -> &Self::Output {
        &self.buffer[ChunkShape::linearize([index.x as u32, index.y as u32, index.z as u32]) as usize]
    }
}
