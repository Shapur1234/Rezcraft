use std::{
    mem::{self, MaybeUninit},
    num::NonZeroI32,
    sync::Arc,
};

use block_mesh::{
    greedy_quads, ndshape::ConstShape, ndshape::ConstShape3u32, GreedyQuadsBuffer, RIGHT_HANDED_Y_UP_CONFIG,
};
use cgmath::Vector3;
use either::Either;
use strum::IntoEnumIterator;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    vertex_attr_array, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingType, Buffer, BufferAddress, BufferBindingType, BufferUsages, Device, IndexFormat, RenderPass, ShaderStages,
    VertexBufferLayout, VertexStepMode,
};

use super::coordinate_in_surrounding_buffers;
use crate::{
    engine::{
        face::{FaceDirection, SideDirection},
        resource::{Draw, Material, Vertex},
        TextureAtlas,
    },
    game::world::{Block, BlockBuffer, LightBuffer, LightVal, Voxel, CHUNK_SIZE_MESHING},
    misc::index::index_from_relative_pos_surrounding,
};

pub type ChunkShapeMeshing = ConstShape3u32<CHUNK_SIZE_MESHING, CHUNK_SIZE_MESHING, CHUNK_SIZE_MESHING>;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Zeroable)]
pub struct BlockVertex {
    pub pos: [u8; 4],
    pub normal: [i8; 4],
    pub color: [u8; 4],
    pub texture_atlas_pos: [f32; 2],
    pub brightness: u8,
    pub transparency: u8,
}

unsafe impl bytemuck::Pod for BlockVertex {}

impl Vertex for BlockVertex {
    fn desc<'a>() -> VertexBufferLayout<'a> {
        use wgpu::VertexAttribute;

        static ATTRIBUTES: [VertexAttribute; 5] = vertex_attr_array![
            0 => Uint8x4,
            1 => Sint8x4,
            2 => Uint8x4,
            3 => Float32x2,
            4 => Uint8x2,
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChunkMeshRaw {
    pub name: String,
    pub vertices: Vec<BlockVertex>,
    pub indices: Vec<u32>,
    pub chunk_pos: Vector3<i32>,
}

impl ChunkMeshRaw {
    pub fn new(
        name: String,
        vertices: Vec<BlockVertex>,
        indices: Vec<u32>,
        chunk_pos: Vector3<impl Into<i32>>,
    ) -> Self {
        Self {
            name,
            vertices,
            indices,
            chunk_pos: {
                let mut chunk_pos: Vector3<i32> =
                    Vector3::new(chunk_pos.x.into(), chunk_pos.y.into(), chunk_pos.z.into());

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
            },
        }
    }
}

pub struct ChunkMesh {
    pub name: String,
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub num_elements: u32,
    pub chunk_pos_buffer: Buffer,
    pub chunk_pos: BindGroup,
}

impl ChunkMesh {
    pub fn new(mesh_raw: ChunkMeshRaw, device: &Device) -> Self {
        let chunk_pos_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[mesh_raw.chunk_pos.x, mesh_raw.chunk_pos.y, mesh_raw.chunk_pos.z, 0]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        Self {
            name: mesh_raw.name,
            num_elements: mesh_raw.indices.len() as u32,
            vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&mesh_raw.vertices),
                usage: BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&mesh_raw.indices),
                usage: BufferUsages::INDEX,
            }),
            chunk_pos: device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: {
                    &device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[BindGroupLayoutEntry {
                            binding: 0,
                            visibility: ShaderStages::VERTEX,
                            ty: BindingType::Buffer {
                                ty: BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }],
                    })
                },
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: chunk_pos_buffer.as_entire_binding(),
                }],
            }),
            chunk_pos_buffer,
        }
    }
}

impl Draw for ChunkMesh {
    fn draw<'a>(
        &'a self,
        material: &'a Material,
        camera_bind_group: &'a BindGroup,
        settings_bind_group: &'a BindGroup,
        render_pass: &mut RenderPass<'a>,
    ) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint32);
        render_pass.set_bind_group(0, &material.bind_group, &[]);
        render_pass.set_bind_group(1, camera_bind_group, &[]);
        render_pass.set_bind_group(2, settings_bind_group, &[]);
        render_pass.set_bind_group(3, &self.chunk_pos, &[]);
        render_pass.draw_indexed(0..self.num_elements, 0, 0..1);
    }
}

#[derive(Clone, Debug)]
pub struct MeshBuffer {
    pub solid_mesh: ChunkMeshRaw,
    pub transparent_mesh: ChunkMeshRaw,
}

impl MeshBuffer {
    pub fn new(
        chunk_pos: &Vector3<NonZeroI32>,
        surrounding_blocks: [Arc<BlockBuffer>; 7],
        surrounding_lights: [Arc<LightBuffer>; 7],
        texture_atlas: &TextureAtlas,
        transparency: bool,
        reused_buffers: &mut (GreedyQuadsBuffer, Vec<Voxel>),
    ) -> Self {
        let mesh = Self::generate_mesh(
            chunk_pos,
            surrounding_blocks,
            surrounding_lights,
            texture_atlas,
            transparency,
            reused_buffers,
        );

        Self {
            solid_mesh: mesh.0,
            transparent_mesh: mesh.1,
        }
    }

    const BUFFER_SIZE: usize = CHUNK_SIZE_MESHING.pow(3) as usize;
    fn generate_mesh(
        chunk_pos: &Vector3<NonZeroI32>,
        surrounding_blocks: [Arc<BlockBuffer>; 7],
        surrounding_lights: [Arc<LightBuffer>; 7],
        texture_atlas: &TextureAtlas,
        transparency: bool,
        reused_buffers: &mut (GreedyQuadsBuffer, Vec<Voxel>),
    ) -> (ChunkMeshRaw, ChunkMeshRaw) {
        let faces = RIGHT_HANDED_Y_UP_CONFIG.faces;

        if !surrounding_blocks[index_from_relative_pos_surrounding(&Vector3::new(0, 0, 0)) as usize]
            .contains_rendered_blocks()
        {
            return (
                ChunkMeshRaw::new(
                    format!("ChunkMesh - Solid {chunk_pos:?}"),
                    Vec::new(),
                    Vec::new(),
                    *chunk_pos,
                ),
                ChunkMeshRaw::new(
                    format!("ChunkMesh - Transparent {chunk_pos:?}"),
                    Vec::new(),
                    Vec::new(),
                    *chunk_pos,
                ),
            );
        }

        {
            for x in -1..CHUNK_SIZE_MESHING as i32 - 1 {
                for y in -1..CHUNK_SIZE_MESHING as i32 - 1 {
                    for z in -1..CHUNK_SIZE_MESHING as i32 - 1 {
                        let in_chunk_pos = Vector3::new(x, y, z);
                        reused_buffers.1
                            [ChunkShapeMeshing::linearize([(x + 1) as u32, (y + 1) as u32, (z + 1) as u32]) as usize] = {
                            let block = if let Some((chunk_pos, in_chunk_pos)) =
                                coordinate_in_surrounding_buffers(in_chunk_pos)
                            {
                                surrounding_blocks[index_from_relative_pos_surrounding(&chunk_pos) as usize]
                                    [&in_chunk_pos]
                                    .clone()
                            } else {
                                Block::default()
                            };

                            let face_lighting = {
                                if block.is_rendered() {
                                    Some(if let Some(light_source) = block.light_source() {
                                        [light_source.light_raw(); 6]
                                    } else {
                                        {
                                            let mut face_lighting: [MaybeUninit<_>; 6] =
                                                unsafe { MaybeUninit::uninit().assume_init() };

                                            FaceDirection::iter().for_each(|face| {
                                                face_lighting[face.as_index()] = MaybeUninit::new(
                                                    if let Some((chunk_pos, in_chunk_pos)) =
                                                        coordinate_in_surrounding_buffers(in_chunk_pos + face.as_dir())
                                                    {
                                                        surrounding_lights
                                                            [index_from_relative_pos_surrounding(&chunk_pos) as usize]
                                                            [&in_chunk_pos]
                                                            .light_raw()
                                                    } else {
                                                        LightVal::default().light_raw()
                                                    },
                                                )
                                            });

                                            unsafe { mem::transmute(face_lighting) }
                                        }
                                    })
                                } else {
                                    None
                                }
                            };

                            Voxel::new(&block, face_lighting)
                        }
                    }
                }
            }
        }

        reused_buffers.0.reset(MeshBuffer::BUFFER_SIZE);
        greedy_quads(
            &reused_buffers.1,
            &ChunkShapeMeshing {},
            [0; 3],
            [CHUNK_SIZE_MESHING - 1; 3],
            &faces,
            &mut reused_buffers.0,
        );

        let (num_indices, num_vertices) = (
            reused_buffers.0.quads.num_quads() * 6,
            reused_buffers.0.quads.num_quads() * 4,
        );
        let (mut solid_indices, mut solid_vertices) =
            (Vec::with_capacity(num_indices), Vec::with_capacity(num_vertices));
        let (mut transparent_indices, mut transparent_vertices) = (Vec::new(), Vec::new());

        for (group, face) in reused_buffers.0.quads.groups.iter().zip(faces.into_iter()) {
            for quad in group.iter() {
                solid_indices.extend_from_slice(&face.quad_mesh_indices(solid_vertices.len() as u32));
                transparent_indices.extend_from_slice(&face.quad_mesh_indices(transparent_vertices.len() as u32));

                let quad_mesh_poses = face.quad_mesh_positions(quad, 1.0);
                let normals = face.quad_mesh_normals();

                let pos = Vector3::new(quad.minimum[0] as i32, quad.minimum[1] as i32, quad.minimum[2] as i32);
                let voxel = reused_buffers.1
                    [ChunkShapeMeshing::linearize([(pos.x) as u32, (pos.y) as u32, (pos.z) as u32]) as usize]
                    .clone();

                for i in 0..4 {
                    let mesh_pos = quad_mesh_poses[i];
                    let normal = {
                        let normal = normals[i];
                        [normal[0] as i8, normal[1] as i8, normal[2] as i8, 0]
                    };

                    let face_direction =
                        FaceDirection::from_dir(&Vector3::new(normal[0] as i32, normal[1] as i32, normal[2] as i32))
                            .unwrap();

                    let light_color = voxel.face_lighting().unwrap()[face_direction.as_index()];

                    let texture_atlas_pos = {
                        if let Some(textures) = voxel.texture() {
                            let atlas_pos = texture_atlas.texture_coordinates(match textures {
                                Either::Left(texture) => texture,
                                Either::Right([texture_top, texture_side, texture_bottom]) => {
                                    match face_direction.into() {
                                        SideDirection::Top => texture_top,
                                        SideDirection::Side => texture_side,
                                        SideDirection::Bottom => texture_bottom,
                                    }
                                }
                            });
                            [atlas_pos.0, atlas_pos.1]
                        } else {
                            log::warn!("Creating vertex without texture");
                            [0.0, 0.0]
                        }
                    };

                    let pos = {
                        let temp_pos = [mesh_pos[0] as i32 - 1, mesh_pos[1] as i32 - 1, mesh_pos[2] as i32 - 1];
                        [temp_pos[0] as u8, temp_pos[1] as u8, temp_pos[2] as u8, 0]
                    };

                    if transparency && voxel.is_transparent() {
                        transparent_indices.extend_from_slice(&face.quad_mesh_indices(solid_vertices.len() as u32));
                        transparent_vertices.push(BlockVertex {
                            pos,
                            normal,
                            color: light_color,
                            texture_atlas_pos,
                            brightness: face_direction.brightness(),
                            transparency: 1,
                        })
                    } else {
                        solid_vertices.push(BlockVertex {
                            pos,
                            normal,
                            color: light_color,
                            texture_atlas_pos,
                            brightness: face_direction.brightness(),
                            transparency: 0,
                        })
                    }
                }
            }
        }

        (
            ChunkMeshRaw::new(
                format!("ChunkMesh - Solid {chunk_pos:?}"),
                solid_vertices,
                solid_indices,
                *chunk_pos,
            ),
            ChunkMeshRaw::new(
                format!("ChunkMesh - Transparent {chunk_pos:?}"),
                transparent_vertices,
                transparent_indices,
                *chunk_pos,
            ),
        )
    }
}
