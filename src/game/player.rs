use std::num::NonZeroI32;

use instant::{Duration, Instant};
use serde::{Deserialize, Serialize};
use winit::event::{ElementState, VirtualKeyCode};

use crate::{
    game::{
        world::{Block, BlockManager, Terrain},
        Camera, CameraController,
    },
    misc::Settings,
};

pub const PLAYER_REACH: f32 = 20.0;
pub const BLOCK_UPDATE_MIN_DELAY: f64 = 0.05;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub selected_block: Block,
    pub camera: Camera,
    #[serde(skip)]
    pub camera_controller: CameraController,
    #[serde(skip)]
    last_block_update_time: Option<Instant>,
}

impl Player {
    pub fn new(block_manager: &BlockManager) -> Self {
        Self {
            selected_block: Block::new(
                block_manager.all_rendered_block_names()[0].as_str(),
                block_manager,
                None,
                false,
            ),
            camera: {
                Camera::new(
                    (
                        NonZeroI32::new(1).unwrap(),
                        NonZeroI32::new(1).unwrap(),
                        NonZeroI32::new(1).unwrap(),
                    ),
                    cgmath::Deg(0.0),
                    cgmath::Deg(0.0),
                )
            },
            camera_controller: CameraController::new(),
            last_block_update_time: None,
        }
    }

    pub fn update(&mut self, dt: Duration, terrain: &mut Terrain, settings: &Settings) {
        self.camera_controller
            .update_camera(&mut self.camera, dt, terrain, settings);
    }

    pub fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        self.camera_controller.process_keyboard(key, state)
    }

    pub fn input_mouse(&mut self, delta: (f64, f64)) {
        self.camera_controller.process_mouse(delta.0, delta.1)
    }

    pub fn selected_block_mut(&mut self) -> &mut Block {
        &mut self.selected_block
    }

    pub fn set_last_block_update_time(&mut self) {
        self.last_block_update_time = Some(Instant::now())
    }

    pub fn last_block_update_time_dt(&self) -> f64 {
        if let Some(last_block_update_time) = self.last_block_update_time {
            (Instant::now() - last_block_update_time).as_secs_f64()
        } else {
            f64::INFINITY
        }
    }
}
