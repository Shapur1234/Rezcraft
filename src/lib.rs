mod engine;
mod game;
mod misc;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[cfg(any(feature = "portable", feature = "save_system"))]
use std::{env, path::PathBuf};

use cfg_if::cfg_if;
use cgmath::{Deg, Rad};
#[cfg(any(feature = "portable", feature = "save_system"))]
use lazy_static::lazy_static;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{self, Icon},
};

#[cfg(target_arch = "wasm32")]
use crate::misc::wasm;
use crate::{
    engine::{resource::Vertex, Renderer},
    game::{
        world::{BlockManager, BlockVertex},
        State,
    },
    misc::{loader::load_resource_binary, ui::UI, Settings},
};

#[cfg(all(target_arch = "wasm32", feature = "save_system"))]
compile_error!("feature \"save_system\" cannot be used on wasm");

pub const TITLE: &'static str = "Rezcraft";
const FPS_UPDATE_INTERVAL: f64 = 0.1;

#[cfg(feature = "portable")]
pub static RESOURCE_DIR: include_dir::Dir<'_> = include_dir::include_dir!("$CARGO_MANIFEST_DIR/res");

#[cfg(not(feature = "portable"))]
lazy_static! {
    pub static ref RESOURCE_PATH: PathBuf = if let Ok(var) = env::var("RESOURCE_PATH") {
        PathBuf::from(var)
    } else {
        PathBuf::from("./res")
    };
}

#[cfg(feature = "save_system")]
lazy_static! {
    pub static ref SAVES_PATH: PathBuf = if let Ok(var) = env::var("SAVES_PATH") {
        PathBuf::from(var)
    } else {
        PathBuf::from("./saves")
    };
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn dummy_main() {}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub async fn run() {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Warn).expect("Could't initialize logger");
        } else {
            env_logger::init()
        }
    }

    let event_loop = EventLoop::new();
    let window = window::WindowBuilder::new()
        .with_title(TITLE)
        .with_min_inner_size(PhysicalSize::new(1280, 720))
        .with_maximized(true)
        .build(&event_loop)
        .unwrap();

    window.set_cursor_visible(false);
    window.set_cursor_grab(window::CursorGrabMode::Confined).ok();

    #[cfg(not(target_arch = "wasm32"))]
    match load_resource_binary("icon.png") {
        Ok(bytes) => match image::load_from_memory(&bytes) {
            Ok(img) => match Icon::from_rgba(img.to_rgba8().into_vec(), img.width(), img.height()) {
                Ok(icon) => window.set_window_icon(Some(icon)),
                Err(e) => log::error!("Failed parsing image as icon - {e:?}"),
            },
            Err(e) => log::error!("Failed parsing icon file as image - {e:?}",),
        },
        Err(e) => log::error!("Failed loading icon file - {e:?}",),
    }

    let running = Arc::new(AtomicBool::new(true));

    #[cfg(target_arch = "wasm32")]
    let window_resized = Arc::new(AtomicBool::new(false));

    #[cfg(target_arch = "wasm32")]
    {
        use winit::{dpi::PhysicalSize, platform::web::WindowExtWebSys};

        let (window_width, window_height) = wasm::window_size();
        window.set_inner_size(PhysicalSize::new(window_width, window_height));

        let canvas_element = web_sys::Element::from(window.canvas());
        canvas_element.set_id("out_canvas");

        wasm::get_element_by_id("rezcraft")
            .append_child(&canvas_element)
            .unwrap();

        wasm::register_mouse_click(running.clone());
        wasm::register_window_resize(window_resized.clone());
    }

    let block_manager = BlockManager::new();
    let mut selected_block_template = block_manager.all_rendered_block_names()[0].to_owned();

    let mut settings = Settings::load_from_file();
    let mut renderer = Renderer::<crate::game::Projection>::new(
        window,
        BlockVertex::desc(),
        block_manager.all_texture_names(),
        &"texture",
        &settings,
    )
    .await;
    let mut game_state = State::new(renderer.texture_atlas(), block_manager, false);

    let mut last_render_time = instant::Instant::now();
    let (mut dt_fps_sum, mut dt_fps, mut dt_frames_occured) = (0.0, 0.0, 0);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        renderer.egui_platform_mut().handle_event(&event);

        match event {
            Event::MainEventsCleared => renderer.window().request_redraw(),
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                if running.load(Ordering::Relaxed) {
                    game_state.input_mouse(delta)
                }
            }
            Event::WindowEvent { ref event, window_id }
                if window_id == renderer.window().id()
                    && !if running.load(Ordering::Relaxed) {
                        cfg_if! {
                            if #[cfg(target_arch = "wasm32")] {
                                if wasm::is_pointer_locked() {
                                    game_state.input(event)
                                } else {
                                    false
                                }
                            } else {
                                game_state.input(event)
                            }
                        }
                    } else {
                        false
                    } =>
            {
                match event {
                    #[cfg(not(target_arch = "wasm32"))]
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => {
                        settings.save();

                        *control_flow = ControlFlow::Exit
                    }
                    WindowEvent::Resized(physical_size) => {
                        renderer.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        renderer.resize(**new_inner_size);
                    }
                    WindowEvent::Focused(focused_gained) => {
                        if *focused_gained {
                            running.store(true, Ordering::Relaxed);
                            renderer.window().set_cursor_visible(false);
                            renderer.window().set_cursor_grab(window::CursorGrabMode::Confined).ok();
                        } else {
                            running.store(false, Ordering::Relaxed);
                            renderer.window().set_cursor_visible(true);
                            renderer.window().set_cursor_grab(window::CursorGrabMode::None).ok();
                        }
                    }
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::F11),
                                ..
                            },
                        ..
                    } => {
                        if renderer.window().fullscreen().is_none() {
                            renderer
                                .window()
                                .set_fullscreen(Some(window::Fullscreen::Borderless(None)))
                        } else {
                            renderer.window().set_fullscreen(None)
                        }
                    }
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Tab),
                                ..
                            },
                        ..
                    } => {
                        settings.save();

                        running.store(running.load(Ordering::Relaxed) ^ true, Ordering::Relaxed);
                        #[cfg(target_arch = "wasm32")]
                        {
                            if running.load(Ordering::Relaxed) {
                                wasm::request_pointer_lock();
                            } else {
                                wasm::exit_pointer_lock();
                            }
                        }
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::F12),
                                ..
                            },
                        ..
                    } => settings.reload(),
                    #[cfg(feature = "save_system")]
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::F5),
                                ..
                            },
                        ..
                    } => game_state.save(),
                    #[cfg(feature = "save_system")]
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::F9),
                                ..
                            },
                        ..
                    } => game_state.load(),
                    _ => {}
                }
            }
            Event::RedrawRequested(window_id) if window_id == renderer.window().id() => {
                #[cfg(target_arch = "wasm32")]
                {
                    if window_resized.load(Ordering::Relaxed) {
                        let (window_width, window_height) = wasm::window_size();

                        renderer
                            .window()
                            .set_inner_size(PhysicalSize::new(window_width, window_height));
                        renderer.resize(PhysicalSize::new(window_width, window_height));

                        window_resized.store(false, Ordering::Relaxed);
                    }

                    running.store(wasm::is_pointer_locked(), Ordering::Relaxed);
                }

                let now = instant::Instant::now();
                let dt = now - last_render_time;
                last_render_time = now;

                {
                    if dt_fps_sum >= FPS_UPDATE_INTERVAL {
                        dt_fps = dt_fps_sum / dt_frames_occured as f64;

                        dt_fps_sum = 0.0;
                        dt_frames_occured = 0;
                    }

                    dt_fps_sum += dt.as_secs_f64();
                    dt_frames_occured += 1;
                }

                game_state.update(running.load(Ordering::Relaxed), dt, &settings);
                renderer.update(game_state.camera(), &settings);

                let settings_clone = settings.clone();
                let mut selected_block = game_state.selected_block_mut().clone();

                let mut selected_save = {
                    #[cfg(feature = "save_system")]
                    {
                        game_state.selected_save()
                    }
                    #[cfg(not(feature = "save_system"))]
                    {
                        "".to_string()
                    }
                };

                let (mut do_save, mut do_load) = (false, false);
                let (last_vertical_fov, last_render_distance) = (
                    settings.vertical_fov,
                    (settings.render_distance_horizontal, settings.render_distance_vertical),
                );

                let mut ui = UI::new(
                    running.clone(),
                    dt_fps,
                    game_state.player().clone(),
                    &mut settings,
                    &mut selected_block,
                    &mut selected_block_template,
                    game_state.block_manager(),
                    game_state.loading_chunks(),
                    game_state.saving_chunks(),
                    &mut selected_save,
                    &mut do_save,
                    &mut do_load,
                );

                let to_render = game_state.meshes_to_render(renderer.device(), &settings_clone);
                match renderer.render(
                    to_render,
                    Some((
                        settings_clone.sky_color[0],
                        settings_clone.sky_color[1],
                        settings_clone.sky_color[2],
                    )),
                    &mut ui,
                ) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => renderer.resize(renderer.size()),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                };

                if settings.vertical_fov != last_vertical_fov {
                    renderer.set_vfov(Rad::from(Deg(settings.vertical_fov)))
                }
                if (settings.render_distance_horizontal, settings.render_distance_vertical) != last_render_distance {
                    game_state.cancel_requests()
                }

                *game_state.selected_block_mut() = selected_block;

                #[cfg(feature = "save_system")]
                {
                    game_state.set_selected_save(selected_save);

                    if do_save {
                        game_state.save();
                    }
                    if do_load {
                        game_state.load();
                    }
                }
            }
            _ => {}
        }
    });
}
