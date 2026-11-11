//! The freshness record: the UTC time of the last successful pin check, kept as `freshness.json` on the `freshness` branch
//! of the package repository and written only by the pin-update workflow. The consumer upgrade workflow fails visibly when
//! it is stale. Ported from `runner/src/freshness.ts`.

use std::time::Duration;

use time::OffsetDateTime;

pub const FRESHNESS_URL: &str =
    "https://raw.githubusercontent.com/pantheon-org/pantheon-scaffold/freshness/freshness.json";
pub const MAX_AGE_DAYS: u64 = 30;

#[derive(Debug, PartialEq, Eq)]
pub struct FreshnessError(pub String);

impl std::fmt::Display for FreshnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for FreshnessError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Fresh,
    Stale,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Freshness {
    pub state: State,
    pub age_days: u64,
    pub checked_at: String,
}

/// The time in a record, or an error. Only a UTC time that ends in `Z` is accepted, so the record never depends on a time zone.
pub fn parse_freshness(text: &str) -> Result<OffsetDateTime, FreshnessError> {
    use time::format_description::well_known::Rfc3339;
    static SHAPE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")
            .expect("valid")
    });
    let fail = |message: String| Err(FreshnessError(message));
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return fail("the freshness record is not valid JSON".to_owned());
    };
    let version = json.get("schemaVersion");
    if !version.is_some_and(|v| v.as_f64() == Some(1.0)) {
        let shown = version.map_or_else(|| "undefined".to_owned(), ToString::to_string);
        return fail(format!(
            "the freshness record has schemaVersion {shown}, this runner reads 1"
        ));
    }
    let Some(when) = json
        .get("lastSuccessfulPinCheck")
        .and_then(serde_json::Value::as_str)
        .filter(|w| SHAPE.is_match(w))
    else {
        return fail(
            "lastSuccessfulPinCheck must be a UTC time such as 2026-11-08T12:00:00Z".to_owned(),
        );
    };
    // The parser refuses a date that does not exist (the 30th of February) and a leap second, so none is quietly moved.
    OffsetDateTime::parse(when, &Rfc3339)
        .or_else(|_| fail(format!("lastSuccessfulPinCheck {when} is not a real time")))
}

const SECONDS: &[time::format_description::BorrowedFormatItem<'static>] =
    time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");

pub struct CheckOptions<'a> {
    /// Fetches a URL and returns its text; the error is the message to show.
    pub fetcher: &'a dyn Fn(&str) -> Result<String, String>,
    pub url: &'a str,
    pub now: OffsetDateTime,
    pub max_age_days: u64,
    pub attempts: u32,
    pub delay: Duration,
}

/// Fetches the record and says whether it is fresh. A fetch that fails is retried and then fails closed: an unreadable
/// record is never fresh.
pub fn check_freshness(opts: &CheckOptions) -> Result<Freshness, FreshnessError> {
    const DAY_MS: i128 = 86_400_000;
    let attempts = opts.attempts.max(1);
    let mut fetched = None;
    for attempt in 1..=attempts {
        match (opts.fetcher)(opts.url) {
            Ok(text) => {
                fetched = Some(text);
                break;
            }
            Err(message) if attempt == attempts => {
                return Err(FreshnessError(format!(
                    "could not fetch {} after {attempts} attempts, failing closed: {message}",
                    opts.url
                )));
            }
            Err(_) => std::thread::sleep(opts.delay),
        }
    }
    let text = fetched.expect("the last attempt either returns an error or has the text");
    let checked = parse_freshness(&text)?;
    let age = (opts.now - checked).whole_milliseconds();
    // A record dated a little ahead is clock skew. Further ahead than a day it is a data error.
    if age < -DAY_MS {
        let iso = checked
            .format(&time::macros::format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second].000Z"
            ))
            .expect("a time formats");
        return Err(FreshnessError(format!(
            "the freshness record is dated in the future ({iso}), which is a clock or data error"
        )));
    }
    let limit = i128::from(opts.max_age_days) * DAY_MS;
    Ok(Freshness {
        state: if age > limit {
            State::Stale
        } else {
            State::Fresh
        },
        age_days: u64::try_from((age / DAY_MS).max(0)).unwrap_or(0),
        checked_at: checked.format(&SECONDS).expect("a time formats"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use time::macros::datetime;

    const NOW: OffsetDateTime = datetime!(2026-11-08 12:00:00 UTC);
    const DAY_MS: i64 = 86_400_000;

    fn record(iso: &str) -> String {
        format!("{{\"schemaVersion\":1,\"lastSuccessfulPinCheck\":\"{iso}\"}}")
    }

    /// The record for a check made `age_ms` before `NOW` (negative: after it).
    fn aged(age_ms: i64) -> String {
        let when = NOW - time::Duration::milliseconds(age_ms);
        record(
            &when
                .format(&time::macros::format_description!(
                    "[year]-[month]-[day]T[hour]:[minute]:[second]Z"
                ))
                .unwrap(),
        )
    }

    fn check(text: String, max_age_days: u64) -> Result<Freshness, FreshnessError> {
        let fetcher = move |_: &str| Ok(text.clone());
        check_freshness(&CheckOptions {
            fetcher: &fetcher,
            url: "u",
            now: NOW,
            max_age_days,
            attempts: 3,
            delay: Duration::from_millis(1),
        })
    }

    fn state(age_ms: i64) -> State {
        check(aged(age_ms), 30).unwrap().state
    }

    #[test]
    fn the_record_lives_on_the_freshness_branch_of_the_package_repository() {
        assert_eq!(
            FRESHNESS_URL,
            "https://raw.githubusercontent.com/pantheon-org/pantheon-scaffold/freshness/freshness.json"
        );
        assert_eq!(MAX_AGE_DAYS, 30);
    }

    #[test]
    fn a_valid_record_parses_to_its_utc_time() {
        assert_eq!(
            parse_freshness(&record("2026-11-01T08:30:00Z")).unwrap(),
            datetime!(2026-11-01 08:30:00 UTC)
        );
    }

    #[test]
    fn anything_that_is_not_a_valid_record_is_refused() {
        let bad = [
            "".to_owned(),
            "nope".to_owned(),
            "[]".to_owned(),
            "{}".to_owned(),
            "{\"schemaVersion\":2,\"lastSuccessfulPinCheck\":\"2026-11-01T08:30:00Z\"}".to_owned(),
            record("2026-11-01"),
            record("2026-11-01T08:30:00+01:00"),
            record("2026-11-01T08:30:00"),
            record("2026-13-45T25:61:61Z"),
            record("yesterday"),
            "{\"schemaVersion\":1,\"lastSuccessfulPinCheck\":5}".to_owned(),
        ];
        for text in bad {
            assert!(parse_freshness(&text).is_err(), "{text}");
        }
    }

    #[test]
    fn each_refusal_has_its_own_message() {
        let message = |text: &str| parse_freshness(text).unwrap_err().to_string();
        assert_eq!(message("nope"), "the freshness record is not valid JSON");
        assert_eq!(
            message("{}"),
            "the freshness record has schemaVersion undefined, this runner reads 1"
        );
        assert_eq!(
            message("[]"),
            "the freshness record has schemaVersion undefined, this runner reads 1"
        );
        assert_eq!(
            message("null"),
            "the freshness record has schemaVersion undefined, this runner reads 1"
        );
        assert_eq!(
            message("{\"schemaVersion\":2,\"lastSuccessfulPinCheck\":\"2026-11-01T08:30:00Z\"}"),
            "the freshness record has schemaVersion 2, this runner reads 1"
        );
        assert_eq!(
            message("{\"schemaVersion\":\"1\"}"),
            "the freshness record has schemaVersion \"1\", this runner reads 1"
        );
        let utc = "lastSuccessfulPinCheck must be a UTC time such as 2026-11-08T12:00:00Z";
        assert_eq!(message(&record("2026-11-01T08:30:00+01:00")), utc);
        assert_eq!(message("{\"schemaVersion\":1}"), utc);
        assert_eq!(
            message("{\"schemaVersion\":1,\"lastSuccessfulPinCheck\":5}"),
            utc
        );
    }

    #[test]
    fn a_time_with_the_right_shape_that_is_not_a_real_time_is_refused_by_name() {
        for when in [
            "2026-13-45T25:61:61Z",
            "2026-02-30T00:00:00Z",
            "2026-11-08T12:00:60Z",
        ] {
            assert_eq!(
                parse_freshness(&record(when)).unwrap_err().to_string(),
                format!("lastSuccessfulPinCheck {when} is not a real time")
            );
        }
    }

    #[test]
    fn a_record_up_to_30_days_old_is_fresh_and_older_than_that_is_stale() {
        assert_eq!(state(0), State::Fresh);
        assert_eq!(state(29 * DAY_MS + DAY_MS * 99 / 100), State::Fresh);
        assert_eq!(state(30 * DAY_MS), State::Fresh);
        assert_eq!(state(30 * DAY_MS + 1000), State::Stale);
        assert_eq!(state(90 * DAY_MS), State::Stale);
    }

    #[test]
    fn the_age_is_reported_in_whole_days_and_the_limit_can_be_changed() {
        let r = check(aged(DAY_MS * 39 / 10), 30).unwrap();
        assert_eq!(
            (r.state, r.age_days, r.checked_at.as_str()),
            (State::Fresh, 3, "2026-11-04T14:24:00Z")
        );
        assert_eq!(
            check(aged(DAY_MS * 39 / 10), 2).unwrap().state,
            State::Stale
        );
        assert_eq!(check(aged(0), 30).unwrap().age_days, 0);
    }

    #[test]
    fn a_record_dated_a_little_in_the_future_is_fresh_with_age_zero() {
        let r = check(aged(-3_600_000), 30).unwrap();
        assert_eq!((r.state, r.age_days), (State::Fresh, 0));
        assert_eq!(check(aged(-DAY_MS), 30).unwrap().state, State::Fresh);
    }

    #[test]
    fn a_record_far_in_the_future_is_refused_as_a_clock_or_data_error() {
        let message = check(aged(-3 * DAY_MS), 30).unwrap_err().to_string();
        assert_eq!(
            message,
            "the freshness record is dated in the future (2026-11-11T12:00:00.000Z), which is a clock or data error"
        );
        assert!(check(aged(-DAY_MS - 1000), 30).is_err());
    }

    #[test]
    fn a_fetch_that_fails_is_retried_and_the_third_attempt_succeeds() {
        let calls = Cell::new(0);
        let fetcher = |_: &str| {
            calls.set(calls.get() + 1);
            if calls.get() < 3 {
                Err("HTTP 504".to_owned())
            } else {
                Ok(aged(DAY_MS))
            }
        };
        let r = check_freshness(&CheckOptions {
            fetcher: &fetcher,
            url: "u",
            now: NOW,
            max_age_days: 30,
            attempts: 3,
            delay: Duration::from_millis(1),
        });
        assert_eq!(r.unwrap().state, State::Fresh);
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn a_fetch_that_never_works_fails_closed_after_every_attempt() {
        let calls = Cell::new(0);
        let fetcher = |_: &str| {
            calls.set(calls.get() + 1);
            Err("HTTP 504".to_owned())
        };
        let r = check_freshness(&CheckOptions {
            fetcher: &fetcher,
            url: "http://x/y",
            now: NOW,
            max_age_days: 30,
            attempts: 3,
            delay: Duration::from_millis(1),
        });
        assert_eq!(
            r.unwrap_err().to_string(),
            "could not fetch http://x/y after 3 attempts, failing closed: HTTP 504"
        );
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn zero_attempts_still_tries_once() {
        let calls = Cell::new(0);
        let fetcher = |_: &str| {
            calls.set(calls.get() + 1);
            Err("boom".to_owned())
        };
        let r = check_freshness(&CheckOptions {
            fetcher: &fetcher,
            url: "u",
            now: NOW,
            max_age_days: 30,
            attempts: 0,
            delay: Duration::ZERO,
        });
        assert_eq!(
            r.unwrap_err().to_string(),
            "could not fetch u after 1 attempts, failing closed: boom"
        );
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn a_wait_happens_between_attempts_and_not_after_the_last() {
        let fetcher = |_: &str| Err("x".to_owned());
        let started = std::time::Instant::now();
        let options = CheckOptions {
            fetcher: &fetcher,
            url: "u",
            now: NOW,
            max_age_days: 30,
            attempts: 3,
            delay: Duration::from_millis(150),
        };
        let _ = check_freshness(&options);
        let took = started.elapsed();
        assert!(took >= Duration::from_millis(300), "{took:?}");
        assert!(took < Duration::from_millis(440), "{took:?}");
    }

    #[test]
    fn a_record_that_is_fetched_but_invalid_is_not_retried() {
        let calls = Cell::new(0);
        let fetcher = |_: &str| {
            calls.set(calls.get() + 1);
            Ok("garbage".to_owned())
        };
        let r = check_freshness(&CheckOptions {
            fetcher: &fetcher,
            url: "u",
            now: NOW,
            max_age_days: 30,
            attempts: 3,
            delay: Duration::from_millis(1),
        });
        assert_eq!(
            r.unwrap_err().to_string(),
            "the freshness record is not valid JSON"
        );
        assert_eq!(calls.get(), 1);
    }
}
