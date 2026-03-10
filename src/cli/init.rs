use crate::config::{Config, init_bmo_dir};
use crate::db::open_db;
use clap::Args;

#[derive(Args)]
pub struct InitArgs {
    /// Project name
    #[arg(long)]
    pub name: Option<String>,
}

pub fn run(args: &InitArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = init_bmo_dir()?;
    let db_path = bmo_dir.join("issues.db");

    let already_exists = db_path.exists();

    // Initialize (or open) the database
    let _repo = open_db(&db_path)?;

    // Write default config if it doesn't already exist
    let config_path = bmo_dir.join("config.toml");
    if !config_path.exists() {
        let config = Config {
            project_name: args.name.clone(),
            ..Default::default()
        };
        config.save(&bmo_dir)?;
    }

    let msg = if already_exists {
        format!("Already initialized — database at {}", db_path.display())
    } else {
        format!("Initialized bmo project at {}", db_path.display())
    };

    if json {
        let data = serde_json::json!({ "db_path": db_path.to_string_lossy(), "already_existed": already_exists });
        let envelope = serde_json::json!({ "ok": true, "data": data, "message": msg });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("{msg}");
        if !already_exists {
            println!("Consider adding .bmo/ to your .gitignore");
        }
    }

    Ok(())
}
