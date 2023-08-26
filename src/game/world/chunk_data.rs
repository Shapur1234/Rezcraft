use std::sync::Arc;

use cgmath::{Vector2, Vector3};
use serde::{Deserialize, Serialize};

use crate::game::world::{Block, BlockBuffer, LightBuffer, LightPosCache, CHUNK_SIZE};

#[derive(Clone, Debug, Serialize, Deserialize, Hash)]
pub enum CacheUpdateActionKind {
    SunlightSource,
    AddLightSource,
    RemoveLightSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash)]
pub struct ChunkData {
    blocks: Arc<BlockBuffer>,
    lights: Option<Arc<LightBuffer>>,
}

impl ChunkData {
    pub fn new(blocks: BlockBuffer) -> Self {
        Self {
            blocks: Arc::new(blocks),
            lights: None,
        }
    }

    pub fn set_block(&mut self, in_chunk_pos: &Vector3<i32>, block: Block) -> (bool, bool) {
        let collum = Vector2::new(in_chunk_pos.x, in_chunk_pos.z);
        let contains_collum_opaque_block_old = self.blocks.contains_collum_opaque_blocks(&collum);

        let mut blocks = (*self.blocks).clone();
        blocks.set(in_chunk_pos, block);
        self.blocks = Arc::new(blocks);

        let contains_collum_opaque_block_new = self.blocks.contains_collum_opaque_blocks(&collum);

        (contains_collum_opaque_block_old, contains_collum_opaque_block_new)
    }

    pub fn blocks(&self) -> Arc<BlockBuffer> {
        self.blocks.clone()
    }

    pub fn lights(&self) -> Option<Arc<LightBuffer>> {
        self.lights.as_ref().map(|lights| lights.clone())
    }

    pub fn set_lights(&mut self, lights: LightBuffer) {
        self.lights = Some(Arc::new(lights));
    }

    pub fn set_light_source_caches(
        &mut self,
        light_source_cache: LightPosCache<0>,
        sunlight_source_cache: LightPosCache<1>,
    ) {
        let mut blocks = (*self.blocks).clone();

        blocks.set_light_source_caches(light_source_cache, sunlight_source_cache);

        self.blocks = Arc::new(blocks);
    }

    pub fn update_sunlight_in_collum(&mut self, collum: &Vector2<u32>, highest_block_in_chunk_sees_sky: bool) {
        let collum = Vector2::new(collum.x as i32, collum.y as i32);

        let mut blocks = (*self.blocks).clone();
        blocks.update_sunlight_in_collum(&collum, highest_block_in_chunk_sees_sky);
        self.blocks = Arc::new(blocks);
    }

    pub fn refresh_sunlight_in_collum(&mut self, collum: &Vector2<u32>) {
        let collum = Vector2::new(collum.x as i32, collum.y as i32);

        let mut blocks = (*self.blocks).clone();
        blocks.update_sunlight_in_collum(
            &collum,
            blocks[&Vector3::new(collum.x, CHUNK_SIZE as i32 - 1, collum.y)].is_sunlit(),
        );
        self.blocks = Arc::new(blocks);
    }

    pub fn do_cache_updates(&mut self, surrounding_blocks: &[Arc<BlockBuffer>; 27]) {
        let mut blocks = (*self.blocks).clone();
        blocks.do_cache_updates(surrounding_blocks);

        self.blocks = Arc::new(blocks);
    }
}
