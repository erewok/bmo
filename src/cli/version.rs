pub fn run(json: bool) -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    if json {
        let envelope =
            serde_json::json!({ "ok": true, "data": { "version": version }, "message": version });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("bmo {version}");
    }
    Ok(())
}
