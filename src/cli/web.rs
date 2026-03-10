use clap::Args;

use crate::config::find_bmo_dir;

#[derive(Args)]
pub struct WebArgs {
    /// Port to listen on
    #[arg(short, long, default_value = "7777")]
    pub port: u16,
    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Do not open a browser window automatically
    #[arg(long)]
    pub no_open: bool,
}

pub fn run(args: &WebArgs, _json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let db_path = bmo_dir.join("issues.db");
    let url = format!("http://{}:{}", args.host, args.port);

    if !args.no_open {
        let url_clone = url.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let _ = open_browser(&url_clone);
        });
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(crate::web::start_server(&args.host, args.port, db_path))
}

fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(url)
        .spawn()?
        .wait()?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()?
        .wait()?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd")
        .args(["/c", "start", url])
        .spawn()?
        .wait()?;
    Ok(())
}
