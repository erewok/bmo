use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub project_name: Option<String>,
    pub default_assignee: Option<String>,
    pub web_port: Option<u16>,
    pub web_host: Option<String>,
}

impl Config {
    pub fn load(bmo_dir: &Path) -> anyhow::Result<Self> {
        let path = bmo_dir.join("config.toml");
        if !path.exists() {
            return Ok(Config::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self, bmo_dir: &Path) -> anyhow::Result<()> {
        let path = bmo_dir.join("config.toml");
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    pub fn web_port(&self) -> u16 {
        self.web_port.unwrap_or(7777)
    }

    pub fn web_host(&self) -> &str {
        self.web_host.as_deref().unwrap_or("127.0.0.1")
    }
}

/// Find the .bmo directory by walking up from CWD.
pub fn find_bmo_dir() -> anyhow::Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        let candidate = dir.join(".bmo");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    anyhow::bail!("not in a bmo project — run `bmo init` first")
}

/// Returns the .bmo directory in the current working directory, creating it if needed.
pub fn init_bmo_dir() -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let bmo_dir = cwd.join(".bmo");
    std::fs::create_dir_all(&bmo_dir)?;
    Ok(bmo_dir)
}
