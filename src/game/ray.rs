use cgmath::{InnerSpace, Vector3};

use crate::{game::world::Terrain, misc::pos::Pos};

pub struct Ray {
    from: Pos,
    dir: Vector3<f32>,
    length: f32,
}

impl Ray {
    pub fn new(from: Pos, dir: Vector3<f32>, length: Option<f32>) -> Self {
        Self {
            from,
            dir: dir.normalize(),
            length: length.unwrap_or(1.0),
        }
    }

    pub fn intersect(&self, terrain: &mut Terrain) -> Option<(Pos, Option<Pos>, Pos)> {
        let mut out = None;
        let mut current_pos = self.from;
        let mut last_pos = None::<Pos>;

        voxel_raycast(
            self.from.in_chunk_pos_f32(),
            self.dir,
            self.length,
            |index, intersect_pos, _| {
                last_pos = Some(current_pos);

                current_pos.chunk_pos = self.from.chunk_pos;
                current_pos.in_chunk_pos = Vector3::new(index.x as f32, index.y as f32, index.z as f32);
                current_pos.check_in_chunk_overflow();

                let mut done = false;

                if let Some(block) = terrain.get_block(&current_pos) {
                    if block.is_rendered() {
                        out = Some((current_pos, last_pos, {
                            let mut pos_tmp = current_pos;
                            pos_tmp.in_chunk_pos = intersect_pos;
                            pos_tmp.check_in_chunk_overflow();
                            pos_tmp
                        }));
                        done = true;
                    }
                } else {
                    out = None;
                    done = true;
                }

                done
            },
        );

        out
    }
}

pub fn move_pos(pos: Pos, motion: Vector3<f32>, terrain: &mut Terrain) -> Pos {
    const MIN_DISTANCE_FROM_BLOCK: f32 = 0.2;

    let min_motion = motion.normalize_to(MIN_DISTANCE_FROM_BLOCK);

    let mut pos_out = pos.clone();
    pos_out.in_chunk_pos += Vector3::new(0, 1, 2).map(|idx: usize| {
        let mut offset = motion[idx];

        let trying_pos = {
            let mut pos_tmp = pos.clone();
            pos_tmp.in_chunk_pos[idx] += offset;
            pos_tmp.check_in_chunk_overflow();
            pos_tmp
        };

        if let Some(block) = terrain.get_block(&trying_pos) {
            if block.is_solid() {
                offset = min_motion[idx];
                let trying_pos = {
                    let mut pos_tmp = pos.clone();
                    pos_tmp.in_chunk_pos[idx] += offset;
                    pos_tmp.check_in_chunk_overflow();
                    pos_tmp
                };

                if let Some(block) = terrain.get_block(&trying_pos) {
                    if block.is_solid() {
                        offset = 0.0
                    }
                }
            }
        }

        if offset.is_finite() {
            offset
        } else {
            0.0
        }
    });
    pos_out.check_in_chunk_overflow();

    pos_out
}

fn voxel_raycast(
    origin: Vector3<f32>,
    dir: Vector3<f32>,
    max_dir: f32,
    mut func: impl FnMut(Vector3<i32>, Vector3<f32>, Vector3<i32>) -> bool,
) {
    // Based on https://docs.rs/voxel-tile-raycast/latest/voxel_tile_raycast/fn.tile_raycast.html

    fn voxel_stepped_index(t_max: Vector3<f32>) -> usize {
        if t_max.x < t_max.y && t_max.x < t_max.z {
            0
        } else if t_max.y < t_max.z {
            1
        } else {
            2
        }
    }

    let dir = dir.normalize();
    let mut t = 0.0;
    let mut index = origin.map(|val| val.floor() as i32);
    let step = dir.map(|val| val.signum() as i32);
    let t_delta = dir.map(|val| (1.0 / val).abs());
    let dist = (Vector3::new(0, 1, 2)).map(|val| {
        if step[val] > 0 {
            index[val] as f32 + 1.0 - origin[val]
        } else {
            origin[val] - index[val] as f32
        }
    });
    let mut t_max = (Vector3::new(0, 1, 2)).map(|val| {
        if t_delta[val] < f32::INFINITY {
            t_delta[val] * dist[val]
        } else {
            f32::INFINITY
        }
    });
    if !func(
        index,
        (Vector3::new(0, 1, 2)).map(|val| origin[val] + t * dir[val]),
        Vector3::new(0, 0, 0),
    ) {
        while t < max_dir {
            let stepped_index = voxel_stepped_index(t_max);
            index[stepped_index] += step[stepped_index];
            t = t_max[stepped_index];
            t_max[stepped_index] += t_delta[stepped_index];
            if func(index, (Vector3::new(0, 1, 2)).map(|val| origin[val] + t * dir[val]), {
                let mut hit_norm = Vector3::new(0, 0, 0);
                hit_norm[stepped_index] = -step[stepped_index];
                hit_norm
            }) {
                break;
            }
        }
    }
}
