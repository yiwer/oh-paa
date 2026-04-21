use chrono::{DateTime, Utc};
use pa_core::Timeframe;
use pa_market::{OpenBarBook, ProviderTick};
use rust_decimal::Decimal;

fn parse_time(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("valid timestamp fixture")
        .with_timezone(&Utc)
}

#[test]
fn later_ticks_update_high_low_and_close_without_changing_open() {
    let mut bars = OpenBarBook::default();
    let open_time = parse_time("2024-01-01T09:30:00Z");

    bars.start_bar(Timeframe::M15, open_time, Decimal::new(100, 0));
    bars.apply_tick(
        Timeframe::M15,
        ProviderTick {
            price: Decimal::new(105, 0),
            size: None,
            tick_time: parse_time("2024-01-01T09:35:00Z"),
        },
    )
    .expect("started bar should accept later ticks");
    bars.apply_tick(
        Timeframe::M15,
        ProviderTick {
            price: Decimal::new(95, 0),
            size: None,
            tick_time: parse_time("2024-01-01T09:40:00Z"),
        },
    )
    .expect("started bar should keep updating");

    let bar = bars
        .current_bar(Timeframe::M15)
        .expect("bar should exist for timeframe");

    assert_eq!(bar.open_time, open_time);
    assert_eq!(bar.open, Decimal::new(100, 0));
    assert_eq!(bar.high, Decimal::new(105, 0));
    assert_eq!(bar.low, Decimal::new(95, 0));
    assert_eq!(bar.close, Decimal::new(95, 0));
}

#[test]
fn rejects_tick_that_arrives_before_bar_open_time() {
    let mut bars = OpenBarBook::default();
    let open_time = parse_time("2024-01-01T09:30:00Z");

    bars.start_bar(Timeframe::M15, open_time, Decimal::new(100, 0));

    let error = bars
        .apply_tick(
            Timeframe::M15,
            ProviderTick {
                price: Decimal::new(105, 0),
                size: None,
                tick_time: parse_time("2024-01-01T09:29:59Z"),
            },
        )
        .expect_err("pre-open tick should be rejected");

    match error {
        pa_core::AppError::Validation { message, .. } => {
            assert!(message.contains("before bar open"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }

    let bar = bars
        .current_bar(Timeframe::M15)
        .expect("bar should remain available");
    assert_eq!(bar.high, Decimal::new(100, 0));
    assert_eq!(bar.low, Decimal::new(100, 0));
    assert_eq!(bar.close, Decimal::new(100, 0));
}

#[test]
fn rejects_tick_that_arrives_older_than_latest_tick() {
    let mut bars = OpenBarBook::default();
    let open_time = parse_time("2024-01-01T09:30:00Z");

    bars.start_bar(Timeframe::M15, open_time, Decimal::new(100, 0));
    bars.apply_tick(
        Timeframe::M15,
        ProviderTick {
            price: Decimal::new(105, 0),
            size: None,
            tick_time: parse_time("2024-01-01T09:35:00Z"),
        },
    )
    .expect("first in-order tick should be accepted");

    let error = bars
        .apply_tick(
            Timeframe::M15,
            ProviderTick {
                price: Decimal::new(95, 0),
                size: None,
                tick_time: parse_time("2024-01-01T09:34:00Z"),
            },
        )
        .expect_err("out-of-order tick should be rejected");

    match error {
        pa_core::AppError::Validation { message, .. } => {
            assert!(message.contains("older than latest tick"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }

    let bar = bars
        .current_bar(Timeframe::M15)
        .expect("bar should remain available");
    assert_eq!(bar.high, Decimal::new(105, 0));
    assert_eq!(bar.low, Decimal::new(100, 0));
    assert_eq!(bar.close, Decimal::new(105, 0));
}
