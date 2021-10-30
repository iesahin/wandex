use dirs_2;

use std::path::PathBuf;

use crate::fail::{WError, WResult};

pub fn home_path() -> WResult<PathBuf> {
    let home = dirs_2::home_dir().ok_or(WError::NoneError)?;
    Ok(home)
}

pub fn ranger_path() -> WResult<PathBuf> {
    let mut ranger_path = dirs_2::config_dir().ok_or(WError::NoneError)?;
    ranger_path.push("ranger/");
    Ok(ranger_path)
}

#[cfg(not(target_os = "macos"))]
pub fn wandex_path() -> WResult<PathBuf> {
    let mut hunter_path = dirs_2::config_dir().ok_or(WError::NoneError)?;
    hunter_path.push("wandex/");
    Ok(hunter_path)
}

#[cfg(target_os = "macos")]
pub fn wandex_path() -> WResult<PathBuf> {
    let mut hunter_path = home_path()?;
    hunter_path.push(".config/");
    hunter_path.push("hunter/");
    Ok(hunter_path)
}

pub fn config_path() -> WResult<PathBuf> {
    let mut config_path = wandex_path()?;
    config_path.push("config");
    Ok(config_path)
}

pub fn bindings_path() -> WResult<PathBuf> {
    let mut config_path = wandex_path()?;
    config_path.push("keys");
    Ok(config_path)
}

pub fn bookmark_path() -> WResult<PathBuf> {
    let mut bookmark_path = wandex_path()?;
    bookmark_path.push("bookmarks");
    Ok(bookmark_path)
}

pub fn tagfile_path() -> WResult<PathBuf> {
    let mut tagfile_path = wandex_path()?;
    tagfile_path.push("tags");
    Ok(tagfile_path)
}

pub fn history_path() -> WResult<PathBuf> {
    let mut history_path = wandex_path()?;
    history_path.push("history");
    Ok(history_path)
}

pub fn actions_path() -> WResult<PathBuf> {
    let mut actions_path = wandex_path()?;
    actions_path.push("actions");
    Ok(actions_path)
}

pub fn previewers_path() -> WResult<PathBuf> {
    let mut previewers_path = wandex_path()?;
    previewers_path.push("previewers");
    Ok(previewers_path)
}
