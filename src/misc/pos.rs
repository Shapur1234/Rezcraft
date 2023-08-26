use std::num::NonZeroI32;

use cgmath::{Point3, Vector3};
use serde::{Deserialize, Serialize};

use crate::game::world::CHUNK_SIZE;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pos {
    pub chunk_pos: Vector3<NonZeroI32>,
    pub in_chunk_pos: Vector3<f32>,
}

impl Pos {
    pub const fn new(chunk_pos: Vector3<NonZeroI32>, in_chunk_pos: Vector3<f32>) -> Self {
        Self {
            chunk_pos,
            in_chunk_pos,
        }
    }

    pub fn check_in_chunk_overflow(&mut self) {
        while self.in_chunk_pos.x < 0.0 {
            self.in_chunk_pos.x += CHUNK_SIZE as f32;
            self.chunk_pos.x = add_to_non_zero_i32(self.chunk_pos.x, -1)
        }
        while self.in_chunk_pos.x >= CHUNK_SIZE as f32 {
            self.in_chunk_pos.x -= CHUNK_SIZE as f32;
            self.chunk_pos.x = add_to_non_zero_i32(self.chunk_pos.x, 1)
        }

        while self.in_chunk_pos.y < 0.0 {
            self.in_chunk_pos.y += CHUNK_SIZE as f32;
            self.chunk_pos.y = add_to_non_zero_i32(self.chunk_pos.y, -1)
        }
        while self.in_chunk_pos.y >= CHUNK_SIZE as f32 {
            self.in_chunk_pos.y -= CHUNK_SIZE as f32;
            self.chunk_pos.y = add_to_non_zero_i32(self.chunk_pos.y, 1)
        }

        while self.in_chunk_pos.z < 0.0 {
            self.in_chunk_pos.z += CHUNK_SIZE as f32;
            self.chunk_pos.z = add_to_non_zero_i32(self.chunk_pos.z, -1)
        }
        while self.in_chunk_pos.z >= CHUNK_SIZE as f32 {
            self.in_chunk_pos.z -= CHUNK_SIZE as f32;
            self.chunk_pos.z = add_to_non_zero_i32(self.chunk_pos.z, 1)
        }
    }

    pub const fn chunk_pos(&self) -> &Vector3<NonZeroI32> {
        &self.chunk_pos
    }

    pub fn in_chunk_pos_i32(&self) -> Vector3<i32> {
        Vector3::new(
            self.in_chunk_pos.x.floor() as i32,
            self.in_chunk_pos.y.floor() as i32,
            self.in_chunk_pos.z.floor() as i32,
        )
    }

    pub const fn in_chunk_pos_f32(&self) -> Vector3<f32> {
        self.in_chunk_pos
    }

    pub const fn in_chunk_pos_point(&self) -> Point3<f32> {
        Point3::new(self.in_chunk_pos.x, self.in_chunk_pos.y, self.in_chunk_pos.z)
    }

    pub fn abs_pos(&self) -> Vector3<f64> {
        let offset: Vector3<f64> = Vector3::new(
            Into::<i32>::into(self.chunk_pos.x) as f64 * CHUNK_SIZE as f64,
            Into::<i32>::into(self.chunk_pos.y) as f64 * CHUNK_SIZE as f64,
            Into::<i32>::into(self.chunk_pos.z) as f64 * CHUNK_SIZE as f64,
        );
        Vector3::new(
            offset.x
                + self.in_chunk_pos.x as f64
                + if Into::<i32>::into(self.chunk_pos.x) < 0 {
                    CHUNK_SIZE as f64
                } else {
                    0.0
                },
            offset.y
                + self.in_chunk_pos.y as f64
                + if Into::<i32>::into(self.chunk_pos.y) < 0 {
                    CHUNK_SIZE as f64
                } else {
                    0.0
                },
            offset.z
                + self.in_chunk_pos.z as f64
                + if Into::<i32>::into(self.chunk_pos.z) < 0 {
                    CHUNK_SIZE as f64
                } else {
                    0.0
                },
        ) - Vector3::new(CHUNK_SIZE as f64, CHUNK_SIZE as f64, CHUNK_SIZE as f64)
    }
}

#[inline]
pub fn add_to_non_zero_i32(num1: NonZeroI32, num2: i32) -> NonZeroI32 {
    let num1_i32: i32 = num1.into();

    let mut result = num1_i32 + num2;
    if result == 0 {
        result = if num2 > 0 { 1 } else { -1 }
    }

    NonZeroI32::new(result).unwrap()
}

#[inline]
pub fn add_non_zero_i32_vector3(vec1: Vector3<NonZeroI32>, vec2: Vector3<i32>) -> Vector3<NonZeroI32> {
    Vector3::new(
        add_to_non_zero_i32(vec1.x, vec2.x),
        add_to_non_zero_i32(vec1.y, vec2.y),
        add_to_non_zero_i32(vec1.z, vec2.z),
    )
}
