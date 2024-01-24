use std::path::Path;

#[cfg(feature = "portable")]
use crate::RESOURCE_DIR;
#[cfg(not(feature = "portable"))]
use crate::RESOURCE_PATH;

pub fn load_resource_binary(path: impl AsRef<Path>) -> Result<Vec<u8>, ()> {
    #[cfg(feature = "portable")]
    {
        if let Some(file) = RESOURCE_DIR.get_file(path) {
            Ok(file.contents().to_owned())
        } else {
            Err(())
        }
    }

    #[cfg(not(feature = "portable"))]
    {
        load_binary(RESOURCE_PATH.join(path))
    }
}

pub fn load_resource_string(path: impl AsRef<Path>) -> Result<String, ()> {
    #[cfg(feature = "portable")]
    {
        if let Some(file) = RESOURCE_DIR.get_file(path) {
            if let Some(text) = file.contents_utf8() {
                Ok(text.to_owned())
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }

    #[cfg(not(feature = "portable"))]
    {
        load_string(RESOURCE_PATH.join(path))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_string(path: impl AsRef<Path>) -> Result<String, ()> {
    if let Ok(txt) = std::fs::read_to_string(path) {
        Ok(txt)
    } else {
        Err(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_binary(path: impl AsRef<Path>) -> Result<Vec<u8>, ()> {
    if let Ok(data) = std::fs::read(path) {
        Ok(data)
    } else {
        Err(())
    }
}
