#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    api::server::serve().await?;

    Ok(())
}
