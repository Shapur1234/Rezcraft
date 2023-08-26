use std::{
    collections::BTreeSet,
    fs::File,
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use rayon::prelude::*;
use serde::Serialize;

use crate::{
    game::{world::BlockBuffer, Player},
    misc::loader::{load_binary, load_string},
};

const SAVE_DIR: &str = "./saves";

pub fn available_saves() -> BTreeSet<String> {
    match std::fs::read_dir(SAVE_DIR) {
        Ok(paths) => paths
            .into_iter()
            .filter_map(|dir_entry| {
                if let Ok(dir_entry) = dir_entry {
                    if let Ok(file_type) = dir_entry.file_type() {
                        if file_type.is_dir() {
                            if let Some(file_name) = dir_entry.file_name().to_str() {
                                return Some(file_name.to_string());
                            }
                        }
                    }
                }
                None
            })
            .collect(),
        Err(e) => {
            log::warn!("Failed listing dirs in save dir {} - {}", SAVE_DIR, e);
            BTreeSet::new()
        }
    }
}

pub fn save_folder(save_name: impl ToString) -> String {
    SAVE_DIR.to_string() + "/" + &save_name.to_string() + "/"
}

pub fn save(save_path: impl ToString, file_name: impl ToString, object: &impl Serialize, save_as_bytes: bool) {
    let path_string =
        save_path.to_string() + "/" + &file_name.to_string() + if !save_as_bytes { ".yaml" } else { ".cbor" };
    let path = Path::new(&path_string);

    match File::create(path) {
        Ok(mut file) => {
            if save_as_bytes {
                if let Err(e) = ciborium::into_writer(object, file) {
                    log::warn!("Failed serializing and writing to file {} - {}", &path_string, e)
                }
            } else {
                match serde_yaml::to_string(object) {
                    Ok(to_write) => {
                        if let Err(e) = writeln!(file, "{}", to_write) {
                            log::warn!("Failed writing to file {} - {}", &path_string, e)
                        }
                    }
                    Err(e) => log::warn!("Failed to serialize - {}", e),
                }
            }
        }
        Err(_) => {
            let prefix = path.parent().unwrap();
            std::fs::create_dir_all(prefix).ok();

            match File::create(path) {
                Ok(mut file) => {
                    if save_as_bytes {
                        if let Err(e) = ciborium::into_writer(object, file) {
                            log::warn!("Failed serializing and writing to file {} - {}", &path_string, e)
                        }
                    } else {
                        match serde_yaml::to_string(object) {
                            Ok(to_write) => {
                                if let Err(e) = writeln!(file, "{}", to_write) {
                                    log::warn!("Failed writing to file {} - {}", &path_string, e)
                                }
                            }
                            Err(e) => log::warn!("Failed to serialize - {}", e),
                        }
                    }
                }
                Err(e) => {
                    std::fs::create_dir_all(prefix).ok();
                    log::warn!("Failed to open file {:?} - {}", &path, e)
                }
            }
        }
    }
}

pub fn save_many(
    save_path: impl ToString,
    objects: Vec<(impl ToString + Sync, impl Serialize + Sync)>,
    counter: Option<Arc<AtomicU32>>,
) {
    let save_path = save_path.to_string();

    if let Some(counter) = counter {
        objects.into_par_iter().for_each(move |(file_name, object)| {
            save(&save_path, file_name.to_string(), &object, true);
            counter.fetch_sub(1, Ordering::Relaxed);
        });
    } else {
        objects.into_par_iter().for_each(move |(file_name, object)| {
            save(&save_path, file_name.to_string(), &object, true);
        })
    }
}

pub fn load_player(file_path: impl ToString) -> Option<Player> {
    let file_string = file_path.to_string() + ".yaml";
    if let Ok(text) = load_string(&file_string) {
        match serde_yaml::from_str(&text) {
            Ok(player) => Some(player),
            Err(e) => {
                log::warn!("Failed deserializing Player from file {} - {}", &file_string, e);
                None
            }
        }
    } else {
        None
    }
}

pub fn load_block_buffer(file_path: impl ToString) -> Option<BlockBuffer> {
    let file_string = file_path.to_string() + ".cbor";
    if let Ok(bytes) = load_binary(&file_string) {
        match ciborium::from_reader(bytes.as_slice()) {
            Ok(block_buffer) => Some(block_buffer),
            Err(e) => {
                log::warn!("Failed deserializing Chunk from file {} - {}", &file_string, e);
                None
            }
        }
    } else {
        None
    }
}

pub fn load_u32(file_path: impl ToString) -> Option<u32> {
    let file_string = file_path.to_string() + ".yaml";
    if let Ok(text) = load_string(&file_string) {
        match serde_yaml::from_str(&text) {
            Ok(num) => Some(num),
            Err(e) => {
                log::warn!("Failed deserializing u32 from file {} - {}", &file_string, e);
                None
            }
        }
    } else {
        None
    }
}
