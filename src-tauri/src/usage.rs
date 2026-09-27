//! Plan usage from the `rate_limit_event`s an Ask run already prints (v1.0 S4). Read-only information: Devlings makes no
//! extra requests for it, and the money guard's overage kill doesn't depend on it.

use serde::Serialize;
use serde_json::Value;

/// The latest plan usage Claude Code reported in an Ask run (`Snapshot.usage`). Unset fields are left out of the JSON.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageInfo {
    /// `allowed`, `allowed_warning` or `rejected` today; anything else is passed on and shown generically.
    pub status: String,
    /// When the limit resets, epoch ms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<i64>,
    /// How much of the limit is used, 0.0–1.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utilization: Option<f64>,
    /// Which limit: `five_hour`, `seven_day`, `seven_day_opus`, …
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// When Devlings saw it, epoch ms.
    pub seen_at: i64,
}

/// Status and window names are short identifiers; anything else is dropped rather than passed to the UI.
const MAX_TOKEN_CHARS: usize = 40;
/// The latest date JavaScript can represent, in epoch ms.
const MAX_DATE_MS: i64 = 8_640_000_000_000_000;

fn token(v: Option<&Value>) -> Option<String> {
    let s = v?.as_str()?;
    let ok = !s.is_empty() && s.len() <= MAX_TOKEN_CHARS && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    ok.then(|| s.to_string())
}

/// A share of the limit, 0.0–1.0. Anything else (a percent, a string, a negative) isn't shown.
fn fraction(v: Option<&Value>) -> Option<f64> {
    v?.as_f64().filter(|f| f.is_finite() && (0.0..=1.0).contains(f))
}

/// Epoch seconds (whole or not) to epoch ms, if it's a plausible date.
fn epoch_ms(v: Option<&Value>) -> Option<i64> {
    let v = v?;
    let secs = v.as_i64().or_else(|| v.as_f64().filter(|f| f.is_finite()).map(|f| f.trunc() as i64))?;
    let ms = secs.checked_mul(1000)?;
    (secs > 0 && ms <= MAX_DATE_MS).then_some(ms)
}

/// Reads a `rate_limit_event`'s `rate_limit_info`, or None when it has no usable status. Lenient: a missing, unknown
/// or odd field is left out, never an error. The documented fields are `status`, `resetsAt` (epoch s),
/// `rateLimitType` and `utilization`; Claude Code 2.1.282 instead reports each window's `utilization` and `resetsAt`
/// under `unifiedWindows.<rateLimitType>`, which is used when the top-level value is missing.
pub fn from_rate_limit_info(info: &Value, seen_at: i64) -> Option<UsageInfo> {
    let status = token(info.get("status"))?;
    let kind = token(info.get("rateLimitType"));
    let window = kind.as_deref().and_then(|k| info.get("unifiedWindows")?.get(k));
    let from_window = |key: &str| window.and_then(|w| w.get(key));
    Some(UsageInfo {
        status,
        resets_at: epoch_ms(info.get("resetsAt")).or_else(|| epoch_ms(from_window("resetsAt"))),
        utilization: fraction(info.get("utilization")).or_else(|| fraction(from_window("utilization"))),
        kind,
        seen_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SEEN: i64 = 42;

    fn usage(info: Value) -> Option<UsageInfo> {
        from_rate_limit_info(&info, SEEN)
    }

    /// The `rate_limit_info` of the first `rate_limit_event` in a captured stream.
    fn from_fixture(fixture: &str) -> Option<UsageInfo> {
        let line = fixture.lines().find(|l| l.contains("\"rate_limit_event\"")).expect("the fixture has a rate_limit_event");
        let v: Value = serde_json::from_str(line.trim_start_matches('\u{feff}')).expect("the fixture line is JSON");
        from_rate_limit_info(&v["rate_limit_info"], SEEN)
    }

    fn info(status: &str, resets_at: Option<i64>, utilization: Option<f64>, kind: Option<&str>) -> UsageInfo {
        UsageInfo { status: status.into(), resets_at, utilization, kind: kind.map(Into::into), seen_at: SEEN }
    }

    /// Claude Code 2.1.282 puts the utilization of each window under `unifiedWindows`, keyed by `rateLimitType`.
    #[test]
    fn allowed_from_a_captured_2_1_282_stream() {
        for fixture in [
            include_str!("../tests/fixtures/cc2.1.282_stream_hook_deny.ndjson"),
            include_str!("../tests/fixtures/cc2.1.282_stream_no_approval.ndjson"),
        ] {
            assert_eq!(from_fixture(fixture), Some(info("allowed", Some(1_790_475_000_000), Some(0.31), Some("five_hour"))));
        }
        assert_eq!(
            from_fixture(include_str!("../tests/fixtures/cc2.1.282_stream_stop_hook_error.ndjson")),
            Some(info("allowed", Some(1_790_475_000_000), Some(0.33), Some("five_hour")))
        );
    }

    /// The documented shape: `utilization` at the top level.
    #[test]
    fn warning_with_a_top_level_utilization() {
        assert_eq!(
            from_fixture(include_str!("../tests/fixtures/stream_success.ndjson")),
            Some(info("allowed_warning", Some(1_790_679_600_000), Some(0.82), Some("seven_day")))
        );
    }

    #[test]
    fn rejected() {
        let rejected = json!({"status": "rejected", "resetsAt": 1_790_475_000, "rateLimitType": "five_hour", "utilization": 1.0, "isUsingOverage": false});
        assert_eq!(usage(rejected), Some(info("rejected", Some(1_790_475_000_000), Some(1.0), Some("five_hour"))));
    }

    #[test]
    fn the_top_level_utilization_wins_over_the_window() {
        let both = json!({"status": "allowed", "rateLimitType": "seven_day", "utilization": 0.5, "resetsAt": 10,
            "unifiedWindows": {"seven_day": {"utilization": 0.1, "resetsAt": 20}}});
        assert_eq!(usage(both), Some(info("allowed", Some(10_000), Some(0.5), Some("seven_day"))));
        let window_only = json!({"status": "allowed", "rateLimitType": "seven_day",
            "unifiedWindows": {"five_hour": {"utilization": 0.9, "resetsAt": 5}, "seven_day": {"utilization": 0.1, "resetsAt": 20}}});
        assert_eq!(usage(window_only), Some(info("allowed", Some(20_000), Some(0.1), Some("seven_day"))));
    }

    #[test]
    fn unknown_and_missing_fields_just_are_not_shown() {
        assert_eq!(usage(json!({"status": "allowed"})), Some(info("allowed", None, None, None)));
        let odd = json!({"status": "allowed", "brandNew": {"x": 1}, "rateLimitType": "five_hour", "resetsAt": "soon",
            "utilization": "lots", "unifiedWindows": {"five_hour": {"utilization": [1], "resetsAt": null}}});
        assert_eq!(usage(odd), Some(info("allowed", None, None, Some("five_hour"))));
        // A status Devlings doesn't know yet still counts; the UI shows it generically.
        assert_eq!(usage(json!({"status": "allowed_soon"})), Some(info("allowed_soon", None, None, None)));
        // Without a window name there's no telling which window to read.
        let no_kind = json!({"status": "allowed", "unifiedWindows": {"five_hour": {"utilization": 0.2}}});
        assert_eq!(usage(no_kind), Some(info("allowed", None, None, None)));
    }

    #[test]
    fn out_of_range_or_odd_values_are_dropped() {
        for u in [json!(1.7), json!(-0.1), json!(42), json!(true)] {
            assert_eq!(usage(json!({"status": "allowed", "utilization": u})).and_then(|x| x.utilization), None, "{u}");
        }
        assert_eq!(usage(json!({"status": "allowed", "utilization": 0})).and_then(|x| x.utilization), Some(0.0));
        for r in [json!(-5), json!(0), json!(9.3e18), json!(i64::MAX)] {
            assert_eq!(usage(json!({"status": "allowed", "resetsAt": r})).and_then(|x| x.resets_at), None, "{r}");
        }
        assert_eq!(usage(json!({"status": "allowed", "resetsAt": 1_790_475_000.4})).and_then(|x| x.resets_at), Some(1_790_475_000_000));
        for k in [json!("five hour"), json!(""), json!("x".repeat(41)), json!(5)] {
            assert_eq!(usage(json!({"status": "allowed", "rateLimitType": k})).and_then(|x| x.kind), None, "{k}");
        }
    }

    #[test]
    fn no_status_means_nothing_to_show() {
        assert_eq!(usage(json!({"resetsAt": 10, "utilization": 0.5})), None);
        assert_eq!(usage(json!({"status": 3})), None);
        assert_eq!(usage(json!({"status": ""})), None);
        assert_eq!(usage(json!({"status": "<b>allowed</b>"})), None);
        assert_eq!(usage(json!("x")), None);
        assert_eq!(usage(Value::Null), None);
        assert_eq!(usage(json!([1])), None);
    }

    #[test]
    fn serializes_for_the_frontend() {
        assert_eq!(serde_json::to_value(info("allowed", None, None, None)).unwrap(), json!({"status": "allowed", "seenAt": 42}));
        assert_eq!(
            serde_json::to_value(info("allowed_warning", Some(1_000), Some(0.82), Some("seven_day"))).unwrap(),
            json!({"status": "allowed_warning", "resetsAt": 1000, "utilization": 0.82, "kind": "seven_day", "seenAt": 42})
        );
    }
}
