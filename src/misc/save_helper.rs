use std::{
    collections::BTreeSet,
    fs::File,
    io::Write,
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
    SAVES_PATH,
};

pub fn available_saves() -> BTreeSet<String> {
    match std::fs::read_dir(&*SAVES_PATH) {
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
            if let Some(str) = SAVES_PATH.to_str() {
                log::warn!("Failed listing dirs in save dir {} - {}", str, e);
            }
            BTreeSet::new()
        }
    }
}

pub fn save(save_name: impl ToString, file_name: impl ToString, object: &impl Serialize, save_as_bytes: bool) {
    let path = SAVES_PATH
        .join(save_name.to_string())
        .join(file_name.to_string() + if !save_as_bytes { ".yaml" } else { ".cbor" });

    match File::create(path.clone()) {
        Ok(mut file) => {
            if save_as_bytes {
                if let Err(e) = ciborium::into_writer(object, file) {
                    log::warn!("Failed serializing and writing to file {} - {}", path.display(), e)
                }
            } else {
                match serde_yaml::to_string(object) {
                    Ok(to_write) => {
                        if let Err(e) = writeln!(file, "{}", to_write) {
                            log::warn!("Failed writing to file {} - {}", path.display(), e)
                        }
                    }
                    Err(e) => log::warn!("Failed to serialize - {}", e),
                }
            }
        }
        Err(_) => {
            let prefix = path.parent().unwrap();
            std::fs::create_dir_all(prefix).ok();

            match File::create(path.clone()) {
                Ok(mut file) => {
                    if save_as_bytes {
                        if let Err(e) = ciborium::into_writer(object, file) {
                            log::warn!("Failed serializing and writing to file {} - {}", path.display(), e)
                        }
                    } else {
                        match serde_yaml::to_string(object) {
                            Ok(to_write) => {
                                if let Err(e) = writeln!(file, "{}", to_write) {
                                    log::warn!("Failed writing to file {} - {}", path.display(), e)
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
    save_name: impl ToString,
    directory_name: impl ToString,
    objects: Vec<(impl ToString + Sync, impl Serialize + Sync)>,
    counter: Option<Arc<AtomicU32>>,
) {
    let (save_name, directory_name) = (save_name.to_string(), directory_name.to_string());

    if let Some(counter) = counter {
        objects.into_par_iter().for_each(move |(file_name, object)| {
            save(
                &save_name,
                directory_name.clone() + "/" + &file_name.to_string(),
                &object,
                true,
            );
            counter.fetch_sub(1, Ordering::Relaxed);
        });
    } else {
        objects.into_par_iter().for_each(move |(file_name, object)| {
            save(
                &save_name,
                directory_name.clone() + "/" + &file_name.to_string(),
                &object,
                true,
            );
        })
    }
}

pub fn load_player(save_name: impl ToString, file_name: impl ToString) -> Option<Player> {
    let path = SAVES_PATH
        .join(save_name.to_string())
        .join(file_name.to_string() + ".yaml");

    if let Ok(text) = load_string(&path) {
        match serde_yaml::from_str(&text) {
            Ok(player) => Some(player),
            Err(e) => {
                log::warn!("Failed deserializing Player from file {} - {}", path.display(), e);
                None
            }
        }
    } else {
        None
    }
}

pub fn load_block_buffer(save_name: impl ToString, file_name: impl ToString) -> Option<BlockBuffer> {
    let path = SAVES_PATH
        .join(save_name.to_string())
        .join("chunks".to_string())
        .join(file_name.to_string() + ".cbor");

    if let Ok(bytes) = load_binary(&path) {
        match ciborium::from_reader(bytes.as_slice()) {
            Ok(block_buffer) => Some(block_buffer),
            Err(e) => {
                log::warn!("Failed deserializing Chunk from file {} - {}", path.display(), e);
                None
            }
        }
    } else {
        None
    }
}

pub fn load_u32(save_name: impl ToString, file_name: impl ToString) -> Option<u32> {
    let path = SAVES_PATH
        .join(save_name.to_string())
        .join(file_name.to_string() + ".yaml");

    if let Ok(text) = load_string(&path) {
        match serde_yaml::from_str(&text) {
            Ok(num) => Some(num),
            Err(e) => {
                log::warn!("Failed deserializing u32 from file {} - {}", path.display(), e);
                None
            }
        }
    } else {
        None
    }
}
