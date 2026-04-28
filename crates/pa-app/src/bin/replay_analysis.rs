#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    pa_app::load_dotenv();
    pa_app::init_cli_tracing();
    let args = pa_app::replay::parse_replay_cli_args(std::env::args())?;
    tracing::info!(mode = ?args.mode, variant = %args.variant, "replay_analysis starting");
    let report = match args.mode {
        pa_app::replay::ReplayExecutionMode::Fixture => {
            pa_app::replay::run_fixture_replay_variant_from_path(args.dataset_path, &args.variant)
                .await?
        }
        pa_app::replay::ReplayExecutionMode::LiveHistorical => {
            pa_app::replay_live::run_live_historical_replay_from_path(
                args.dataset_path,
                args.config_path.expect("live mode validated config"),
                &args.variant,
            )
            .await?
        }
    };

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
