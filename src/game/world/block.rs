use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    mem,
    ops::{Deref, Index},
    sync::Arc,
};

use block_mesh::ndshape::ConstShape;
use cfg_if::cfg_if;
use cgmath::{Vector2, Vector3};
use either::Either;
use rle_vec::RleVec;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

#[cfg(feature = "portable")]
use crate::RESOURCE_DIR;
use crate::{
    engine::face::FaceDirection,
    game::world::{coordinate_in_surrounding_buffers_cube, CacheUpdateActionKind, ChunkShape, LightSource, CHUNK_SIZE},
    misc::{
        index::{index_from_pos_2d, index_from_relative_pos_surrounding_cubes},
        loader::load_string_async,
    },
    RESOURCE_PATH,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextureID(u64);

impl Deref for TextureID {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for TextureID {
    fn from(value: &str) -> Self {
        TextureID({
            let mut hasher = DefaultHasher::new();
            value.to_string().hash(&mut hasher);
            hasher.finish()
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct BlockDescriptor {
    name: String,
    texture: Vec<String>,
    is_transparent: bool,
    is_solid: bool,
    is_lightsource: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Block {
    texture_id: Option<Either<TextureID, [TextureID; 3]>>,
    is_transparent: bool,
    is_solid: bool,
    light_source: Option<Box<LightSource>>,
    sunlit: bool,
}

impl From<BlockDescriptor> for Block {
    fn from(val: BlockDescriptor) -> Self {
        assert!(
            matches!(val.texture.len(), 0 | 1 | 3),
            "Attempting to create block `{:}` with invalid number of textures - {:}. Only 0, 1 or 3 textures are valid",
            val.name,
            val.texture.len()
        );

        let tmp = Block {
            texture_id:
                match val.texture.len() {
                    0 => None,
                    1 => Some(Either::Left(TextureID::from(val.texture[0].as_str()))),
                    3 => Some(Either::Right([TextureID::from(val.texture[0].as_str()), TextureID::from(val.texture[1].as_str()), TextureID::from(val.texture[2].as_str())])),
                    _ => panic!("Attempting to create block `{:}` with invalid number of textures - {:}. Only 0, 1 or 3 textures are valid", val.name, val.texture.len())
                },
            is_transparent: val.is_transparent,
            is_solid: val.is_solid,
            light_source: if val.is_lightsource {
                Some(Box::default())
            } else {
                None
            },
            sunlit: false
        };
        tmp
    }
}

impl Block {
    pub fn new(
        block_name: &str,
        block_manager: &BlockManager,
        light_source: Option<LightSource>,
        sunlit: bool,
    ) -> Self {
        let mut block = block_manager
            .get(block_name)
            .expect("Attempted to create block `{:block_name}` than has no entry file")
            .to_owned();

        if block.light_source.is_some() && light_source.is_some() {
            block.light_source = light_source.map(Box::new);
        }
        block.sunlit = sunlit;

        block
    }

    pub fn new_with_default(block_name: &str, block_manager: &BlockManager) -> Self {
        let mut block = block_manager
            .get(block_name)
            .expect("Attempted to create block `{:block_name}` than has no entry file")
            .to_owned();

        block.sunlit = block.is_transparent();

        block
    }

    pub const fn is_rendered(&self) -> bool {
        self.texture_id.is_some()
    }

    pub const fn is_transparent(&self) -> bool {
        self.is_transparent
    }

    pub fn is_transparent_mut(&mut self) -> &mut bool {
        &mut self.is_transparent
    }

    pub const fn is_opaque(&self) -> bool {
        self.is_rendered() && !self.is_transparent
    }

    pub const fn is_sunlit(&self) -> bool {
        self.sunlit && self.is_transparent()
    }

    pub const fn is_solid(&self) -> bool {
        self.is_solid
    }

    pub fn is_solid_mut(&mut self) -> &mut bool {
        &mut self.is_solid
    }

    pub fn light_source(&self) -> Option<&LightSource> {
        self.light_source.as_deref()
    }

    pub fn light_source_mut(&mut self) -> Option<&mut LightSource> {
        self.light_source.as_deref_mut()
    }

    pub fn set_light_source(&mut self, light_source: Option<LightSource>) {
        self.light_source = light_source.map(Box::new);
    }

    pub fn texture_id(&self) -> &Option<Either<TextureID, [TextureID; 3]>> {
        &self.texture_id
    }

    pub fn set_texture_id(&mut self, texture_id: Option<Either<TextureID, [TextureID; 3]>>) {
        self.texture_id = texture_id;
    }
}

#[derive(Clone, Debug)]
pub struct BlockManager {
    blocks: FxHashMap<String, (Block, Option<Either<String, [String; 3]>>)>,
    all_block_names: Vec<String>,
    all_rendered_block_names: Vec<String>,
    all_texture_names: Vec<String>,
    texture_id_to_name: FxHashMap<TextureID, String>,
}

impl BlockManager {
    pub async fn new() -> Self {
        use std::fs;

        let mut out = Self {
            blocks: FxHashMap::default(),
            all_block_names: Vec::new(),
            all_rendered_block_names: Vec::new(),
            all_texture_names: Vec::new(),
            texture_id_to_name: FxHashMap::default(),
        };

        let paths: Vec<String>;
        cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                // Working with the web is very fun since you cant list all files in a directory apparently?

                paths = {
                    let mut out = Vec::new();
                    for entry in RESOURCE_DIR.get_dir("block").unwrap().entries() {
                        if let include_dir::DirEntry::File(file) = entry {
                            if let Some(file_name) = entry.path().file_name() {
                                if let Some(name) = file_name.to_str() {
                                    out.push(name.to_string());
                                }
                            }
                        }
                    }
                    out
                };
            } else {
                paths = fs::read_dir(RESOURCE_PATH.join("block"))
                    .unwrap()
                    .map(|dir| dir.unwrap().file_name().to_string_lossy().to_string())
                    .collect();
            }
        }

        for block_file_name in paths {
            let path = RESOURCE_PATH.join("block").join(&block_file_name);
            match load_string_async(path).await {
                Ok(block_string) => match serde_yaml::from_str::<BlockDescriptor>(block_string.as_str()) {
                    Ok(block_descriptor) => {
                        out.blocks
                            .insert(block_descriptor.name.clone(), (block_descriptor.clone().into(), match block_descriptor.texture.len() {
                    0 => None,
                    1 => Some(Either::Left(block_descriptor.texture[0].clone())),
                    3 => Some(Either::Right([block_descriptor.texture[0].clone(), block_descriptor.texture[1].clone(), block_descriptor.texture[2].clone()])),
                    _ => panic!("Attempting to create block `{:}` with invalid number of textures - {:}. Only 0, 1 or 3 textures are valid", block_descriptor.name, block_descriptor.texture.len())
                }));
                    }
                    Err(e) => log::error!("Failed parsing `{block_file_name:}` - {e:?}"),
                },
                Err(e) => {
                    log::error!("Attempted to load block `{block_file_name:}` without a block file - {e:?}");
                }
            }
        }

        out.all_texture_names = {
            let mut tmp = out
                .blocks
                .iter()
                .map(|(_, (_, texture))| {
                    if let Some(textures) = texture {
                        match textures {
                            Either::Left(texture) => vec![texture.clone()],
                            Either::Right([texture_top, texture_side, texture_bottom]) => {
                                vec![texture_top.clone(), texture_side.clone(), texture_bottom.clone()]
                            }
                        }
                    } else {
                        vec![]
                    }
                })
                .flatten()
                .collect::<Vec<_>>();

            let to_extend: Vec<String>;
            cfg_if! {
                if #[cfg(target_arch = "wasm32")] {
                    to_extend = {
                        let mut out = Vec::new();
                        for entry in RESOURCE_DIR.get_dir("texture").unwrap().entries() {
                            if let include_dir::DirEntry::File(file) = entry {
                                if let Some(file_name) = entry.path().file_name() {
                                    if let Some(name) = file_name.to_str() {
                                        out.push(name.to_string());
                                    }
                                }
                            }
                        }
                        out
                    };
                } else {
                    to_extend = fs::read_dir(RESOURCE_PATH.join("texture")).unwrap().filter_map(|dir| {
                        let dir_entry = dir.unwrap();
                        if dir_entry.file_type().unwrap().is_file() {
                            dir_entry
                                .file_name()
                                .to_string_lossy()
                                .split(".")
                                .next()
                                .map(|file_name| file_name.to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
                }
            }

            tmp.extend(to_extend);

            tmp.sort();
            tmp.dedup();

            tmp
        };

        out.all_block_names = {
            let mut tmp = out
                .blocks
                .iter()
                .map(|(block_name, (_, _))| block_name.to_owned())
                .collect::<Vec<_>>();

            tmp.sort();
            tmp.dedup();

            tmp
        };

        out.all_rendered_block_names = {
            let mut tmp = out
                .blocks
                .iter()
                .map(|(block_name, (_, _))| block_name.to_owned())
                .filter(|block_name| out.blocks[block_name].0.is_rendered())
                .collect::<Vec<_>>();

            tmp.sort();
            tmp.dedup();

            tmp
        };

        out.texture_id_to_name = out
            .all_texture_names
            .iter()
            .map(|texture_name| (TextureID::from(texture_name.as_str()), texture_name.to_owned()))
            .collect();

        out
    }

    pub fn get(&self, k: &str) -> Option<&Block> {
        self.blocks.get(k).map(|(block, _)| block)
    }

    #[allow(dead_code)]
    pub fn all_block_names(&self) -> &[String] {
        self.all_block_names.as_ref()
    }

    #[allow(dead_code)]
    pub fn all_rendered_block_names(&self) -> &[String] {
        self.all_rendered_block_names.as_ref()
    }

    #[allow(dead_code)]
    pub fn all_texture_names(&self) -> &[String] {
        self.all_texture_names.as_ref()
    }

    pub fn get_texture_name(&self, k: &TextureID) -> Option<&String> {
        self.texture_id_to_name.get(k)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash)]
pub struct BlockBuffer {
    buffer: RleVec<Block>,
    collum_contains_opaque_blocks: Vec<bool>,
    light_source_cache: Option<LightPosCache<0>>,
    sunlight_source_cache: Option<LightPosCache<1>>,
    to_update_cache_later: Vec<(Vector3<i32>, CacheUpdateActionKind)>,
}

impl BlockBuffer {
    pub fn new(blocks: Vec<Block>) -> Self {
        debug_assert!(blocks.len() == (CHUNK_SIZE as usize).pow(3));

        Self {
            collum_contains_opaque_blocks: {
                fn check_for_visible_blocks_in_collum(blocks: &Vec<Block>, collum: &Vector2<i32>) -> bool {
                    for y in 0..CHUNK_SIZE as usize {
                        let index = Vector3::new(collum.x, y as i32, collum.y);

                        if blocks[ChunkShape::linearize([index.x as u32, index.y as u32, index.z as u32]) as usize]
                            .is_opaque()
                        {
                            return true;
                        }
                    }

                    false
                }

                let mut out = vec![false; CHUNK_SIZE.pow(2) as usize];

                for x in 0..CHUNK_SIZE as i32 {
                    for z in 0..CHUNK_SIZE as i32 {
                        let index = Vector2::new(x, z);

                        out[index_from_pos_2d(&index) as usize] = check_for_visible_blocks_in_collum(&blocks, &index)
                    }
                }

                out
            },
            buffer: RleVec::from_iter(blocks.into_iter()),
            light_source_cache: None,
            sunlight_source_cache: None,
            to_update_cache_later: Vec::new(),
        }
    }

    pub fn set(&mut self, in_chunk_pos: &Vector3<i32>, block: Block) {
        self.buffer.set(
            ChunkShape::linearize([in_chunk_pos.x as u32, in_chunk_pos.y as u32, in_chunk_pos.z as u32]) as usize,
            block.clone(),
        );

        let collum = Vector2::new(in_chunk_pos.x, in_chunk_pos.z);
        self.update_visible_blocks_in_collum(&collum);

        for x in -1..=1 as i32 {
            for y in -1..=1 as i32 {
                for z in -1..=1 as i32 {
                    let neighbour_pos = in_chunk_pos + Vector3::new(x, y, z);
                    let (neighbour_chunk_pos, neighbour_in_chunk_pos) =
                        coordinate_in_surrounding_buffers_cube(neighbour_pos);

                    if neighbour_chunk_pos == Vector3::new(0, 0, 0) {
                        let neighbor_block = self[&neighbour_in_chunk_pos].clone();

                        self.to_update_cache_later.push((
                            neighbour_pos,
                            if neighbor_block.light_source().is_some() {
                                CacheUpdateActionKind::AddLightSource
                            } else {
                                CacheUpdateActionKind::RemoveLightSource
                            },
                        ));
                        if neighbor_block.is_transparent() {
                            self.to_update_cache_later
                                .push((neighbour_pos, CacheUpdateActionKind::SunlightSource))
                        };
                    }
                }
            }
        }
    }

    pub fn do_cache_updates(&mut self, surrounding_blocks: &[Arc<BlockBuffer>; 27]) {
        let to_update_cache_later = {
            let mut to_update_cache_later = Vec::new();
            mem::swap(&mut to_update_cache_later, &mut self.to_update_cache_later);
            to_update_cache_later
        };

        for to_update in to_update_cache_later {
            let (block_in_chunk_pos, kind) = to_update;
            match kind {
                CacheUpdateActionKind::SunlightSource => {
                    let block = self[&block_in_chunk_pos].clone();

                    if let Some(sunlight_source_cache) = &mut self.sunlight_source_cache {
                        sunlight_source_cache.remove(block_in_chunk_pos, &surrounding_blocks);

                        if block.is_sunlit() {
                            sunlight_source_cache.insert(block_in_chunk_pos, &surrounding_blocks);
                        }
                    }
                }
                CacheUpdateActionKind::AddLightSource => {
                    if let Some(light_source_cache) = &mut self.light_source_cache {
                        light_source_cache.insert(block_in_chunk_pos, &surrounding_blocks);
                    }
                }
                CacheUpdateActionKind::RemoveLightSource => {
                    if let Some(light_source_cache) = &mut self.light_source_cache {
                        light_source_cache.remove(block_in_chunk_pos, &surrounding_blocks);
                    }
                }
            }
        }
    }

    pub fn contains_rendered_blocks(&self) -> bool {
        let runs = self.buffer.runs();
        if runs.len() == 1 {
            return self.buffer[0].is_rendered();
        } else {
            for run in runs {
                if run.value.is_rendered() {
                    return true;
                }
            }
        }

        false
    }

    pub fn contains_collum_opaque_blocks(&self, collum: &Vector2<i32>) -> bool {
        self.collum_contains_opaque_blocks[index_from_pos_2d(&collum) as usize]
    }

    fn update_visible_blocks_in_collum(&mut self, collum: &Vector2<i32>) {
        self.collum_contains_opaque_blocks[index_from_pos_2d(&collum) as usize] = (0..CHUNK_SIZE as i32)
            .into_iter()
            .any(|y| self[&Vector3::new(collum.x, y, collum.y)].is_opaque())
    }

    pub fn update_sunlight_in_collum(&mut self, collum: &Vector2<i32>, highest_block_in_chunk_sees_sky: bool) {
        let mut found_visible = !highest_block_in_chunk_sees_sky;

        for y in (0..CHUNK_SIZE as i32).rev() {
            let block_in_chunk_pos = Vector3::new(collum.x, y, collum.y);
            let block = &self[&block_in_chunk_pos];

            if !found_visible && block.is_opaque() {
                found_visible = true
            }

            if block.is_transparent() {
                self.buffer.set(
                    ChunkShape::linearize([
                        block_in_chunk_pos.x as u32,
                        block_in_chunk_pos.y as u32,
                        block_in_chunk_pos.z as u32,
                    ]) as usize,
                    {
                        let mut block = block.clone();
                        block.sunlit = !found_visible;
                        block
                    },
                );
            } else {
                self.buffer.set(
                    ChunkShape::linearize([
                        block_in_chunk_pos.x as u32,
                        block_in_chunk_pos.y as u32,
                        block_in_chunk_pos.z as u32,
                    ]) as usize,
                    {
                        let mut block = block.clone();
                        block.sunlit = false;
                        block
                    },
                );
            }
            self.to_update_cache_later
                .push((block_in_chunk_pos, CacheUpdateActionKind::SunlightSource));
        }
    }

    pub fn light_sources(&self) -> Option<&LightPosCache<0>> {
        self.light_source_cache.as_ref()
    }

    pub fn sunlight_sources(&self) -> Option<&LightPosCache<1>> {
        self.sunlight_source_cache.as_ref()
    }

    pub fn set_light_source_caches(
        &mut self,
        light_source_cache: LightPosCache<0>,
        sunlight_source_cache: LightPosCache<1>,
    ) {
        self.light_source_cache = Some(light_source_cache);
        self.sunlight_source_cache = Some(sunlight_source_cache);
    }
}

impl Index<&Vector3<i32>> for BlockBuffer {
    type Output = Block;

    fn index(&self, index: &Vector3<i32>) -> &Self::Output {
        &self.buffer[ChunkShape::linearize([index.x as u32, index.y as u32, index.z as u32]) as usize]
    }
}

// Kind == 0 for LightSource, Kind == 1 for SunlightSource
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightPosCache<const KIND: u8> {
    cache: FxHashSet<Vector3<i32>>,
}

impl<const KIND: u8> LightPosCache<KIND> {
    pub fn new(surrounding_blocks: &[Arc<BlockBuffer>; 27]) -> Self {
        let mut self_temp = Self {
            cache: FxHashSet::default(),
        };

        for x in 0..CHUNK_SIZE as i32 {
            for y in 0..CHUNK_SIZE as i32 {
                for z in 0..CHUNK_SIZE as i32 {
                    let index = Vector3::new(x, y, z);

                    if match KIND {
                        0 => surrounding_blocks
                            [index_from_relative_pos_surrounding_cubes(&Vector3::new(0, 0, 0)) as usize][&index]
                            .light_source()
                            .is_some(),
                        1 => surrounding_blocks
                            [index_from_relative_pos_surrounding_cubes(&Vector3::new(0, 0, 0)) as usize][&index]
                            .is_sunlit(),
                        _ => unreachable!("LightPosCache with Kind different than 0 or 1"),
                    } {
                        self_temp.insert(index, surrounding_blocks);
                    }
                }
            }
        }

        self_temp
    }

    pub fn cache(&self) -> &FxHashSet<Vector3<i32>> {
        &self.cache
    }

    fn insert(&mut self, in_chunk_pos: Vector3<i32>, surrounding_blocks: &[Arc<BlockBuffer>; 27]) {
        if in_chunk_pos.x >= 0
            && in_chunk_pos.x < CHUNK_SIZE as i32
            && in_chunk_pos.y >= 0
            && in_chunk_pos.y < CHUNK_SIZE as i32
            && in_chunk_pos.z >= 0
            && in_chunk_pos.z < CHUNK_SIZE as i32
        {
            let (mut found_opaque, mut found_transparent_unsunlit) = (false, false);

            if KIND == 1 {
                'outer: for x in -1..=1 as i32 {
                    for y in -1..=1 as i32 {
                        for z in -1..=1 as i32 {
                            if x.abs() + y.abs() + z.abs() >= 2 {
                                let neighbour_pos = in_chunk_pos + Vector3::new(x, y, z);
                                let (neighbour_chunk_pos, neighbour_in_chunk_pos) =
                                    coordinate_in_surrounding_buffers_cube(neighbour_pos);
                                let neighbor_block = &surrounding_blocks
                                    [index_from_relative_pos_surrounding_cubes(&neighbour_chunk_pos) as usize]
                                    [&neighbour_in_chunk_pos];

                                if neighbor_block.is_opaque() {
                                    found_opaque = true;
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
            }

            for face in FaceDirection::iter() {
                let neighbour_pos = in_chunk_pos + face.as_dir();
                let (neighbour_chunk_pos, neighbour_in_chunk_pos) =
                    coordinate_in_surrounding_buffers_cube(neighbour_pos);
                let neighbor_block = &surrounding_blocks
                    [index_from_relative_pos_surrounding_cubes(&neighbour_chunk_pos) as usize][&neighbour_in_chunk_pos];

                if KIND == 1 {
                    if neighbor_block.is_opaque() {
                        found_opaque = true
                    }
                    if neighbor_block.is_transparent() && !neighbor_block.is_sunlit() {
                        found_transparent_unsunlit = true
                    }
                }

                if match KIND {
                    0 => neighbor_block.is_transparent(),
                    1 => found_opaque && found_transparent_unsunlit,
                    _ => unreachable!("LightPosCache with Kind different than 0 or 1"),
                } {
                    self.cache.insert(in_chunk_pos);
                    return;
                }
            }
        }
    }

    fn remove(&mut self, in_chunk_pos: Vector3<i32>, surrounding_blocks: &[Arc<BlockBuffer>; 27]) {
        for face in FaceDirection::iter() {
            let neighbour_pos = in_chunk_pos + face.as_dir();
            let (neighbour_chunk_pos, neighbour_in_chunk_pos) = coordinate_in_surrounding_buffers_cube(neighbour_pos);
            let neighbor_block = &surrounding_blocks
                [index_from_relative_pos_surrounding_cubes(&neighbour_chunk_pos) as usize][&neighbour_in_chunk_pos];

            if match KIND {
                0 => neighbor_block.light_source().is_some(),
                1 => neighbor_block.is_sunlit(),
                _ => unreachable!("LightPosCache with Kind different than 0 or 1"),
            } {
                self.insert(neighbour_pos, surrounding_blocks)
            }
        }

        self.cache.remove(&in_chunk_pos);
    }
}

impl<const KIND: u8> Hash for LightPosCache<KIND> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        KIND.hash(state);
        self.cache.len().hash(state);

        for (idx, value) in self.cache.iter().enumerate() {
            if idx < 4 {
                value.hash(state);
            } else {
                break;
            }
        }
    }
}
