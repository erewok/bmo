use clap::Args;

use crate::config::{Config, find_bmo_dir};

#[derive(Args)]
pub struct ConfigArgs {
    /// Get a config value by key
    #[arg(long)]
    pub get: Option<String>,
    /// Set a config value (format: key=value)
    #[arg(long)]
    pub set: Option<String>,
}

pub fn run(args: &ConfigArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let mut config = Config::load(&bmo_dir)?;

    if let Some(kv) = &args.set {
        let (key, value) = kv
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("--set requires key=value format"))?;
        match key {
            "project_name" => config.project_name = Some(value.to_string()),
            "default_assignee" => config.default_assignee = Some(value.to_string()),
            "web_port" => {
                config.web_port = Some(
                    value
                        .parse()
                        .map_err(|_| anyhow::anyhow!("web_port must be a number"))?,
                )
            }
            "web_host" => config.web_host = Some(value.to_string()),
            _ => anyhow::bail!("unknown config key: {key}"),
        }
        config.save(&bmo_dir)?;
        if !json {
            println!("Set {kv}");
        }
    }

    if let Some(key) = &args.get {
        let value = match key.as_str() {
            "project_name" => config.project_name.as_deref().unwrap_or("").to_string(),
            "default_assignee" => config.default_assignee.as_deref().unwrap_or("").to_string(),
            "web_port" => config.web_port().to_string(),
            "web_host" => config.web_host().to_string(),
            _ => anyhow::bail!("unknown config key: {key}"),
        };
        if json {
            let envelope = serde_json::json!({ "ok": true, "data": { key: value }, "message": "" });
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        } else {
            println!("{value}");
        }
        return Ok(());
    }

    // List all config
    if json {
        let envelope = serde_json::json!({ "ok": true, "data": config, "message": "" });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!(
            "project_name     = {}",
            config.project_name.as_deref().unwrap_or("(not set)")
        );
        println!(
            "default_assignee = {}",
            config.default_assignee.as_deref().unwrap_or("(not set)")
        );
        println!("web_port         = {}", config.web_port());
        println!("web_host         = {}", config.web_host());
        println!("db               = {}", bmo_dir.join("issues.db").display());
    }

    Ok(())
}
