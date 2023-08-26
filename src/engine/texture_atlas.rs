use std::{collections::HashMap, path::Path};

use image::{DynamicImage, ImageBuffer, Rgb, RgbImage};

use crate::{engine::resource::Texture, game::world::TextureID, misc::loader::load_binary_async};

pub struct TextureAtlas {
    texture_buffer: ImageBuffer<Rgb<u8>, Vec<u8>>,
    offset: HashMap<TextureID, (u32, u32)>,
    atlas_size: (u32, u32),
}

impl TextureAtlas {
    pub async fn new(texture_names: &[String], texture_folder: &impl AsRef<Path>) -> Self {
        let mut images: HashMap<&str, ImageBuffer<Rgb<u8>, Vec<u8>>> = HashMap::default();

        let (mut last_width, mut last_height) = (0, 0);
        for texture_name in texture_names {
            let img = load_image(texture_name.clone(), texture_folder).await;

            if last_width != 0 && last_height != 0 {
                assert!(
                    (last_width == img.width()) && (last_height == img.height()),
                    "All textures must have same size"
                );
            }

            last_width = img.width();
            last_height = img.height();

            images.insert(texture_name, img);
        }

        let texture_width = (images.len() as f32).sqrt().ceil() as u32;
        let texture_height = texture_width;

        let mut offset = HashMap::default();
        let mut images_iter = images.into_iter();
        let mut texture_buffer = RgbImage::new(texture_width * last_width, texture_height * last_height);

        for x in 0..texture_width {
            for y in 0..texture_height {
                if let Some((texture_name, image)) = images_iter.next() {
                    offset.insert(texture_name.into(), (x, y));
                    for image_x in 0..image.width() {
                        for image_y in 0..image.height() {
                            texture_buffer.put_pixel(
                                image_x + x * image.width(),
                                image_y + y * image.height(),
                                *image.get_pixel(image_x, image_y),
                            )
                        }
                    }
                }
            }
        }

        Self {
            texture_buffer,
            offset,
            atlas_size: (texture_width, texture_height),
        }
    }

    pub fn load_texture(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Texture {
        Texture::from_image(
            device,
            queue,
            &DynamicImage::ImageRgb8(self.texture_buffer.clone()),
            Some("TextureAtlas"),
        )
        .expect("Failed creating TextureAtlas")
    }

    pub fn texture_coordinates(&self, texture: &TextureID) -> (f32, f32) {
        let coords = self.offset[texture];
        (
            coords.0 as f32 / self.atlas_size.0 as f32,
            coords.1 as f32 / self.atlas_size.1 as f32,
        )
    }

    pub fn atlas_size(&self) -> (f32, f32) {
        (self.atlas_size.0 as f32, self.atlas_size.1 as f32)
    }

    pub fn tile_size(&self) -> (f32, f32) {
        let atlas_size = self.atlas_size();
        (1.0 / atlas_size.0, 1.0 / atlas_size.1)
    }

    pub fn clone_without_image(&self) -> Self {
        Self {
            texture_buffer: ImageBuffer::new(1, 1),
            offset: self.offset.clone(),
            atlas_size: self.atlas_size,
        }
    }
}

async fn load_image(texture_name: String, texture_folder: &impl AsRef<Path>) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let path = texture_folder.as_ref().join(texture_name.clone()).with_extension("png");

    let bytes = load_binary_async(&path)
        .await
        .unwrap_or_else(|_| panic!("Failed to load texture: {texture_name:?} - {path:?}"));

    image::load_from_memory(&bytes)
        .unwrap_or_else(|_| panic!("Failed to parse {texture_name:?} - {path:?} as image"))
        .to_rgb8()
}
