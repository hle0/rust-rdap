use std::time::{Duration, SystemTime};

use anyhow::Context;
use directories::ProjectDirs;
use async_std::{fs::{File, read, metadata, rename}, io::WriteExt, path::{Path, PathBuf}};

async fn read_cache(path: &Path) -> anyhow::Result<serde_json::Value> {
    let text = read(path).await?;    // File doesn't already exist
    anyhow::Result::<serde_json::Value>::Ok(serde_json::from_slice(&text[..])?)
}

async fn write_cache(webloc: &str, path: &Path, tmp_path: &Path) -> anyhow::Result<serde_json::Value> {
    // File doesn't already exist
    let text = reqwest::get(webloc).await?.text().await?;
    let json = serde_json::from_str(text.as_str())?;

    // Try to cache it
    {
        let mut tmp = File::create(tmp_path).await?;
        tmp.write(text.as_bytes()).await?;
    }

    // Moves should be atomic. TODO: verify?
    rename(tmp_path, path).await.context("renaming temporary cache file")?;

    Ok(json)
}

pub async fn get_bootstrap_file(filename: &str, webloc: &str) -> anyhow::Result<serde_json::Value> {
    let loc = ProjectDirs::from("", "", "rust-rdap").context("cannot find cache dir")?;

    let path = {
        let mut buf = PathBuf::new();
        buf.extend(loc.cache_dir());
        buf.extend([filename]);
        buf.into_boxed_path()
    };

    let tmp_path = {
        let mut buf = PathBuf::new();
        buf.extend(loc.cache_dir());
        buf.extend([filename.to_owned() + ".tmp"]);
        buf.into_boxed_path()
    };

    if let Ok(meta) = metadata(&path).await {
        // File exists

        // If older than a week, rewrite it.
        if meta.created()? + Duration::from_secs(60 * 60 * 24 * 7) < SystemTime::now() {
            write_cache(webloc, &path, &tmp_path).await
        } else {
            match read_cache(&path).await {
                Ok(o) => Ok(o),
                Err(_) => write_cache(webloc, &path, &tmp_path).await // rewrite if something is wrong
            }
        }
    } else {
        // Make the cached file
        write_cache(webloc, &path, &tmp_path).await
    }
}