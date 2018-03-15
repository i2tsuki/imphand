use std::env;
use std::fs;
use std::path;

pub struct App {
    pub name: &'static str,
    pub version: &'static str,
    pub config_dir: path::PathBuf,
    pub config_file: path::PathBuf,
}

pub fn new() -> App {
    let name = "imphand";
    let version = "0.1.0";
    let config_dir = match env::home_dir() {
        Some(home_dir) => home_dir.join(".config").join(name),
        None => panic!("home directory is not set"),
    };
    let config_file = config_dir.clone().join("config.toml");
    fs::create_dir_all(&config_dir).ok();
    App {
        name: name,
        version: version,
        config_dir: config_dir,
        config_file: config_file,
    }
}
