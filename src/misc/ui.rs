use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use egui::{Align, Align2, Area, ComboBox, Context, CursorIcon, Layout, Order, RichText, Window};
use either::Either;

use crate::{
    game::{
        world::{Block, BlockManager, LightSource, TextureID, MAX_LIGHT_VAL},
        Player,
    },
    misc::settings::Settings,
};

pub struct UI<'a> {
    running: Arc<AtomicBool>,
    elapsed_secs: f64,
    player: Player,
    settings: &'a mut Settings,
    selected_block: &'a mut Block,
    selected_block_template: &'a mut String,
    block_manager: Rc<BlockManager>,
    loading_chunks: u32,
    saving_chunks: u32,
    selected_save: &'a mut String,
    do_save: &'a mut bool,
    do_load: &'a mut bool,
}

impl<'a> UI<'a> {
    pub fn new(
        running: Arc<AtomicBool>,
        elapsed_secs: f64,
        player: Player,
        settings: &'a mut Settings,
        selected_block: &'a mut Block,
        selected_block_template: &'a mut String,
        block_manager: Rc<BlockManager>,
        loading_chunks: u32,
        saving_chunks: u32,
        selected_save: &'a mut String,
        do_save: &'a mut bool,
        do_load: &'a mut bool,
    ) -> Self {
        Self {
            running,
            elapsed_secs,
            player,
            settings,
            selected_block,
            selected_block_template,
            block_manager,
            loading_chunks,
            saving_chunks,
            selected_save,
            do_save,
            do_load,
        }
    }

    fn show_camera(&mut self, ctx: &Context) {
        Window::new("Camera")
            .title_bar(false)
            .anchor(Align2::LEFT_TOP, [4.0, 4.0])
            .show(ctx, |ui| {
                let cam_pos = self.player.camera.pos.abs_pos();
                ui.label(format!("Pos: ({:.2}, {:.2}, {:.2})", cam_pos.x, cam_pos.y, cam_pos.z,));

                ui.label(format!(
                    "Rotation: ({:?}, {:?})",
                    self.player.camera.yaw(),
                    self.player.camera.pitch()
                ));

                let chunk_pos = self.player.camera.pos.chunk_pos();
                ui.label(format!(
                    "Chunk pos: ({}, {}, {})",
                    chunk_pos.x, chunk_pos.y, chunk_pos.z,
                ));
                let in_chunk_pos = self.player.camera.pos.in_chunk_pos_f32();
                ui.label(format!(
                    "InChunk pos: ({:.2}, {:.2}, {:.2})",
                    in_chunk_pos.x, in_chunk_pos.y, in_chunk_pos.z,
                ));
            });
    }

    fn show_crosshair(&mut self, ctx: &Context) {
        if self.settings.show_crosshair {
            Area::new("Crosshair")
                .order(Order::TOP)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(RichText::new("+").strong().size(20.0));
                });
        }
    }

    fn show_saves(&mut self, ctx: &Context) {
        #[cfg(not(target_arch = "wasm32"))]
        Window::new("Saves")
            .collapsible(false)
            .default_width(0.01)
            .default_height(0.01)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let current_selected = self.selected_save.clone();

                        ui.label("Select save:");
                        egui::ComboBox::from_label("")
                            .selected_text(format!("{:?}", current_selected))
                            .show_ui(ui, |ui| {
                                let available_saves = crate::misc::save_helper::available_saves();

                                if !available_saves.contains(&self.selected_save.clone()) {
                                    ui.selectable_value(
                                        self.selected_save,
                                        self.selected_save.clone(),
                                        RichText::new(format!("{:?}", self.selected_save.clone())).italics(),
                                    );
                                }
                                for save_name in available_saves {
                                    ui.selectable_value(
                                        self.selected_save,
                                        save_name.clone(),
                                        format!("{:?}", save_name),
                                    );
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Rename save:");
                        ui.text_edit_singleline(self.selected_save);
                    });
                });

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        *self.do_save = ui.button("Save").clicked();
                        *self.do_load = ui.button("Load").clicked();
                    });
                });
            });
    }

    fn show_performance(&mut self, ctx: &Context) {
        Window::new("Performance")
            .title_bar(false)
            .anchor(Align2::RIGHT_TOP, [-4.0, 4.0])
            .show(ctx, |ui| {
                let frame_time = self.elapsed_secs * 1000.0;
                let fps = 1.0 / self.elapsed_secs;

                ui.label(format!("FPS: {:.2}", fps));
                ui.label(format!("Frametime: {:.2} ms", frame_time));
            });
    }

    fn show_resume(&mut self, ctx: &Context) {
        Area::new("Paused")
            .order(Order::TOP)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                self.running.store(
                    ui.button(RichText::new("RESUME").heading()).clicked() ^ self.running.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );

                #[cfg(target_arch = "wasm32")]
                if self.running.load(Ordering::Relaxed) {
                    crate::misc::wasm::request_pointer_lock()
                }
            });
    }

    fn show_edit_block(&mut self, ctx: &Context) {
        Window::new("Edit block").collapsible(false).default_width(0.01).default_height(0.01).show(ctx, |ui| {
            ui.group(|ui| {
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    ui.label("Template");
                });

                ComboBox::from_label("Select template").selected_text(self.selected_block_template.to_owned()).show_ui(ui, |ui| {
                    for block_name in self.block_manager.all_rendered_block_names() {
                        ui.selectable_value(self.selected_block_template, block_name.clone(), block_name);
                    }
                });

                if ui.button("Load template").clicked() {
                    *self.selected_block = Block::new_with_default(self.selected_block_template, self.block_manager.as_ref())
                }
            });

            if let Some(textures) = self.selected_block.texture_id() {
                match textures {
                    Either::Left(texture_id) => {
                        if let Some(mut texture_name) = self.block_manager.get_texture_name(texture_id) {
                            ui.group(|ui| {
                                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                                    ui.label("Texture");
                                });

                                ComboBox::from_label("Select texture").selected_text(texture_name) .show_ui(ui, |ui| {
                                    for possible_texture_name in self.block_manager.all_texture_names() {
                                        ui.selectable_value(&mut texture_name,possible_texture_name, possible_texture_name );
                                    }
                                });

                                ui.separator();

                                if ui.button("Make sides have different textures").clicked() {
                                    self.selected_block.set_texture_id(Some(Either::Right([TextureID::from(texture_name.as_str()), TextureID::from(texture_name.as_str()),TextureID::from(texture_name.as_str())])))
                                } else {
                                    self.selected_block.set_texture_id(Some(Either::Left(TextureID::from(texture_name.as_str()))))
                                }
                            });
                        }
                    }
                    Either::Right([texture_id_top, texture_id_side, texture_id_bottom]) => {
                        if let [Some(mut texture_name_top), Some(mut texture_name_side), Some(mut texture_name_bottom)] = [
                            self.block_manager.get_texture_name(texture_id_top),
                            self.block_manager.get_texture_name(texture_id_side),
                            self.block_manager.get_texture_name(texture_id_bottom),
                        ] {
                            ui.group(|ui| {
                                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                                    ui.label("Textures");
                                });

                                ComboBox::from_label("Select top texture") .selected_text(texture_name_top) .show_ui(ui, |ui| {
                                    for possible_texture_name in self.block_manager.all_texture_names() {
                                        ui.selectable_value(&mut texture_name_top, possible_texture_name, possible_texture_name);
                                    }
                                });
                                ComboBox::from_label("Select side texture") .selected_text(texture_name_side) .show_ui(ui, |ui| {
                                    for possible_texture_name in self.block_manager.all_texture_names() {
                                        ui.selectable_value(&mut texture_name_side, possible_texture_name, possible_texture_name);
                                    }
                                });
                                ComboBox::from_label("Select bottom texture") .selected_text(texture_name_bottom) .show_ui(ui, |ui| {
                                    for possible_texture_name in self.block_manager.all_texture_names() {
                                        ui.selectable_value(&mut texture_name_bottom, possible_texture_name, possible_texture_name);
                                    }
                                });

                                ui.separator();

                                if ui.button("Make sides share a texture").clicked() {
                                    self.selected_block.set_texture_id(Some(Either::Left(TextureID::from(texture_name_top.as_str()))))
                                } else {
                                    self.selected_block.set_texture_id(Some(Either::Right([TextureID::from(texture_name_top.as_str()), TextureID::from(texture_name_side.as_str()),TextureID::from(texture_name_bottom.as_str())])))
                                }

                            });
                        }
                    }
                }

                ui.group(|ui| {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.label("Light source");
                    });

                    if let Some(light_source) = self.selected_block.light_source_mut() {
                        let light_source_old = light_source.clone();

                        ui.horizontal(|ui| {
                            ui.checkbox(&mut light_source.red, "Red");
                            ui.checkbox(&mut light_source.green, "Green");
                            ui.checkbox(&mut light_source.blue, "Blue");
                        });
                        ui.add(egui::Slider::new(&mut light_source.strength, 1..=MAX_LIGHT_VAL).text("Light strength"));

                        ui.separator();

                        if !light_source.is_valid() {
                            if light_source_old.is_valid() {
                                self.selected_block.set_light_source(Some(light_source_old));
                            } else {
                                self.selected_block.set_light_source(Some(LightSource::default()))
                            }
                        }

                        if ui.button("Remove light source").clicked() {
                            self.selected_block.set_light_source(None)
                        }
                    } else {
                        if ui.button("Add light source").clicked() {
                            self.selected_block.set_light_source(Some(LightSource::default()))
                        }
                    }
                });

                {
                    ui.group(|ui| {
                        ui.with_layout(Layout::top_down(Align::Center), |ui| {
                            ui.label("Properties");
                        });

                        ui.checkbox(&mut self.selected_block.is_transparent_mut(), "Transparent");
                        ui.checkbox(&mut self.selected_block.is_solid_mut(), "Solid");
                    });
                }
            }
        });
    }

    fn show_settings(&mut self, ctx: &Context) {
        Window::new("Settings")
            .collapsible(false)
            .default_width(0.01)
            .default_height(0.01)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.label("Render distance");
                    });

                    ui.add(
                        egui::Slider::new(&mut self.settings.render_distance_horizontal, 2..=32)
                            .text("Horizontal radius"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.settings.render_distance_vertical, 2..=32).text("Vertical radius"),
                    );
                });

                ui.group(|ui| {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.label("Camera");
                    });

                    ui.add(egui::Slider::new(&mut self.settings.camera_speed, 1.0..=100.0).text("Movement speed"));
                    ui.add(
                        egui::Slider::new(&mut self.settings.camera_sensitivity, 0.01..=5.0).text("Mouse sensitivity"),
                    );
                    ui.add(egui::Slider::new(&mut self.settings.vertical_fov, 1.0..=179.0).text("Vertical FOV"));
                });

                ui.group(|ui| {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.label("Physics");
                    });

                    ui.checkbox(&mut self.settings.collision, "Collision detection");
                });

                ui.group(|ui| {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.label("UI");
                    });

                    ui.checkbox(&mut self.settings.show_crosshair, "Show Crosshair");
                    ui.checkbox(&mut self.settings.show_performance, "Show Performance info");
                    ui.checkbox(&mut self.settings.show_camera, "Show Camera info");
                    ui.checkbox(&mut self.settings.show_working, "Show Progress when loading / saving");
                });

                ui.group(|ui| {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.label("Rendering");
                    });

                    ui.horizontal(|ui| {
                        ui.label("Sky color");
                        egui::widgets::color_picker::color_edit_button_rgb(ui, &mut self.settings.sky_color);
                    });
                    ui.add(egui::Slider::new(&mut self.settings.sunlight_intensity, 0..=15).text("Sunlight intensity"));
                    ui.add(egui::Slider::new(&mut self.settings.base_light_value, 0.0..=0.1).text("Base light value"));
                    ui.add(
                        egui::Slider::new(&mut self.settings.light_power_factor, 1.0..=2.0).text("Light power factor"),
                    );
                });
            });
    }

    fn show_working(&mut self, ctx: &Context) {
        Window::new("Working...")
            .collapsible(false)
            .anchor(Align2::LEFT_BOTTOM, [4.0, -4.0])
            .show(ctx, |ui| {
                if self.saving_chunks > 0 {
                    ui.label(format!("Saving {} chunks...", self.saving_chunks));
                }
                if self.loading_chunks > 0 {
                    ui.label(format!("Loading {} chunks...", self.loading_chunks));
                }
            });
    }
}

impl<'a> crate::engine::GUI for UI<'a> {
    fn show_ui(&mut self, ctx: &Context) {
        ctx.set_cursor_icon(if self.running.load(Ordering::Relaxed) {
            CursorIcon::None
        } else {
            CursorIcon::default()
        });

        if self.settings.show_working && (self.saving_chunks > 0 || self.loading_chunks > 26) {
            self.show_working(ctx);
        }

        if self.settings.show_performance {
            self.show_performance(ctx);
        }
        if self.settings.show_camera {
            self.show_camera(ctx);
        }

        if self.running.load(Ordering::Relaxed) {
            self.show_crosshair(ctx);
        } else {
            self.show_resume(ctx);
            self.show_settings(ctx);
            self.show_saves(ctx);
            self.show_edit_block(ctx);
        }
    }

    fn elapsed_secs(&self) -> f64 {
        self.elapsed_secs
    }
}
