use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    let path = if let Ok(home) = std::env::var("HEXAPOD_HOME") {
        PathBuf::from(home).join("config")
    } else {
        PathBuf::from("config")
    };
    println!("Using config directory: {:?}", path);
    path
}

fn main() {
    let _ = config_dir();
}
