use cgmath::{Matrix4, Rad, SquareMatrix, Vector2, Vector3};

use crate::{game::world::CHUNK_SIZE, misc::pos::Pos};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_position: [f32; 4],
    view_proj: [[f32; 4]; 4],
    chunk_pos_and_chunk_size: [i32; 4],
}

impl CameraUniform {
    pub fn update_view_proj(&mut self, camera: &impl Camera, projection: &impl Projection) {
        let pos = camera.pos();

        self.view_position = pos.in_chunk_pos_point().to_homogeneous().into();
        self.view_proj = (projection.calc_matrix() * camera.calc_matrix()).into();
        self.chunk_pos_and_chunk_size = {
            let chunk_pos = {
                let mut chunk_pos: Vector3<i32> = {
                    let chunk_pos = pos.chunk_pos();
                    Vector3::new(chunk_pos.x.into(), chunk_pos.y.into(), chunk_pos.z.into())
                };

                if chunk_pos.x < 0 {
                    chunk_pos.x += 1
                }
                if chunk_pos.y < 0 {
                    chunk_pos.y += 1
                }
                if chunk_pos.z < 0 {
                    chunk_pos.z += 1
                }

                chunk_pos
            };
            [chunk_pos.x, chunk_pos.y, chunk_pos.z, CHUNK_SIZE as i32]
        }
    }
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view_position: [0.0; 4],
            view_proj: Matrix4::identity().into(),
            chunk_pos_and_chunk_size: [0; 4],
        }
    }
}

pub trait Camera {
    fn pos(&self) -> Pos;
    fn calc_matrix(&self) -> Matrix4<f32>;
}

pub trait Projection: Sized + Default {
    fn calc_matrix(&self) -> Matrix4<f32>;
    fn resize(&mut self, new_size: Vector2<u32>);
    fn set_vfov(&mut self, val: Rad<f32>, display_size: Vector2<u32>);
}
