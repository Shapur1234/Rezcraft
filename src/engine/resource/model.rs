use wgpu::{BindGroup, RenderPass, VertexBufferLayout};

use crate::engine::resource::texture;

pub trait Vertex {
    fn desc<'a>() -> VertexBufferLayout<'a>;
}

pub struct Material {
    pub name: String,
    pub diffuse_texture: texture::Texture,
    pub bind_group: BindGroup,
}

pub trait Draw {
    fn draw<'a>(
        &'a self,
        material: &'a Material,
        camera_bind_group: &'a BindGroup,
        settings_bind_group: &'a BindGroup,
        render_pass: &mut RenderPass<'a>,
    );
}
