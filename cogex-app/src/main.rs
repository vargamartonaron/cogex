mod app;
pub use app::App;

fn main() -> anyhow::Result<()> {
    let app = App::new()?;
    app.run()?;

    Ok(())
}
