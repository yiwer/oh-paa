#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    pa_app::init_cli_tracing();
    let args = pa_app::replay_probe::parse_probe_cli_args(std::env::args())?;
    tracing::info!(step_key = %args.step_key, step_version = %args.step_version, "replay_probe starting");
    let result = pa_app::replay_probe::run_probe_from_path(
        args.config_path,
        &args.step_key,
        &args.step_version,
        args.input_path,
    )
    .await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
