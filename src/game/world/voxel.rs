use either::Either;

use crate::game::world::{Block, TextureID};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Voxel {
    texture: Option<Either<TextureID, [TextureID; 3]>>,
    face_lighting: Option<[[u8; 4]; 6]>,
    is_renderer: bool,
    is_opaque: bool,
    is_transparent: bool,
}

impl Voxel {
    pub fn new(block: &Block, face_lighting: Option<[[u8; 4]; 6]>) -> Self {
        Self {
            texture: block.texture_id().to_owned(),
            is_renderer: block.is_rendered(),
            is_opaque: block.is_opaque(),
            is_transparent: block.is_transparent(),
            face_lighting,
        }
    }

    pub const fn texture(&self) -> Option<&Either<TextureID, [TextureID; 3]>> {
        self.texture.as_ref()
    }

    pub const fn face_lighting(&self) -> Option<[[u8; 4]; 6]> {
        self.face_lighting
    }

    pub fn is_renderer(&self) -> bool {
        self.is_renderer
    }

    pub const fn is_opaque(&self) -> bool {
        self.is_opaque
    }

    pub const fn is_transparent(&self) -> bool {
        self.is_transparent
    }
}

impl block_mesh::Voxel for Voxel {
    fn get_visibility(&self) -> block_mesh::VoxelVisibility {
        if self.is_transparent() && self.is_renderer() {
            block_mesh::VoxelVisibility::Translucent
        } else if self.is_opaque() {
            block_mesh::VoxelVisibility::Opaque
        } else {
            block_mesh::VoxelVisibility::Empty
        }
    }
}

impl block_mesh::MergeVoxel for Voxel {
    type MergeValue = Voxel;

    fn merge_value(&self) -> Self::MergeValue {
        self.clone()
    }
}
