use std::rc::Rc;

use winit::event::*;

use crate::engine::{resource::Draw, TextureAtlas};

const PURGE_ENABLED: bool = false;
// const PURGE_ENABLED: bool = cfg!(not(target_arch = "wasm32"));

#[cfg(feature = "save_system")]
use crate::misc::save_helper::{available_saves, load_player, load_u32, save};
use crate::{
    game::{
        player::Player,
        player::BLOCK_UPDATE_MIN_DELAY,
        player::PLAYER_REACH,
        ray::Ray,
        world::{Block, BlockManager, Terrain, TerrainGenerator},
        Camera,
    },
    misc::Settings,
};

const CHUNK_PURGE_INTERVAL: f64 = 120.0;

pub struct State {
    terrain: Terrain,
    block_manager: Rc<BlockManager>,
    player: Player,
    seed: u32,
    purge_counter: f64,
    #[cfg(feature = "save_system")]
    current_save_name: String,
}

impl State {
    pub fn new(texture_atlas: &TextureAtlas, block_manager: BlockManager, load_last_save: bool) -> Self {
        let seed = TerrainGenerator::generate_seed();

        #[cfg(feature = "save_system")]
        let current_save_name = if load_last_save {
            available_saves().into_iter().next().unwrap_or(seed.to_string())
        } else {
            seed.to_string()
        };

        let mut out = Self {
            terrain: {
                let mut terrain = Terrain::new(
                    !cfg!(target_arch = "wasm32"),
                    texture_atlas,
                    seed,
                    block_manager.clone(),
                );
                #[cfg(feature = "save_system")]
                terrain.set_save_name(current_save_name.clone());

                terrain
            },
            player: Player::new(&block_manager),
            block_manager: Rc::new(block_manager),
            seed,
            #[cfg(feature = "save_system")]
            current_save_name,
            purge_counter: 0.0,
        };

        #[cfg(feature = "save_system")]
        if load_last_save {
            out.load();
        }

        out
    }

    pub fn update(&mut self, game_running: bool, dt: instant::Duration, settings: &Settings) {
        self.terrain.update();

        if PURGE_ENABLED && self.purge_counter >= CHUNK_PURGE_INTERVAL {
            self.terrain.purge(
                &self.player.camera.pos.chunk_pos(),
                settings.render_distance_horizontal,
                settings.render_distance_vertical,
            );

            self.purge_counter = 0.0;
        }
        self.purge_counter += dt.as_secs_f64();

        if game_running {
            self.player.update(dt, &mut self.terrain, settings);
        }
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state: ElementState::Pressed,
                ..
            }
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::X),
                        ..
                    },
                ..
            } => {
                self.break_block();
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Right,
                state: ElementState::Pressed,
                ..
            }
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::C),
                        ..
                    },
                ..
            } => {
                self.place_block();
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Middle,
                state: ElementState::Pressed,
                ..
            }
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::V),
                        ..
                    },
                ..
            } => {
                self.pick_block();
                true
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(VirtualKeyCode::M),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                if let Some(mut chunk) = self.terrain.get_chunk_mut(self.player.camera.pos.chunk_pos(), false) {
                    chunk.set_lights_outdated();
                    chunk.set_mesh_outdated();
                }
                true
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(key),
                        state,
                        ..
                    },
                ..
            } => self.player.process_keyboard(*key, *state),
            _ => false,
        }
    }

    pub fn input_mouse(&mut self, delta: (f64, f64)) {
        self.player.input_mouse(delta)
    }

    #[cfg(feature = "save_system")]
    pub fn save(&mut self) {
        if self.saving_chunks() == 0 {
            self.terrain.set_save_name(self.current_save_name.clone());

            save(self.current_save_name.clone(), "player", &self.player, false);
            save(self.current_save_name.clone(), "seed", &self.seed, false);
            self.terrain.save();
        } else {
            log::warn!("Already saving")
        }
    }

    #[cfg(feature = "save_system")]
    pub fn load(&mut self) {
        self.purge_counter = 0.0;

        self.player = if let Some(player) = load_player(self.current_save_name.clone(), "player") {
            player
        } else {
            log::warn!("Failed loading player from save {:?}", self.current_save_name);
            Player::new(&self.block_manager)
        };
        self.seed = if let Some(seed) = load_u32(self.current_save_name.clone(), "seed") {
            seed
        } else {
            log::warn!("Failed loading seed from save {:?}", self.current_save_name);
            self.seed
        };

        self.terrain = {
            let mut terrain = Terrain::new(
                self.terrain.transparency(),
                self.terrain.texture_atlas(),
                self.seed,
                (*self.block_manager).clone(),
            );
            terrain.set_save_name(self.current_save_name.to_string());
            terrain
        };
    }

    pub fn meshes_to_render(&mut self, device: &wgpu::Device, settings: &Settings) -> Vec<&impl Draw> {
        self.terrain.meshes_to_render(
            &self.player.camera,
            settings.render_distance_horizontal,
            settings.render_distance_vertical,
            device,
        )
    }

    pub fn loading_chunks(&self) -> u32 {
        self.terrain.loading_chunks()
    }

    pub fn saving_chunks(&self) -> u32 {
        self.terrain.saving_chunks()
    }

    fn break_block(&mut self) {
        if self.player.last_block_update_time_dt() >= BLOCK_UPDATE_MIN_DELAY {
            let ray = Ray::new(self.camera().pos, self.camera().forward_vec_xyz(), Some(PLAYER_REACH));

            if let Some((intersect_pos, _, _)) = ray.intersect(&mut self.terrain) {
                self.terrain
                    .set_block(&intersect_pos, Block::new("Air", &self.block_manager, None, false))
            }

            self.player.set_last_block_update_time()
        } else {
            log::info!("Player trying to break blocks too fast")
        }
    }

    fn place_block(&mut self) {
        if self.player.last_block_update_time_dt() >= BLOCK_UPDATE_MIN_DELAY {
            let ray = Ray::new(self.camera().pos, self.camera().forward_vec_xyz(), Some(PLAYER_REACH));

            let selected_block = self.player.selected_block.clone();
            if let Some(light_source) = selected_block.light_source() {
                if !light_source.is_valid() {
                    log::warn!("Trying to place invalid light source");
                    return;
                }
            }

            if let Some((_, Some(place_pos), _)) = ray.intersect(&mut self.terrain) {
                if place_pos.in_chunk_pos_i32() != self.player.camera.pos.in_chunk_pos_i32() {
                    self.terrain.set_block(&place_pos, selected_block)
                }
            }
            self.player.set_last_block_update_time()
        } else {
            log::info!("Player trying to break blocks too fast")
        }
    }

    fn pick_block(&mut self) {
        let ray = Ray::new(self.camera().pos, self.camera().forward_vec_xyz(), Some(PLAYER_REACH));

        if let Some((intersect_pos, _, _)) = ray.intersect(&mut self.terrain) {
            if let Some(block) = self.terrain.get_block(&intersect_pos) {
                self.player.selected_block = block
            }
        }
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    pub fn camera(&self) -> &Camera {
        &self.player.camera
    }

    pub fn selected_block_mut(&mut self) -> &mut Block {
        self.player.selected_block_mut()
    }

    #[cfg(feature = "save_system")]
    pub fn selected_save(&self) -> String {
        self.current_save_name.clone()
    }

    #[cfg(feature = "save_system")]
    pub fn set_selected_save(&mut self, save_name: String) {
        self.current_save_name = save_name;
    }

    pub fn cancel_requests(&mut self) {
        self.terrain.reset_chunks(self.seed);

        #[cfg(feature = "save_system")]
        self.terrain.set_save_name(self.current_save_name.clone())
    }

    pub fn block_manager(&self) -> Rc<BlockManager> {
        self.block_manager.clone()
    }
}
