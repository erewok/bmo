use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::output::{OutputMode, make_printer};

pub fn run(json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let stats = repo.get_stats()?;
    printer.print_stats(&stats);
    Ok(())
}
