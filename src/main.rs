use anyhow::Result;
use biu_tui::app::App;

fn main() -> Result<()> {
    let mut app = App::new()?;
    app.run()?;
    Ok(())
}
