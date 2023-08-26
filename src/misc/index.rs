use cgmath::{Vector2, Vector3};

use crate::game::world::CHUNK_SIZE;

#[allow(dead_code)]
#[inline]
pub const fn index_from_relative_pos_surrounding(pos: &Vector3<i32>) -> u8 {
    debug_assert!(
        pos.x >= -1
            && pos.x <= 1
            && pos.y >= -1
            && pos.y <= 1
            && pos.z >= -1
            && pos.z <= 1
            && pos.x.abs() + pos.y.abs() + pos.z.abs() <= 2
    );

    if pos.x == 0 && pos.y == 0 && pos.z == 0 {
        0
    } else if pos.x == 0 && pos.y == 1 && pos.z == 0 {
        1
    } else if pos.x == 0 && pos.y == -1 && pos.z == 0 {
        2
    } else if pos.x == -1 && pos.y == 0 && pos.z == 0 {
        3
    } else if pos.x == 1 && pos.y == 0 && pos.z == 0 {
        4
    } else if pos.x == 0 && pos.y == 0 && pos.z == -1 {
        5
    } else if pos.x == 0 && pos.y == 0 && pos.z == 1 {
        6
    } else {
        unreachable!()
    }
}

#[allow(dead_code)]
#[inline]
pub const fn relative_pos_surrounding_from_index(index: u8) -> Vector3<i32> {
    debug_assert!(index < 7);

    if index == 0 {
        Vector3::new(0, 0, 0)
    } else if index == 1 {
        Vector3::new(0, 1, 0)
    } else if index == 2 {
        Vector3::new(0, -1, 0)
    } else if index == 3 {
        Vector3::new(-1, 0, 0)
    } else if index == 4 {
        Vector3::new(1, 0, 0)
    } else if index == 5 {
        Vector3::new(0, 0, -1)
    } else if index == 6 {
        Vector3::new(0, 0, 1)
    } else {
        unreachable!()
    }
}

#[allow(dead_code)]
#[inline]
pub const fn index_from_relative_pos_surrounding_cubes(pos: &Vector3<i32>) -> u8 {
    debug_assert!(pos.x >= -1 && pos.x <= 1 && pos.y >= -1 && pos.y <= 1 && pos.z >= -1 && pos.z <= 1);

    (((pos.z + 1) * 3 * 3) + ((pos.y + 1) * 3) + (pos.x + 1)) as u8
}

#[allow(dead_code)]
#[inline]
pub const fn relative_pos_surrounding_cubes_from_index(index: u8) -> Vector3<i32> {
    debug_assert!(index < 27);

    let mut index = index as i32;

    let z = index / (3 * 3);
    index -= z * 3 * 3;

    let y = index / 3;
    let x = index % 3;

    Vector3::new(x - 1, y - 1, z - 1)
}

#[allow(dead_code)]
#[inline]
pub const fn index_from_pos_2d(pos: &Vector2<i32>) -> u32 {
    debug_assert!(pos.x >= 0 && pos.x < CHUNK_SIZE as i32 && pos.y >= 0 && pos.y < CHUNK_SIZE as i32);

    (pos.y * CHUNK_SIZE as i32 + pos.x) as u32
}

#[allow(dead_code)]
#[inline]
pub const fn pos_from_index_2d(index: u32) -> Vector2<i32> {
    debug_assert!(index < CHUNK_SIZE.pow(2));

    let x = index % CHUNK_SIZE;
    let y = (index - x) / CHUNK_SIZE;

    Vector2::new(x as i32, y as i32)
}
