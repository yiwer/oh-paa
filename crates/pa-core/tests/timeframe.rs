use std::time::Duration;

use pa_core::timeframe::Timeframe;

#[test]
fn timeframe_exposes_expected_string_values_and_durations() {
    assert_eq!(Timeframe::M15.as_str(), "15m");
    assert_eq!(Timeframe::H1.as_str(), "1h");
    assert_eq!(Timeframe::D1.as_str(), "1d");

    assert_eq!(Timeframe::M15.duration(), Duration::from_secs(15 * 60));
    assert_eq!(Timeframe::H1.duration(), Duration::from_secs(60 * 60));
    assert_eq!(Timeframe::D1.duration(), Duration::from_secs(24 * 60 * 60));
}
