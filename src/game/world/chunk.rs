use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

use block_mesh::ndshape::ConstShape3u32;
use cgmath::{Vector2, Vector3};
use either::Either;
use serde::{Deserialize, Serialize};

use crate::game::world::{Block, BlockBuffer, ChunkData, ChunkMesh, ChunkMeshRaw, LightBuffer, LightPosCache};

pub const CHUNK_SIZE: u32 = 32;
pub const CHUNK_SIZE_VEC: Vector3<i32> = Vector3::new(CHUNK_SIZE as i32, CHUNK_SIZE as i32, CHUNK_SIZE as i32);
pub const CHUNK_SIZE_MESHING: u32 = CHUNK_SIZE + 2;

pub type ChunkShape = ConstShape3u32<CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE>;

#[derive(Serialize, Deserialize)]
pub struct Chunk {
    data: ChunkData,
    #[serde(skip)]
    light_pos_cache_requested_for_state: Option<u64>,
    #[serde(skip)]
    lights_requested_for_state: Option<u64>,
    lights_up_to_date: bool,
    #[serde(skip)]
    mesh: Option<Either<(ChunkMesh, ChunkMesh), (ChunkMeshRaw, ChunkMeshRaw)>>,
    #[serde(skip)]
    mesh_requested_for_state: Option<u64>,
    mesh_up_to_date: bool,
}

impl Chunk {
    pub fn new(blocks: BlockBuffer) -> Self {
        Self {
            data: ChunkData::new(blocks),
            light_pos_cache_requested_for_state: None,
            lights_requested_for_state: None,
            lights_up_to_date: false,
            mesh: None,
            mesh_requested_for_state: None,
            mesh_up_to_date: false,
        }
    }

    // --------------------------------

    pub fn blocks(&self) -> Arc<BlockBuffer> {
        self.data.blocks()
    }

    pub fn set_block(&mut self, in_chunk_pos: &Vector3<i32>, block: Block) -> (bool, bool) {
        self.data.set_block(in_chunk_pos, block)
    }

    // --------------------------------

    pub fn mesh(&mut self, device: &wgpu::Device) -> Option<&(ChunkMesh, ChunkMesh)> {
        if self.mesh.is_some() {
            if self.mesh.as_ref().unwrap().is_right() {
                let mesh_raw = self.mesh.take().unwrap().right().unwrap();
                self.mesh = Some(Either::Left((
                    ChunkMesh::new(mesh_raw.0, device),
                    ChunkMesh::new(mesh_raw.1, device),
                )))
            }
            Some(self.mesh.as_ref().unwrap().as_ref().left().unwrap())
        } else {
            None
        }
    }

    pub fn set_mesh(&mut self, mesh_raw: (ChunkMeshRaw, ChunkMeshRaw)) {
        self.mesh_up_to_date = true;
        self.mesh = Some(Either::Right(mesh_raw))
    }

    pub fn mesh_requested(&self) -> bool {
        if let Some(hash) = self.mesh_requested_for_state {
            hash == self.state_hash()
        } else {
            false
        }
    }

    pub fn set_mesh_requested(&mut self, val: bool) {
        if val {
            self.mesh_requested_for_state = Some(self.state_hash())
        } else {
            self.mesh_requested_for_state = None
        }
    }

    pub fn mesh_up_to_date(&self) -> bool {
        self.mesh_up_to_date
    }

    pub fn set_mesh_outdated(&mut self) {
        self.mesh_up_to_date = false
    }

    // --------------------------------

    pub fn set_light_source_caches(
        &mut self,
        light_source_cache: LightPosCache<0>,
        sunlight_source_cache: LightPosCache<1>,
    ) {
        self.data
            .set_light_source_caches(light_source_cache, sunlight_source_cache)
    }

    pub fn light_pos_cache_requested(&self) -> bool {
        if let Some(hash) = self.light_pos_cache_requested_for_state {
            hash == self.state_hash()
        } else {
            false
        }
    }

    pub fn set_light_pos_cache_requested(&mut self, val: bool) {
        if val {
            self.light_pos_cache_requested_for_state = Some(self.state_hash())
        } else {
            self.light_pos_cache_requested_for_state = None
        }
    }

    // --------------------------------

    pub fn lights(&self) -> Option<Arc<LightBuffer>> {
        self.data.lights()
    }

    pub fn set_lights(&mut self, lights: LightBuffer) {
        self.lights_up_to_date = true;
        self.data.set_lights(lights)
    }

    pub fn lights_requested(&self) -> bool {
        if let Some(hash) = self.lights_requested_for_state {
            hash == self.state_hash()
        } else {
            false
        }
    }

    pub fn set_lights_requested(&mut self, val: bool) {
        if val {
            self.lights_requested_for_state = Some(self.state_hash())
        } else {
            self.lights_requested_for_state = None
        }
    }

    pub fn lights_up_to_date(&self) -> bool {
        self.lights_up_to_date
    }

    pub fn set_lights_outdated(&mut self) {
        self.lights_up_to_date = false
    }

    // --------------------------------

    pub fn update_sunlight_in_collum(&mut self, collum: &Vector2<u32>, highest_block_in_chunk_sees_sky: bool) {
        self.data
            .update_sunlight_in_collum(collum, highest_block_in_chunk_sees_sky)
    }

    pub fn refresh_sunlight_in_collum(&mut self, collum: &Vector2<u32>) {
        self.data.refresh_sunlight_in_collum(collum)
    }

    pub fn do_cache_updates(&mut self, surrounding_blocks: &[Arc<BlockBuffer>; 27]) {
        self.data.do_cache_updates(surrounding_blocks)
    }

    // --------------------------------

    pub fn state_hash(&self) -> u64 {
        let mut hasher = rustc_hash::FxHasher::default();

        self.data.hash(&mut hasher);

        hasher.finish()
    }
}

pub fn coordinate_in_surrounding_buffers(in_chunk_pos: Vector3<i32>) -> Option<(Vector3<i32>, Vector3<i32>)> {
    let chunk_pos = Vector3::<i32>::new(
        if in_chunk_pos.x < 0 {
            -1
        } else if in_chunk_pos.x >= CHUNK_SIZE as i32 {
            1
        } else {
            0
        },
        if in_chunk_pos.y < 0 {
            -1
        } else if in_chunk_pos.y >= CHUNK_SIZE as i32 {
            1
        } else {
            0
        },
        if in_chunk_pos.z < 0 {
            -1
        } else if in_chunk_pos.z >= CHUNK_SIZE as i32 {
            1
        } else {
            0
        },
    );

    {
        if (chunk_pos.x.abs() + chunk_pos.y.abs() + chunk_pos.z.abs()) > 1 {
            None
        } else {
            Some((
                chunk_pos,
                (in_chunk_pos % CHUNK_SIZE as i32 + CHUNK_SIZE_VEC) % CHUNK_SIZE as i32,
            ))
        }
    }
}

pub fn coordinate_in_surrounding_buffers_cube(in_chunk_pos: Vector3<i32>) -> (Vector3<i32>, Vector3<i32>) {
    let chunk_pos = Vector3::<i32>::new(
        if in_chunk_pos.x < 0 {
            -1
        } else if in_chunk_pos.x >= CHUNK_SIZE as i32 {
            1
        } else {
            0
        },
        if in_chunk_pos.y < 0 {
            -1
        } else if in_chunk_pos.y >= CHUNK_SIZE as i32 {
            1
        } else {
            0
        },
        if in_chunk_pos.z < 0 {
            -1
        } else if in_chunk_pos.z >= CHUNK_SIZE as i32 {
            1
        } else {
            0
        },
    );

    (
        chunk_pos,
        (in_chunk_pos % CHUNK_SIZE as i32 + CHUNK_SIZE_VEC) % CHUNK_SIZE as i32,
    )
}
