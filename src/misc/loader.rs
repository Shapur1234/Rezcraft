use std::path::Path;

use cfg_if::cfg_if;

#[cfg(target_arch = "wasm32")]
fn format_url(file_name: &str) -> reqwest::Url {
    let window = web_sys::window().unwrap();
    let location = window.location();

    reqwest::Url::parse(&format!("{}{}", location.href().unwrap(), file_name)).unwrap()

    // Dirty hack for gh pages
    // reqwest::Url::parse(&format!("https://shapur1234.github.io/Rezcraft-Demo/{}", file_name)).unwrap()
}

#[allow(dead_code)]
pub async fn load_string_async(path: impl AsRef<Path>) -> Result<String, ()> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(
                if let Some(path) = path.as_ref().to_str() {
                    path
                } else {
                    return Err(())
                }
            );

            if let Ok(request) = reqwest::get(url).await {
                if let Ok(txt) = request.text().await {
                    Ok(txt)
                } else {
                    Err(())
                }
            } else {
                Err(())
            }
        } else {
            let path = Path::new("./")
                .join(path);

            if let Ok(txt) = std::fs::read_to_string(path) {
                Ok(txt)
            } else {
                Err(())
            }
        }
    }
}

#[allow(dead_code)]
pub async fn load_binary_async(path: impl AsRef<Path>) -> Result<Vec<u8>, ()> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(
                if let Some(path) = path.as_ref().to_str() {
                    path
                } else {
                    return Err(())
                }
            );

            if let Ok(request) = reqwest::get(url).await {
                if let Ok(bytes) = request.bytes().await {
                    Ok(bytes.to_vec())
                } else {
                    Err(())
                }
            } else {
                Err(())
            }
        } else {
            let path = Path::new("./")
                .join(path);

            if let Ok(data) = std::fs::read(path) {
                Ok(data)
            } else {
                Err(())
            }
        }
    }
}

#[allow(dead_code)]
#[cfg(not(target_arch = "wasm32"))]
pub fn load_string(path: impl AsRef<Path>) -> Result<String, ()> {
    pollster::block_on(load_string_async(path))
}

#[allow(dead_code)]
#[cfg(not(target_arch = "wasm32"))]
pub fn load_binary(path: impl AsRef<Path>) -> Result<Vec<u8>, ()> {
    pollster::block_on(load_binary_async(path))
}
