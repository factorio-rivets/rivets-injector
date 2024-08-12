#![allow(dead_code)]

use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

use configparser::ini::Ini;
use mod_util::{
    mod_list::{ModList, ModListError},
    mod_loader::ModError,
};

struct CfgFile {
    map: HashMap<String, String>,
}

impl CfgFile {
    fn load(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let file = std::fs::read_to_string(path)?;

        let res = file
            .lines()
            .filter_map(|line| {
                if line.is_empty() || line.starts_with('#') || !line.contains('=') {
                    return None;
                }

                let parts = line.split('=').collect::<Vec<_>>();

                if parts.len() != 2 {
                    return None;
                }

                Some((parts[0].to_string(), parts[1].to_string()))
            })
            .collect();

        Ok(Self { map: res })
    }
}

impl std::ops::Deref for CfgFile {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to load config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse config file: {0}")]
    Config(String),

    #[error("Missing required config key: {0}")]
    MissingKey(String),

    #[error("Failed to resolve path: {0}")]
    PathResolve(String),
}

fn resolve_path(path: &str, bin_path: impl AsRef<Path>) -> Result<PathBuf, ConfigError> {
    let mut buf = PathBuf::from(path);
    let first = buf
        .components()
        .next()
        .ok_or(ConfigError::PathResolve(path.to_owned()))?;

    'resolver: {
        // this unwrap is safe since we matched the first component against the prefix
        #[allow(clippy::unwrap_used)]
        if let Component::Normal(first) = first {
            let prefix = first.to_str().unwrap_or_default();
            let base = match prefix {
                "__PATH__system-write-data__" => todo!(),
                "__PATH__system-read-data__" => todo!(),
                "__PATH__executable__" => bin_path.as_ref().to_path_buf(),
                _ => break 'resolver,
            };

            buf = base.join(buf.strip_prefix(prefix).unwrap());
        }
    }

    Ok(buf)
}

pub fn get_data_dirs(bin_path: impl AsRef<Path>) -> Result<(PathBuf, PathBuf), ConfigError> {
    let config_path = CfgFile::load(bin_path.as_ref().join("../../config-path.cfg"))?;
    let config_path = config_path
        .get("config-path")
        .ok_or(ConfigError::MissingKey("config-path".to_string()))?;

    let mut config = Ini::new();
    config
        .load(resolve_path(config_path, &bin_path)?.join("config.ini"))
        .map_err(ConfigError::Config)?;

    let read_path = config
        .get("path", "read-data")
        .ok_or(ConfigError::MissingKey("read-data".to_string()))?;
    let write_path = config
        .get("path", "write-data")
        .ok_or(ConfigError::MissingKey("write-data".to_string()))?;

    Ok((
        resolve_path(&read_path, &bin_path)?.canonicalize()?,
        resolve_path(&write_path, bin_path)?.canonicalize()?,
    ))
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("{0}")]
    ModList(#[from] ModListError),

    #[error("{0}")]
    Mod(#[from] ModError),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("Rivets is not enabled")]
    RivetsNotEnabled,
}

pub fn extract_rivets_lib(
    read_data: impl AsRef<Path>,
    write_data: impl AsRef<Path>,
) -> Result<PathBuf, ExtractError> {
    #[cfg(target_os = "linux")]
    static RIVETS_LIB: &str = "rivets.so";
    #[cfg(target_os = "windows")]
    static RIVETS_LIB: &str = "rivets.dll";

    let mod_list = ModList::generate_custom(read_data, &write_data)?;

    let Some(rivets) = mod_list.load_mod("rivets")? else {
        return Err(ExtractError::RivetsNotEnabled);
    };

    let lib = rivets.get_file(RIVETS_LIB)?;

    std::fs::create_dir_all(write_data.as_ref().join("temp/rivets"))?;

    let lib_path = write_data.as_ref().join("temp/rivets").join(RIVETS_LIB);
    std::fs::write(&lib_path, lib)?;

    Ok(lib_path)
}

#[derive(Debug, thiserror::Error)]
pub enum BinFolderError {
    #[error("Failed to get binary path: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to get binary folder")]
    BinFolder,
}

pub fn get_bin_folder() -> Result<PathBuf, BinFolderError> {
    std::env::current_exe()?
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or(BinFolderError::BinFolder)
}
