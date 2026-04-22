#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let dataset = std::env::args().nth(1).expect("dataset path");
    let variant = std::env::args().nth(2).expect("pipeline variant");
    let report = pa_app::replay::run_replay_variant_from_path(dataset, &variant).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
