use std::{
    cell::RefCell,
    mem::{self, MaybeUninit},
    num::NonZeroI32,
    pin::Pin,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use block_mesh::GreedyQuadsBuffer;
use cfg_if::cfg_if;
use cgmath::{MetricSpace, Vector2, Vector3};
use either::Either;
use futures_channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use instant::Duration;
use ref_thread_local::{ref_thread_local, RefThreadLocal};
use rustc_hash::{FxHashMap, FxHashSet};
use strum::IntoEnumIterator;

#[cfg(feature = "save_system")]
use crate::misc::save_helper::save_many;
use crate::{
    engine::{face::FaceDirection, TextureAtlas},
    game::{
        world::{
            coordinate_in_surrounding_buffers_cube, Block, BlockBuffer, BlockManager, Chunk, ChunkMesh, LightBuffer,
            LightPosCache, LightVal, MeshBuffer, TerrainGenerator, Voxel, CHUNK_SIZE, CHUNK_SIZE_MESHING,
            MAX_LIGHT_VAL,
        },
        Camera,
    },
    misc::{
        index::{
            index_from_relative_pos_surrounding, index_from_relative_pos_surrounding_cubes,
            relative_pos_surrounding_cubes_from_index,
        },
        pos::{add_non_zero_i32_vector3, add_to_non_zero_i32, Pos},
    },
};

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use wasm_thread as thread;
    } else {
        use std::thread;
    }
}

#[cfg(feature = "rayon")]
use rayon::prelude::*;

const THREAD_SLEEP_TIME: u64 = 10;

struct BlocksThreadRequest {
    pos: Vector3<NonZeroI32>,
    current_save_name: String,
}

impl BlocksThreadRequest {
    fn new(pos: Vector3<NonZeroI32>, current_save_name: String) -> Self {
        Self { pos, current_save_name }
    }
}

struct BlocksThreadReturn {
    pos: Vector3<NonZeroI32>,
    blocks: BlockBuffer,
}

impl BlocksThreadReturn {
    fn new(pos: Vector3<NonZeroI32>, blocks: BlockBuffer) -> Self {
        Self { pos, blocks }
    }
}

struct LightThreadRequest {
    pos: Vector3<NonZeroI32>,
    surrounding_blocks: [Arc<BlockBuffer>; 27],
    for_state: u64,
}

impl LightThreadRequest {
    fn new(pos: Vector3<NonZeroI32>, surrounding_blocks: [Arc<BlockBuffer>; 27], for_state: u64) -> Self {
        Self {
            pos,
            surrounding_blocks,
            for_state,
        }
    }
}

struct LightThreadReturn {
    pos: Vector3<NonZeroI32>,
    lights: Option<LightBuffer>,
    for_state: u64,
}

impl LightThreadReturn {
    fn new(pos: Vector3<NonZeroI32>, lights: Option<LightBuffer>, for_state: u64) -> Self {
        Self { pos, lights, for_state }
    }
}

struct LightPosCacheThreadRequest {
    pos: Vector3<NonZeroI32>,
    surrounding_blocks: [Arc<BlockBuffer>; 27],
    for_state: u64,
}

impl LightPosCacheThreadRequest {
    fn new(pos: Vector3<NonZeroI32>, surrounding_blocks: [Arc<BlockBuffer>; 27], for_state: u64) -> Self {
        Self {
            pos,
            surrounding_blocks,
            for_state,
        }
    }
}

struct LightPosCacheThreadReturn {
    pos: Vector3<NonZeroI32>,
    light_source_cache: LightPosCache<0>,
    sunlight_source_cache: LightPosCache<1>,
    for_state: u64,
}

impl LightPosCacheThreadReturn {
    fn new(
        pos: Vector3<NonZeroI32>,
        light_source_cache: LightPosCache<0>,
        sunlight_source_cache: LightPosCache<1>,
        for_state: u64,
    ) -> Self {
        Self {
            pos,
            light_source_cache,
            sunlight_source_cache,
            for_state,
        }
    }
}

struct MeshThreadRequest {
    pos: Vector3<NonZeroI32>,
    surrounding_blocks: [Arc<BlockBuffer>; 7],
    surrounding_lights: [Arc<LightBuffer>; 7],
    for_state: u64,
}

impl MeshThreadRequest {
    fn new(
        pos: Vector3<NonZeroI32>,
        surrounding_blocks: [Arc<BlockBuffer>; 7],
        surrounding_lights: [Arc<LightBuffer>; 7],
        for_state: u64,
    ) -> Self {
        Self {
            pos,
            surrounding_blocks,
            surrounding_lights,
            for_state,
        }
    }
}

struct MeshThreadReturn {
    pos: Vector3<NonZeroI32>,
    mesh: MeshBuffer,
    for_state: u64,
}

impl MeshThreadReturn {
    fn new(pos: Vector3<NonZeroI32>, mesh: MeshBuffer, for_state: u64) -> Self {
        Self { pos, mesh, for_state }
    }
}

#[cfg(feature = "save_system")]
struct SaveChunkRequest {
    current_save_name: String,
    chunks: Vec<(String, BlockBuffer)>,
}

#[cfg(feature = "save_system")]
impl SaveChunkRequest {
    fn new(current_save_name: String, chunks: Vec<(String, BlockBuffer)>) -> Self {
        Self {
            current_save_name,
            chunks,
        }
    }
}

pub struct Terrain {
    chunks: FxHashMap<Vector3<NonZeroI32>, Pin<Box<Chunk>>>,
    requested_chunks_list: FxHashSet<Vector3<NonZeroI32>>,
    mesh_reciever: UnboundedReceiver<MeshThreadReturn>,
    mesh_sender: UnboundedSender<MeshThreadRequest>,
    blocks_reciever: UnboundedReceiver<BlocksThreadReturn>,
    blocks_sender: UnboundedSender<BlocksThreadRequest>,
    light_pos_cache_reciever: UnboundedReceiver<LightPosCacheThreadReturn>,
    light_pos_cache_sender: UnboundedSender<LightPosCacheThreadRequest>,
    light_reciever: UnboundedReceiver<LightThreadReturn>,
    light_sender: UnboundedSender<LightThreadRequest>,
    #[cfg(feature = "save_system")]
    chunk_save_sender: UnboundedSender<SaveChunkRequest>,
    #[cfg(feature = "save_system")]
    current_save_name: String,
    transparency: bool,
    texture_atlas: TextureAtlas,
    loading_chunks: u32,
    saving_chunks: Arc<AtomicU32>,
    block_manager: BlockManager,
}

impl Terrain {
    pub fn new(transparency: bool, texture_atlas: &TextureAtlas, seed: u32, block_manager: BlockManager) -> Self {
        let (main_mesh_sender, mut thread_mesh_reciever) = unbounded::<MeshThreadRequest>();
        let (thread_mesh_sender, main_mesh_reciever) = unbounded::<MeshThreadReturn>();

        let atlas_clone = texture_atlas.clone_without_image();
        thread::Builder::new()
            .name("Mesh generator".to_string())
            .spawn(move || {
                const BUFFER_SIZE: usize = CHUNK_SIZE_MESHING.pow(3) as usize;
                thread_local! {
                    static REUSED_BUFFER: RefCell<(GreedyQuadsBuffer, Vec<Voxel>)> = RefCell::new((GreedyQuadsBuffer::new(BUFFER_SIZE), Vec::from_iter(std::iter::repeat(Voxel::default()).take(BUFFER_SIZE))));
                }

                loop {
                    #[allow(unused_mut)]
                    let mut recieved_messages = {
                        #[cfg(feature = "rayon")] 
                        {
                            collect_messages(&mut thread_mesh_reciever).into_par_iter()
                        }
                        #[cfg(not(feature = "rayon"))]
                        {
                            collect_messages(&mut thread_mesh_reciever).into_iter()
                        }
                    };

                    if recieved_messages.len() == 0 {
                        thread::sleep(Duration::from_millis(THREAD_SLEEP_TIME));
                    } else if recieved_messages.try_for_each(|recieved| {
                        let mesh = REUSED_BUFFER.with(|buffer| MeshBuffer::new(&recieved.pos, recieved.surrounding_blocks, recieved.surrounding_lights, &atlas_clone, transparency, &mut buffer.borrow_mut()));

                        thread_mesh_sender
                            .clone()
                            .unbounded_send(MeshThreadReturn::new(recieved.pos, mesh, recieved.for_state))
                    }).is_err() {
                        break;
                    }
                }
            }
        ).unwrap();

        let (main_lightpos_cache_sender, mut thread_lightpos_cache_reciever) =
            unbounded::<LightPosCacheThreadRequest>();
        let (thread_lightpos_cache_sender, main_lightpos_cache_reciever) = unbounded::<LightPosCacheThreadReturn>();

        thread::Builder::new()
            .name("Lightpos cache generator".to_string())
            .spawn(move || loop {
                #[allow(unused_mut)]
                let mut recieved_messages = {
                    #[cfg(feature = "rayon")]
                    {
                        collect_messages(&mut thread_lightpos_cache_reciever).into_par_iter()
                    }
                    #[cfg(not(feature = "rayon"))]
                    {
                        collect_messages(&mut thread_lightpos_cache_reciever).into_iter()
                    }
                };

                if recieved_messages.len() == 0 {
                    thread::sleep(Duration::from_millis(THREAD_SLEEP_TIME));
                } else if recieved_messages
                    .try_for_each(|recieved| {
                        let (light_pos_cache, sunlight_pos_cache) = (
                            LightPosCache::<0>::new(&recieved.surrounding_blocks),
                            LightPosCache::<1>::new(&recieved.surrounding_blocks),
                        );

                        thread_lightpos_cache_sender
                            .clone()
                            .unbounded_send(LightPosCacheThreadReturn::new(
                                recieved.pos,
                                light_pos_cache,
                                sunlight_pos_cache,
                                recieved.for_state,
                            ))
                    })
                    .is_err()
                {
                    break;
                }
            })
            .unwrap();

        let (main_light_sender, mut thread_light_reciever) = unbounded::<LightThreadRequest>();
        let (thread_light_sender, main_light_reciever) = unbounded::<LightThreadReturn>();

        thread::Builder::new()
            .name("Light generator".to_string())
            .spawn(move || loop {
                #[allow(unused_mut)]
                let mut recieved_messages = {
                    #[cfg(feature = "rayon")]
                    {
                        collect_messages(&mut thread_light_reciever).into_par_iter()
                    }
                    #[cfg(not(feature = "rayon"))]
                    {
                        collect_messages(&mut thread_light_reciever).into_iter()
                    }
                };

                if recieved_messages.len() == 0 {
                    thread::sleep(Duration::from_millis(THREAD_SLEEP_TIME));
                } else if recieved_messages
                    .try_for_each(|recieved| {
                        let lights = LightBuffer::new(recieved.surrounding_blocks);

                        thread_light_sender.clone().unbounded_send(LightThreadReturn::new(
                            recieved.pos,
                            lights,
                            recieved.for_state,
                        ))
                    })
                    .is_err()
                {
                    break;
                }
            })
            .unwrap();

        let (main_blocks_sender, mut thread_blocks_reciever) = unbounded::<BlocksThreadRequest>();
        let (thread_blocks_sender, main_blocks_reciever) = unbounded::<BlocksThreadReturn>();

        let block_manager_2 = block_manager.clone();
        thread::Builder::new()
            .name("Terrain generator".to_string())
            .spawn(move || {
                ref_thread_local! {
                    static managed TERRAIN_GENERATOR: Option<TerrainGenerator> = None;
                }

                loop {
                    #[allow(unused_mut)]
                    let mut recieved_messages = {
                        #[cfg(feature = "rayon")]
                        {
                            collect_messages(&mut thread_blocks_reciever).into_par_iter()
                        }
                        #[cfg(not(feature = "rayon"))]
                        {
                            collect_messages(&mut thread_blocks_reciever).into_iter()
                        }
                    };

                    if recieved_messages.len() == 0 {
                        thread::sleep(Duration::from_millis(THREAD_SLEEP_TIME))
                    } else if recieved_messages
                        .try_for_each(|recieved| {
                            if TERRAIN_GENERATOR.borrow().is_none() {
                                *TERRAIN_GENERATOR.borrow_mut() =
                                    Some(TerrainGenerator::new(seed, block_manager_2.clone()));
                            }

                            let blocks = {
                                cfg_if! {
                                    if #[cfg(feature = "save_system")] {
                                        if let Some(block_buffer) = crate::misc::save_helper::load_block_buffer(recieved.current_save_name, "chunks/".to_string() + &chunk_file_name(&recieved.pos)) {
                                            block_buffer
                                        } else {
                                            TERRAIN_GENERATOR
                                                .borrow_mut()
                                                .as_mut()
                                                .unwrap()
                                                .generate_blocks(&recieved.pos)
                                        }
                                    } else {
                                        TERRAIN_GENERATOR
                                            .borrow_mut()
                                            .as_mut()
                                            .unwrap()
                                            .generate_blocks(&recieved.pos)
                                    }
                                }
                            };

                            thread_blocks_sender
                                .clone()
                                .unbounded_send(BlocksThreadReturn::new(recieved.pos, blocks))
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .unwrap();

        let saving_chunks = Arc::new(AtomicU32::new(0));

        #[cfg(feature = "save_system")]
        let (main_chunk_save_sender, mut thread_chunk_save_reciever) = unbounded::<SaveChunkRequest>();

        #[cfg(feature = "save_system")]
        {
            let saving_chunks = saving_chunks.clone();
            thread::Builder::new()
                .name("Chunk saver".to_string())
                .spawn(move || loop {
                    collect_messages(&mut thread_chunk_save_reciever)
                        .into_iter()
                        .for_each(|recieved| {
                            saving_chunks.fetch_add(recieved.chunks.len() as u32, Ordering::Relaxed);
                            save_many(
                                recieved.current_save_name,
                                "chunks",
                                recieved.chunks,
                                Some(saving_chunks.clone()),
                            );
                        });
                })
                .unwrap();
        }

        Self {
            chunks: FxHashMap::default(),
            requested_chunks_list: FxHashSet::default(),
            mesh_reciever: main_mesh_reciever,
            mesh_sender: main_mesh_sender,
            blocks_reciever: main_blocks_reciever,
            blocks_sender: main_blocks_sender,
            light_reciever: main_light_reciever,
            light_sender: main_light_sender,
            light_pos_cache_reciever: main_lightpos_cache_reciever,
            light_pos_cache_sender: main_lightpos_cache_sender,
            #[cfg(feature = "save_system")]
            chunk_save_sender: main_chunk_save_sender,
            #[cfg(feature = "save_system")]
            current_save_name: String::default(),
            transparency,
            texture_atlas: texture_atlas.clone_without_image(),
            loading_chunks: 0,
            saving_chunks,
            block_manager,
        }
    }

    #[cfg(feature = "save_system")]
    pub fn set_save_name(&mut self, name: String) {
        self.current_save_name = name;
    }

    #[allow(dead_code)]
    pub fn get_chunk(&mut self, chunk_pos: &Vector3<NonZeroI32>, load: bool) -> Option<Pin<&Chunk>> {
        if self.chunks.get(chunk_pos).is_some() {
            Some(self.chunks.get(chunk_pos).unwrap().as_ref())
        } else {
            if load {
                self.request_chunk_blocks(chunk_pos)
            }
            None
        }
    }

    #[allow(dead_code)]
    pub fn get_chunk_mut(&mut self, chunk_pos: &Vector3<NonZeroI32>, load: bool) -> Option<Pin<&mut Chunk>> {
        if self.chunks.get(chunk_pos).is_some() {
            Some(self.chunks.get_mut(chunk_pos).unwrap().as_mut())
        } else {
            if load {
                self.request_chunk_blocks(chunk_pos)
            }
            None
        }
    }

    #[allow(dead_code)]
    pub fn get_blocks(&mut self, chunk_pos: &Vector3<NonZeroI32>, load: bool) -> Option<Arc<BlockBuffer>> {
        self.get_chunk(chunk_pos, load).map(|chunk| chunk.blocks())
    }

    #[allow(dead_code)]
    pub fn get_lights(
        &mut self,
        chunk_pos: &Vector3<NonZeroI32>,
        load: bool,
        return_if_outdated: bool,
    ) -> Option<Arc<LightBuffer>> {
        let mut out = None;

        if let Some(chunk) = self.get_chunk(chunk_pos, load) {
            out = chunk.lights();

            if !chunk.lights_up_to_date() {
                if load {
                    self.request_chunk_light(chunk_pos)
                }
                if !return_if_outdated {
                    return None;
                }
            }
        }

        out
    }

    #[allow(dead_code)]
    pub fn get_surrounding_blocks(
        &mut self,
        center_chunk_pos: &Vector3<NonZeroI32>,
        load: bool,
    ) -> Option<[Arc<BlockBuffer>; 7]> {
        let mut out: [MaybeUninit<_>; 7] = unsafe { MaybeUninit::uninit().assume_init() };

        out[index_from_relative_pos_surrounding(&Vector3::new(0, 0, 0)) as usize] = MaybeUninit::new(self.get_blocks(
            &Vector3::new(center_chunk_pos.x, center_chunk_pos.y, center_chunk_pos.z),
            load,
        )?);
        for face in FaceDirection::iter() {
            let dir = face.as_dir();

            out[index_from_relative_pos_surrounding(&dir) as usize] = MaybeUninit::new(self.get_blocks(
                &Vector3::new(
                    add_to_non_zero_i32(center_chunk_pos.x, dir.x),
                    add_to_non_zero_i32(center_chunk_pos.y, dir.y),
                    add_to_non_zero_i32(center_chunk_pos.z, dir.z),
                ),
                load,
            )?)
        }

        Some(unsafe { mem::transmute(out) })
    }

    #[allow(dead_code)]
    pub fn get_surrounding_blocks_cube(
        &mut self,
        center_chunk_pos: &Vector3<NonZeroI32>,
        load: bool,
    ) -> Option<[Arc<BlockBuffer>; 27]> {
        let mut out: [MaybeUninit<_>; 27] = unsafe { MaybeUninit::uninit().assume_init() };

        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    out[index_from_relative_pos_surrounding_cubes(&Vector3::new(x, y, z)) as usize] =
                        MaybeUninit::new(self.get_blocks(
                            &Vector3::new(
                                add_to_non_zero_i32(center_chunk_pos.x, x),
                                add_to_non_zero_i32(center_chunk_pos.y, y),
                                add_to_non_zero_i32(center_chunk_pos.z, z),
                            ),
                            load,
                        )?)
                }
            }
        }

        Some(unsafe { mem::transmute(out) })
    }

    #[allow(dead_code)]
    pub fn get_surrounding_lights(
        &mut self,
        center_chunk_pos: &Vector3<NonZeroI32>,
        load: bool,
        return_if_outdated: bool,
    ) -> Option<[Arc<LightBuffer>; 7]> {
        let mut out: [MaybeUninit<_>; 7] = unsafe { MaybeUninit::uninit().assume_init() };

        out[index_from_relative_pos_surrounding(&Vector3::new(0, 0, 0)) as usize] = MaybeUninit::new(self.get_lights(
            &Vector3::new(center_chunk_pos.x, center_chunk_pos.y, center_chunk_pos.z),
            load,
            return_if_outdated,
        )?);
        for face in FaceDirection::iter() {
            let dir = face.as_dir();

            out[index_from_relative_pos_surrounding(&dir) as usize] = MaybeUninit::new(self.get_lights(
                &Vector3::new(
                    add_to_non_zero_i32(center_chunk_pos.x, dir.x),
                    add_to_non_zero_i32(center_chunk_pos.y, dir.y),
                    add_to_non_zero_i32(center_chunk_pos.z, dir.z),
                ),
                load,
                return_if_outdated,
            )?)
        }

        Some(unsafe { mem::transmute(out) })
    }

    #[allow(dead_code)]
    pub fn get_surrounding_lights_cube(
        &mut self,
        center_chunk_pos: &Vector3<NonZeroI32>,
        load: bool,
        return_if_outdated: bool,
    ) -> Option<[Arc<LightBuffer>; 27]> {
        let mut out: [MaybeUninit<_>; 27] = unsafe { MaybeUninit::uninit().assume_init() };

        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    out[index_from_relative_pos_surrounding_cubes(&Vector3::new(x, y, z)) as usize] =
                        MaybeUninit::new(self.get_lights(
                            &Vector3::new(
                                add_to_non_zero_i32(center_chunk_pos.x, x),
                                add_to_non_zero_i32(center_chunk_pos.y, y),
                                add_to_non_zero_i32(center_chunk_pos.z, z),
                            ),
                            load,
                            return_if_outdated,
                        )?)
                }
            }
        }

        Some(unsafe { mem::transmute(out) })
    }

    pub fn set_block(&mut self, pos: &Pos, block: Block) {
        const fn can_affect_chunk_light(x: i32, y: i32, z: i32, in_chunk_pos: Vector3<u32>) -> bool {
            !((x != 0 && y != 0 && z != 0)
                && ((x == -1 && in_chunk_pos.x >= MAX_LIGHT_VAL as u32)
                    || (x == 1 && in_chunk_pos.x <= CHUNK_SIZE - MAX_LIGHT_VAL as u32)
                    || (y == -1 && in_chunk_pos.x >= MAX_LIGHT_VAL as u32)
                    || (y == 1 && in_chunk_pos.y <= CHUNK_SIZE - MAX_LIGHT_VAL as u32)
                    || (z == -1 && in_chunk_pos.z >= MAX_LIGHT_VAL as u32)
                    || (z == 1 && in_chunk_pos.z <= CHUNK_SIZE - MAX_LIGHT_VAL as u32)))
        }

        fn highest_block_in_chunk_sees_sky(terrain: &mut Terrain, pos: &Pos) -> bool {
            let collum = {
                let in_chunk_pos = pos.in_chunk_pos_i32();
                Vector2::new(in_chunk_pos.x, in_chunk_pos.z)
            };
            let mut current_chunk_pos = *pos.chunk_pos();

            current_chunk_pos = add_non_zero_i32_vector3(current_chunk_pos, Vector3::new(0, 1, 0));
            while let Some(blocks) = terrain.get_blocks(&current_chunk_pos, false) {
                if blocks.contains_collum_opaque_blocks(&collum) {
                    return false;
                }

                current_chunk_pos = add_non_zero_i32_vector3(current_chunk_pos, Vector3::new(0, 1, 0));
            }

            true
        }

        fn chunks_to_update(pos: &Pos) -> FxHashSet<Vector3<NonZeroI32>> {
            let mut out = FxHashSet::default();
            let in_chunk_pos = {
                let temp = pos.in_chunk_pos_i32();
                Vector3::new(temp.x as u32, temp.y as u32, temp.z as u32)
            };

            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        if can_affect_chunk_light(x, y, z, in_chunk_pos) {
                            out.insert(Vector3::new(
                                add_to_non_zero_i32(pos.chunk_pos.x, x),
                                add_to_non_zero_i32(pos.chunk_pos.y, y),
                                add_to_non_zero_i32(pos.chunk_pos.z, z),
                            ));
                        }
                    }
                }
            }

            out
        }

        fn chunks_to_update_sunlight(
            terrain: &mut Terrain,
            pos: &Pos,
            contains_collum_opaque_blocks_changed: bool,
        ) -> FxHashSet<(Vector3<NonZeroI32>, Option<Either<(), Vector2<u32>>>)> {
            let mut out = FxHashSet::default();
            let in_chunk_pos = pos.in_chunk_pos_i32();
            let collum = { Vector2::new(in_chunk_pos.x, in_chunk_pos.z) };

            let mut current_chunk_pos = *pos.chunk_pos();
            out.insert((
                Vector3::new(pos.chunk_pos().x, pos.chunk_pos().y, pos.chunk_pos().z),
                Some(Either::Left(())),
            ));

            if contains_collum_opaque_blocks_changed {
                current_chunk_pos = add_non_zero_i32_vector3(current_chunk_pos, Vector3::new(0, -1, 0));
                while let Some(blocks) = terrain.get_blocks(&current_chunk_pos, false) {
                    {
                        let pos = &Pos::new(
                            current_chunk_pos,
                            Vector3::new(in_chunk_pos.x as f32, in_chunk_pos.y as f32, in_chunk_pos.z as f32),
                        );
                        let in_chunk_pos = {
                            let temp = pos.in_chunk_pos_i32();
                            Vector3::new(temp.x as u32, temp.y as u32, temp.z as u32)
                        };

                        for x in -1..=1 {
                            for y in -1..=0 {
                                for z in -1..=1 {
                                    let current_chunk_pos = Vector3::new(
                                        add_to_non_zero_i32(pos.chunk_pos.x, x),
                                        add_to_non_zero_i32(pos.chunk_pos.y, y),
                                        add_to_non_zero_i32(pos.chunk_pos.z, z),
                                    );

                                    {
                                        if can_affect_chunk_light(x, y, z, in_chunk_pos) {
                                            out.insert((
                                                current_chunk_pos,
                                                if x == 0 && y == 0 && z == 0 {
                                                    Some(Either::Left(()))
                                                } else {
                                                    None
                                                },
                                            ));

                                            {
                                                let collum = Vector2::new(in_chunk_pos.x, in_chunk_pos.z);

                                                if (x == -1 && collum.x == 0)
                                                    || (x == 1 && collum.x == CHUNK_SIZE - 1)
                                                    || (z == -1 && collum.y == 0)
                                                    || (z == 1 && collum.y == CHUNK_SIZE - 1)
                                                {
                                                    out.insert((current_chunk_pos, Some(Either::Right(collum))));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    };

                    if blocks.contains_collum_opaque_blocks(&collum) {
                        break;
                    }

                    current_chunk_pos = add_non_zero_i32_vector3(current_chunk_pos, Vector3::new(0, -1, 0));
                }
            }

            out
        }

        let highest_block_in_chunk_sees_sky = highest_block_in_chunk_sees_sky(self, pos);
        if let Some(mut chunk) = self.get_chunk_mut(&pos.chunk_pos, false) {
            let (contains_collum_opaque_block_old, contains_collum_opaque_block_new) =
                chunk.set_block(&pos.in_chunk_pos_i32(), block);

            drop(chunk);

            {
                let main_chunk_collum = {
                    let in_chunk_pos_i32 = pos.in_chunk_pos_i32();
                    Vector2::new(in_chunk_pos_i32.x as u32, in_chunk_pos_i32.z as u32)
                };
                let chunks_to_update = {
                    let mut chunks_to_update_tmp = chunks_to_update_sunlight(
                        self,
                        pos,
                        contains_collum_opaque_block_old != contains_collum_opaque_block_new,
                    );
                    chunks_to_update_tmp.extend(
                        chunks_to_update(pos)
                            .into_iter()
                            .map(|x| (x, Option::<Either<(), Vector2<u32>>>::None)),
                    );
                    chunks_to_update_tmp
                };

                {
                    let mut chunks_to_update_cache = FxHashSet::default();
                    let mut to_update_collums_in_other_chunks = Vec::new();

                    for (current_chunk_pos, update_sunlight) in chunks_to_update {
                        if let Some(mut current_chunk) = self.get_chunk_mut(&current_chunk_pos, false) {
                            if let Some(update_sunlight) = update_sunlight {
                                match update_sunlight {
                                    Either::Left(_) => {
                                        current_chunk.update_sunlight_in_collum(
                                            &main_chunk_collum,
                                            if current_chunk_pos.y < pos.chunk_pos.y {
                                                !contains_collum_opaque_block_new && highest_block_in_chunk_sees_sky
                                            } else {
                                                highest_block_in_chunk_sees_sky
                                            },
                                        );

                                        for i in 0..=3 {
                                            let collum =
                                                Vector2::new(main_chunk_collum.x as i32, main_chunk_collum.y as i32)
                                                    + (match i {
                                                        0 => Vector2::new(1, 0),
                                                        1 => Vector2::new(-1, 0),
                                                        2 => Vector2::new(0, 1),
                                                        3 => Vector2::new(0, -1),
                                                        _ => unreachable!(),
                                                    });

                                            if (collum.x >= 0 && collum.x < CHUNK_SIZE as i32)
                                                && (collum.y >= 0 && collum.y < CHUNK_SIZE as i32)
                                            {
                                                current_chunk.refresh_sunlight_in_collum(&Vector2::new(
                                                    collum.x as u32,
                                                    collum.y as u32,
                                                ))
                                            } else {
                                                let (to_update_chunk_pos, to_update_in_chunk_pos) =
                                                    coordinate_in_surrounding_buffers_cube(Vector3::new(
                                                        collum.x, 0, collum.y,
                                                    ));

                                                to_update_collums_in_other_chunks.push((
                                                    add_non_zero_i32_vector3(current_chunk_pos, to_update_chunk_pos),
                                                    Vector2::new(to_update_in_chunk_pos.x, to_update_in_chunk_pos.z),
                                                ));
                                            }
                                        }
                                    }
                                    Either::Right(collum) => current_chunk.refresh_sunlight_in_collum(&collum),
                                }

                                chunks_to_update_cache.insert(current_chunk_pos);
                            }
                            current_chunk.set_lights_outdated();
                            current_chunk.set_mesh_outdated();
                        }
                    }

                    for (current_chunk_pos, collum) in to_update_collums_in_other_chunks {
                        if let Some(mut current_chunk) = self.get_chunk_mut(&current_chunk_pos, false) {
                            current_chunk.refresh_sunlight_in_collum(&Vector2::new(collum.x as u32, collum.y as u32));

                            current_chunk.set_lights_outdated();
                            current_chunk.set_mesh_outdated();

                            chunks_to_update_cache.insert(current_chunk_pos);
                        }
                    }

                    for to_update_cache_chunk_pos in chunks_to_update_cache {
                        if let Some(surrounding_blocks) =
                            self.get_surrounding_blocks_cube(&to_update_cache_chunk_pos, false)
                        {
                            if let Some(mut current_chunk) = self.get_chunk_mut(&to_update_cache_chunk_pos, false) {
                                current_chunk.do_cache_updates(&surrounding_blocks)
                            }
                        } else {
                            log::error!(
                                "Trying to set block in a chunk with atleast one nonexistent neighbouring chunk"
                            );
                            return;
                        }
                    }
                }
            }
        } else {
            log::warn!("Trying to set block in a nonexistent chunk")
        }
    }

    #[allow(dead_code)]
    pub fn get_block(&mut self, pos: &Pos) -> Option<Block> {
        self.get_blocks(&pos.chunk_pos, false)
            .map(|blocks| blocks[&pos.in_chunk_pos_i32()].clone())
    }

    #[allow(dead_code)]
    pub fn get_light(&mut self, pos: &Pos) -> Option<LightVal> {
        self.get_lights(&pos.chunk_pos, false, false)
            .map(|lights| lights[&pos.in_chunk_pos_i32()].clone())
    }

    pub fn meshes_to_render(
        &mut self,
        camera: &Camera,
        render_distance_horizontal: u32,
        render_distance_vertical: u32,
        device: &wgpu::Device,
    ) -> Vec<&ChunkMesh> {
        #[inline]
        fn append_all_chunk_combinations(
            terrain: &mut Terrain,
            camera: &Camera,
            camera_offset: Vector3<i32>,
            device: &wgpu::Device,
            out: &mut Vec<&ChunkMesh>,
            out_transparents: &mut Vec<&ChunkMesh>,
        ) {
            chunk_to_out(
                terrain,
                add_non_zero_i32_vector3(
                    camera.pos.chunk_pos,
                    Vector3::new(camera_offset.x, camera_offset.y, camera_offset.z),
                ),
                device,
                out,
                out_transparents,
            );
            chunk_to_out(
                terrain,
                add_non_zero_i32_vector3(
                    camera.pos.chunk_pos,
                    Vector3::new(-camera_offset.x, camera_offset.y, camera_offset.z),
                ),
                device,
                out,
                out_transparents,
            );
            chunk_to_out(
                terrain,
                add_non_zero_i32_vector3(
                    camera.pos.chunk_pos,
                    Vector3::new(camera_offset.x, -camera_offset.y, camera_offset.z),
                ),
                device,
                out,
                out_transparents,
            );
            chunk_to_out(
                terrain,
                add_non_zero_i32_vector3(
                    camera.pos.chunk_pos,
                    Vector3::new(camera_offset.x, camera_offset.y, -camera_offset.z),
                ),
                device,
                out,
                out_transparents,
            );
            chunk_to_out(
                terrain,
                add_non_zero_i32_vector3(
                    camera.pos.chunk_pos,
                    Vector3::new(-camera_offset.x, -camera_offset.y, camera_offset.z),
                ),
                device,
                out,
                out_transparents,
            );
            chunk_to_out(
                terrain,
                add_non_zero_i32_vector3(
                    camera.pos.chunk_pos,
                    Vector3::new(camera_offset.x, -camera_offset.y, -camera_offset.z),
                ),
                device,
                out,
                out_transparents,
            );
            chunk_to_out(
                terrain,
                add_non_zero_i32_vector3(
                    camera.pos.chunk_pos,
                    Vector3::new(-camera_offset.x, camera_offset.y, -camera_offset.z),
                ),
                device,
                out,
                out_transparents,
            );
            chunk_to_out(
                terrain,
                add_non_zero_i32_vector3(
                    camera.pos.chunk_pos,
                    Vector3::new(-camera_offset.x, -camera_offset.y, -camera_offset.z),
                ),
                device,
                out,
                out_transparents,
            );
        }

        #[inline]
        fn chunk_to_out(
            terrain: &mut Terrain,
            chunk_pos: Vector3<NonZeroI32>,
            device: &wgpu::Device,
            out: &mut Vec<&ChunkMesh>,
            out_transparents: &mut Vec<&ChunkMesh>,
        ) {
            mesh_to_out(terrain, chunk_pos, device, out, out_transparents);
        }

        #[inline]
        fn mesh_to_out(
            terrain: &mut Terrain,
            chunk_pos: Vector3<NonZeroI32>,
            device: &wgpu::Device,
            out: &mut Vec<&ChunkMesh>,
            out_transparents: &mut Vec<&ChunkMesh>,
        ) {
            let mut do_request = false;

            if let Some(mut chunk) = terrain.get_chunk_mut(&chunk_pos, true) {
                if !chunk.mesh_up_to_date() || !chunk.lights_up_to_date() {
                    do_request = true
                }

                if let Some(mesh) = chunk.mesh(device) {
                    unsafe {
                        if mesh.0.num_elements > 0 {
                            out.push(mem::transmute::<&ChunkMesh, &'static ChunkMesh>(&mesh.0));
                        }
                        if mesh.1.num_elements > 0 {
                            out_transparents.push(mem::transmute::<&ChunkMesh, &'static ChunkMesh>(&mesh.1));
                        }
                    }
                } else {
                    do_request = true
                }
            }

            if do_request {
                terrain.request_chunk_mesh(&chunk_pos)
            }
        }

        let mut out =
            Vec::with_capacity(((render_distance_horizontal * 2 + 1).pow(2) * render_distance_vertical) as usize);
        let mut out_transparents =
            Vec::with_capacity(((render_distance_horizontal * 2 + 1) * render_distance_vertical) as usize);

        for x in 0..=render_distance_horizontal {
            for y in 0..=render_distance_vertical {
                for z in 0..=render_distance_horizontal {
                    append_all_chunk_combinations(
                        self,
                        camera,
                        Vector3::new(x as i32, y as i32, z as i32),
                        device,
                        &mut out,
                        &mut out_transparents,
                    );
                }
            }
        }

        out.extend(out_transparents);
        out
    }

    pub fn update(&mut self) {
        self.handle_recieved_chunk_blocks();
        self.handle_recieved_chunk_light_pos_caches();
        self.handle_recieved_chunk_lights();
        self.handle_recieved_chunk_meshes();
    }

    pub fn purge(
        &mut self,
        camera_chunk_pos: &Vector3<NonZeroI32>,
        render_distance_horizontal: u32,
        render_distance_vertical: u32,
    ) {
        const KEPT_SURROUNDING_CHUNKS: u32 = 4;
        log::info!("Purging chunks");

        let camera_pos_f32 = Vector3::new(
            Into::<i32>::into(camera_chunk_pos.x) as f32,
            Into::<i32>::into(camera_chunk_pos.y) as f32,
            Into::<i32>::into(camera_chunk_pos.z) as f32,
        );

        #[cfg(feature = "save_system")]
        let mut to_save = Vec::new();

        self.chunks.retain(|chunk_pos, chunk| {
            if camera_pos_f32.distance(Vector3::new(
                Into::<i32>::into(chunk_pos.x) as f32,
                Into::<i32>::into(chunk_pos.y) as f32,
                Into::<i32>::into(chunk_pos.z) as f32,
            )) <= (render_distance_horizontal.max(render_distance_vertical) + KEPT_SURROUNDING_CHUNKS) as f32
            {
                true
            } else {
                #[cfg(feature = "save_system")]
                to_save.push((chunk_file_name(chunk_pos), chunk.blocks()));

                false
            }
        });

        #[cfg(feature = "save_system")]
        self.chunk_save_sender
            .unbounded_send(SaveChunkRequest::new(
                self.current_save_name.clone(),
                to_save
                    .into_iter()
                    .map(|(save_name, blocks)| (save_name, (*blocks).clone()))
                    .collect(),
            ))
            .unwrap();
    }

    #[cfg(feature = "save_system")]
    pub fn save(&mut self) {
        self.chunk_save_sender
            .unbounded_send(SaveChunkRequest::new(
                self.current_save_name.clone(),
                self.chunks
                    .iter()
                    .map(|(chunk_pos, chunk)| (chunk_file_name(chunk_pos), (*chunk.blocks()).clone()))
                    .collect::<Vec<_>>(),
            ))
            .unwrap();
    }

    fn handle_recieved_chunk_meshes(&mut self) {
        for recieved in collect_messages(&mut self.mesh_reciever) {
            if let Some(mut chunk) = self.get_chunk_mut(&recieved.pos, false) {
                if chunk.mesh_requested() && recieved.for_state == chunk.state_hash() {
                    chunk.set_mesh((recieved.mesh.solid_mesh, recieved.mesh.transparent_mesh));

                    chunk.set_mesh_requested(false);

                    self.loading_chunks -= 1;
                }
            } else {
                log::warn!("Recieved mesh for nonexistent chunk")
            }
        }
    }

    fn handle_recieved_chunk_lights(&mut self) {
        for recieved in collect_messages(&mut self.light_reciever) {
            if let Some(mut chunk) = self.get_chunk_mut(&recieved.pos, false) {
                if chunk.lights_requested() && recieved.for_state == chunk.state_hash() {
                    if let Some(lights) = recieved.lights {
                        chunk.set_lights(lights);

                        chunk.set_lights_requested(false);
                        self.loading_chunks -= 1;
                    }
                }
            } else {
                log::warn!("Recieved chunk light for nonexistent chunk")
            }
        }
    }

    fn handle_recieved_chunk_light_pos_caches(&mut self) {
        for recieved in collect_messages(&mut self.light_pos_cache_reciever) {
            if let Some(mut chunk) = self.get_chunk_mut(&recieved.pos, false) {
                if chunk.light_pos_cache_requested() && recieved.for_state == chunk.state_hash() {
                    chunk.set_light_source_caches(recieved.light_source_cache, recieved.sunlight_source_cache);

                    chunk.set_light_pos_cache_requested(false);
                    self.loading_chunks -= 1;
                }
            } else {
                log::warn!("Recieved chunk light pos cache for nonexistent chunk")
            }
        }
    }

    fn handle_recieved_chunk_blocks(&mut self) {
        for recieved in collect_messages(&mut self.blocks_reciever) {
            if self.get_chunk(&recieved.pos, false).is_some() {
                log::warn!("Recieved blocks for already loaded chunk");
            } else {
                self.chunks.insert(recieved.pos, Box::pin(Chunk::new(recieved.blocks)));

                self.requested_chunks_list.remove(&recieved.pos);

                self.loading_chunks -= 1;
            }
        }
    }

    fn request_chunk_mesh(&mut self, chunk_pos: &Vector3<NonZeroI32>) {
        let mut set_mesh_requested = false;

        let chunk = if let Some(chunk) = self.get_chunk(chunk_pos, true) {
            chunk
        } else {
            return;
        };
        let for_state = chunk.state_hash();

        if !chunk.mesh_requested() {
            let surrounding_blocks = if let Some(surrounding_blocks) = self.get_surrounding_blocks(chunk_pos, true) {
                surrounding_blocks
            } else {
                return;
            };
            let surrounding_lights =
                if let Some(surrounding_lights) = self.get_surrounding_lights(chunk_pos, true, false) {
                    surrounding_lights
                } else {
                    return;
                };

            set_mesh_requested = true;

            self.mesh_sender
                .unbounded_send(MeshThreadRequest::new(
                    *chunk_pos,
                    surrounding_blocks,
                    surrounding_lights,
                    for_state,
                ))
                .unwrap();

            self.loading_chunks += 1;
        }

        if set_mesh_requested {
            self.get_chunk_mut(chunk_pos, true).unwrap().set_mesh_requested(true);
        }
    }

    fn request_chunk_light(&mut self, chunk_pos: &Vector3<NonZeroI32>) {
        let mut set_lights_requested = false;

        let chunk = if let Some(chunk) = self.get_chunk(chunk_pos, true) {
            chunk
        } else {
            return;
        };
        let for_state = chunk.state_hash();

        if !chunk.lights_requested() {
            let surrounding_blocks = if let Some(surrounding_blocks) = self.get_surrounding_blocks_cube(chunk_pos, true)
            {
                surrounding_blocks
            } else {
                return;
            };

            let mut all_fine = true;
            for (index, blocks) in surrounding_blocks.iter().enumerate() {
                if blocks.light_sources().is_none() || blocks.sunlight_sources().is_none() {
                    all_fine = false;
                    self.request_chunk_light_pos_cache(&add_non_zero_i32_vector3(
                        *chunk_pos,
                        relative_pos_surrounding_cubes_from_index(index as u8),
                    ))
                }
            }

            if all_fine {
                set_lights_requested = true;

                self.light_sender
                    .unbounded_send(LightThreadRequest::new(*chunk_pos, surrounding_blocks, for_state))
                    .unwrap();

                self.loading_chunks += 1;
            }
        }

        if set_lights_requested {
            self.get_chunk_mut(chunk_pos, true).unwrap().set_lights_requested(true);
        }
    }

    fn request_chunk_light_pos_cache(&mut self, chunk_pos: &Vector3<NonZeroI32>) {
        let mut set_light_pos_cache_requested = false;

        let chunk = if let Some(chunk) = self.get_chunk(chunk_pos, true) {
            chunk
        } else {
            return;
        };
        let for_state = chunk.state_hash();

        if !chunk.light_pos_cache_requested() {
            let surrounding_blocks = if let Some(surrounding_blocks) = self.get_surrounding_blocks_cube(chunk_pos, true)
            {
                surrounding_blocks
            } else {
                return;
            };

            set_light_pos_cache_requested = true;

            self.light_pos_cache_sender
                .unbounded_send(LightPosCacheThreadRequest::new(
                    *chunk_pos,
                    surrounding_blocks,
                    for_state,
                ))
                .unwrap();

            self.loading_chunks += 1;
        }

        if set_light_pos_cache_requested {
            self.get_chunk_mut(chunk_pos, true)
                .unwrap()
                .set_light_pos_cache_requested(true);
        }
    }

    fn request_chunk_blocks(&mut self, chunk_pos: &Vector3<NonZeroI32>) {
        if self.get_chunk(chunk_pos, false).is_some() {
            log::warn!("Requsting blocks for existing chunk");
        } else {
            if !self.requested_chunks_list.contains(chunk_pos) {
                self.requested_chunks_list.insert(*chunk_pos);

                self.blocks_sender
                    .unbounded_send(BlocksThreadRequest::new(*chunk_pos, {
                        #[cfg(feature = "save_system")]
                        {
                            self.current_save_name.clone()
                        }
                        #[cfg(not(feature = "save_system"))]
                        {
                            String::default()
                        }
                    }))
                    .unwrap();

                self.loading_chunks += 1;
            }
        }
    }

    pub fn reset_chunks(&mut self, seed: u32) {
        let mut new_terrain = Terrain::new(self.transparency, &self.texture_atlas, seed, self.block_manager.clone());

        mem::swap(self, &mut new_terrain);
        self.chunks = new_terrain.chunks;

        self.chunks.iter_mut().for_each(|(_, chunk)| {
            chunk.set_mesh_requested(false);
            chunk.set_lights_requested(false);
            chunk.set_light_pos_cache_requested(false);
        });
    }

    pub fn transparency(&self) -> bool {
        self.transparency
    }

    pub fn texture_atlas(&self) -> &TextureAtlas {
        &self.texture_atlas
    }

    pub fn loading_chunks(&self) -> u32 {
        self.loading_chunks / 4
    }

    pub fn saving_chunks(&self) -> u32 {
        self.saving_chunks.load(Ordering::Relaxed)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn chunk_file_name(chunk_pos: &Vector3<impl Into<i32> + Copy>) -> String {
    format!(
        "({}, {}, {})",
        chunk_pos.x.into(),
        chunk_pos.y.into(),
        chunk_pos.z.into()
    )
}

fn collect_messages<T>(reciever: &mut UnboundedReceiver<T>) -> Vec<T> {
    {
        let mut out = Vec::new();

        while let Ok(Some(msg)) = reciever.try_next() {
            out.push(msg)
        }

        out
    }
}
