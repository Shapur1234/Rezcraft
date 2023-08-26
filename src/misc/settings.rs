use cfg_if::cfg_if;
use serde::{Deserialize, Serialize};

use crate::TITLE;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub vertical_fov: f32,
    pub render_distance_horizontal: u32,
    pub render_distance_vertical: u32,
    pub camera_speed: f32,
    pub camera_sensitivity: f32,
    pub collision: bool,
    pub show_crosshair: bool,
    pub show_performance: bool,
    pub show_camera: bool,
    pub show_working: bool,
    pub sky_color: [f32; 3],
    pub sunlight_intensity: u8,
    pub base_light_value: f32,
    pub light_power_factor: f32,
}

impl Settings {
    pub fn load_from_file() -> Self {
        cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                if let Some(Ok(settings_text)) = wasm_cookies::get(&format!("{TITLE}_settings")) {
                    if let Ok(settings) = serde_yaml::from_str(&settings_text) {
                        settings
                    } else {
                        Settings::default()
                    }
                } else {
                    Settings::default()
                }
            } else {
                match confy::load(TITLE, Some(TITLE)) {
                    Ok(settings) => settings,
                    Err(e) => {
                        log::error!("Failed to load config from file - {}", e);
                        Settings::default()
                    }
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn reload(&mut self) {
        *self = Settings::load_from_file();
    }

    pub fn save(&self) {
        cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                use wasm_cookies::{CookieOptions, SameSite};

                if let Ok(settings_text) = serde_yaml::to_string(self) {
                    wasm_cookies::set(&format!("{TITLE}_settings"), &settings_text, &CookieOptions::default().with_same_site(SameSite::Strict).expires_after(core::time::Duration::from_secs(31536000)))
                }
            } else {
                if let Err(e) = confy::store(TITLE, Some(TITLE), self.clone()) {
                    log::error!("Failed to load config from file - {}", e)
                }
            }
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            render_distance_horizontal: if cfg!(debug_assertions) { 2 } else { 8 },
            render_distance_vertical: if cfg!(debug_assertions) { 2 } else { 4 },
            camera_speed: 10.0,
            camera_sensitivity: if cfg!(not(target_arch = "wasm32")) { 0.5 } else { 0.2 },
            collision: true,
            vertical_fov: 50.0,
            show_crosshair: true,
            show_performance: true,
            show_camera: true,
            show_working: true,
            sky_color: [0.1, 0.2, 0.3],
            sunlight_intensity: 12,
            base_light_value: 0.003,
            light_power_factor: 1.6,
        }
    }
}
