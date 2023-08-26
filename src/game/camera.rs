use std::{f32::consts::FRAC_PI_2, num::NonZeroI32};

use cgmath::{perspective, Angle, Deg, InnerSpace, Matrix4, Rad, Vector2, Vector3};
use instant::Duration;
use serde::{Deserialize, Serialize};
use winit::event::{ElementState, VirtualKeyCode};

use crate::{
    game::{
        move_pos,
        world::{Terrain, CHUNK_SIZE},
    },
    misc::{pos::Pos, Settings},
};

#[rustfmt::skip]
 const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);
const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Camera {
    pub pos: Pos,
    pub yaw: Rad<f32>,
    pub pitch: Rad<f32>,
}

impl Camera {
    pub fn new(
        chunk_pos: impl Into<Vector3<NonZeroI32>>,
        yaw: impl Into<Rad<f32>>,
        pitch: impl Into<Rad<f32>>,
    ) -> Self {
        Self {
            pos: Pos::new(
                chunk_pos.into(),
                Vector3::new(
                    (CHUNK_SIZE as f32 - 1.0) / 2.0,
                    (CHUNK_SIZE as f32 - 1.0) / 2.0,
                    (CHUNK_SIZE as f32 - 1.0) / 2.0,
                ),
            ),
            yaw: yaw.into(),
            pitch: pitch.into(),
        }
    }

    pub fn yaw(&self) -> Rad<f32> {
        self.yaw
    }

    pub fn pitch(&self) -> Rad<f32> {
        self.pitch
    }

    #[allow(dead_code)]
    pub fn forward_vec_xz(&self) -> Vector3<f32> {
        let (yaw_sin, yaw_cos) = self.yaw.0.sin_cos();
        Vector3::new(yaw_cos, 0.0, yaw_sin).normalize()
    }

    #[allow(dead_code)]
    pub fn forward_vec_xyz(&self) -> Vector3<f32> {
        let xz_len = self.pitch.cos();
        let (yaw_sin, yaw_cos) = self.yaw.0.sin_cos();
        Vector3::new(yaw_cos * xz_len, self.pitch.sin(), yaw_sin * xz_len).normalize()
    }

    #[allow(dead_code)]
    pub fn right_vec(&self) -> Vector3<f32> {
        let (yaw_sin, yaw_cos) = self.yaw.0.sin_cos();
        Vector3::new(-yaw_sin, 0.0, yaw_cos).normalize()
    }

    #[allow(dead_code)]
    pub fn up_vec(&self) -> Vector3<f32> {
        self.right_vec().cross(self.forward_vec_xz())
    }
}

impl crate::engine::camera::Camera for Camera {
    fn pos(&self) -> Pos {
        self.pos
    }

    fn calc_matrix(&self) -> Matrix4<f32> {
        let (sin_pitch, cos_pitch) = self.pitch.0.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.0.sin_cos();

        Matrix4::look_to_rh(
            self.pos.in_chunk_pos_point(),
            Vector3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize(),
            Vector3::unit_y(),
        )
    }
}

#[derive(Clone, Debug)]
pub struct Projection {
    aspect: f32,
    vfov: Rad<f32>,
    znear: f32,
    zfar: f32,
}

impl Projection {
    const ZNEAR: f32 = 0.005;
    const ZFAR: f32 = 10000.0;

    pub fn new(display_size: Vector2<u32>, vfov: impl Into<Rad<f32>>, znear: f32, zfar: f32) -> Self {
        Self {
            aspect: display_size.x as f32 / display_size.y as f32,
            vfov: vfov.into(),
            znear,
            zfar,
        }
    }
}

impl crate::engine::camera::Projection for Projection {
    fn resize(&mut self, new_size: Vector2<u32>) {
        self.aspect = new_size.x as f32 / new_size.y as f32;
    }

    fn calc_matrix(&self) -> Matrix4<f32> {
        OPENGL_TO_WGPU_MATRIX * perspective(self.vfov, self.aspect, self.znear, self.zfar)
    }

    fn set_vfov(&mut self, val: Rad<f32>, display_size: Vector2<u32>) {
        *self = Self::new(display_size, val, self.znear, self.zfar);
    }
}

impl Default for Projection {
    fn default() -> Self {
        Projection::new(Vector2::new(512, 512), Deg(100.0), Projection::ZNEAR, Projection::ZFAR)
    }
}

#[derive(Clone, Debug, Default)]
pub struct CameraController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
}

impl CameraController {
    pub fn new() -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
        }
    }

    pub fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        let amount = if state == ElementState::Pressed { 1.0 } else { 0.0 };

        match key {
            VirtualKeyCode::W | VirtualKeyCode::Up => {
                self.amount_forward = amount;
                true
            }
            VirtualKeyCode::S | VirtualKeyCode::Down => {
                self.amount_backward = amount;
                true
            }
            VirtualKeyCode::A | VirtualKeyCode::Left => {
                self.amount_left = amount;
                true
            }
            VirtualKeyCode::D | VirtualKeyCode::Right => {
                self.amount_right = amount;
                true
            }
            VirtualKeyCode::Space | VirtualKeyCode::K => {
                self.amount_up = amount;
                true
            }
            VirtualKeyCode::LShift | VirtualKeyCode::J => {
                self.amount_down = amount;
                true
            }
            _ => false,
        }
    }

    pub fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    pub fn update_camera(&mut self, camera: &mut Camera, dt: Duration, terrain: &mut Terrain, settings: &Settings) {
        let dt = dt.as_secs_f32();

        let motion = self.motion_amount(camera, settings.camera_speed * dt);
        if settings.collision {
            camera.pos = move_pos(camera.pos, motion, terrain)
        } else {
            camera.pos.in_chunk_pos += motion;
            camera.pos.check_in_chunk_overflow();
        }

        camera.yaw += Rad(self.rotate_horizontal) * settings.camera_sensitivity * dt;

        camera.pitch += Rad(-self.rotate_vertical) * settings.camera_sensitivity * dt;

        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        if camera.pitch < -Rad(SAFE_FRAC_PI_2) {
            camera.pitch = -Rad(SAFE_FRAC_PI_2);
        } else if camera.pitch > Rad(SAFE_FRAC_PI_2) {
            camera.pitch = Rad(SAFE_FRAC_PI_2);
        }
        camera.yaw = camera.yaw.normalize_signed()
    }

    fn motion_amount(&mut self, camera: &mut Camera, by: f32) -> Vector3<f32> {
        ((camera.forward_vec_xz() * (self.amount_forward - self.amount_backward))
            + (camera.right_vec() * (self.amount_right - self.amount_left))
            + (Vector3::new(0.0, 1.0, 0.0) * (self.amount_up - self.amount_down)))
            * by
    }
}
