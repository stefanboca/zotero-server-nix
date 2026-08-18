mod keys;
mod profile;
mod savekey;
mod security;
mod sessions;
mod storage;

pub(in crate::app) use storage::storage as storage_route;

use jiff::Timestamp;
use topcoat::{
    Result,
    context::{Cx, request_context},
    router::{
        Body, Next, error::rewrite, href, layer, layout, request::uri, response::Response, route,
    },
    view::{PromotedStr, view},
};

use crate::{app::signed_in_user, database};

#[derive(Clone)]
pub(super) struct SettingsUser(database::UserSummary);

pub(super) fn settings_user(cx: &Cx) -> &database::UserSummary {
    &request_context::<SettingsUser>(cx).0
}

#[layer]
async fn authenticated_settings(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let cx = cx.with(SettingsUser(signed_in_user(cx).await?));
    next.run(&cx, body).await
}

#[route(GET)]
pub(super) async fn settings(cx: &Cx) -> Result<Response> {
    Err(rewrite(href!(profile::profile).resolve(cx), Body::empty()).into())
}

#[layout]
async fn settings_layout(cx: &Cx, slot: Result) -> Result {
    let _ = settings_user(cx);
    let path = uri(cx).path();
    let settings_root = href!(settings).resolve(cx);
    let navigation = [
        (href!(profile::profile).resolve(cx), PromotedStr(&"Profile")),
        (
            href!(security::security).resolve(cx),
            PromotedStr(&"Security"),
        ),
        (href!(storage::storage).resolve(cx), PromotedStr(&"Storage")),
    ];
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <div class="grid gap-8 md:grid-cols-[11rem_minmax(0,1fr)]">
                <nav aria-label="Settings sections">
                    <h2 class="text-lg font-semibold">"Settings"</h2>
                    <ul
                        class="mt-3 border-y border-stone-200 py-2 dark:border-stone-800"
                    >
                        for (href, label) in navigation {
                            let active = path == href
                                || (href.ends_with("/profile") && path == settings_root);
                            <li>
                                <a
                                    class=(if active {
                                        "block border-l-2 border-red-800 bg-red-50 px-3 py-2 font-semibold text-red-800 dark:border-red-400 dark:bg-red-950 dark:text-red-400"
                                    } else {
                                        "block border-l-2 border-transparent px-3 py-2 hover:bg-stone-100 dark:hover:bg-stone-900"
                                    })
                                    href=(href)
                                    aria-current=(active.then_some("page"))
                                >
                                    (label)
                                </a>
                            </li>
                        }
                    </ul>
                </nav>
                <div class="min-w-0">(slot?)</div>
            </div>
        </main>
    }
}

pub(super) fn format_storage_size(mebibytes: f64) -> String {
    const MIB_PER_GIB: f64 = 1024.0;
    const MIB_PER_TIB: f64 = MIB_PER_GIB * 1024.0;
    if mebibytes >= MIB_PER_TIB {
        format!("{:.2} TiB", mebibytes / MIB_PER_TIB)
    } else if mebibytes >= MIB_PER_GIB {
        format!("{:.2} GiB", mebibytes / MIB_PER_GIB)
    } else {
        format!("{mebibytes:.1} MiB")
    }
}

fn format_relative_time(now: Timestamp, timestamp: Timestamp) -> String {
    let seconds = now.duration_until(timestamp).as_secs();
    if seconds > 0 {
        return format_future_time(seconds.unsigned_abs());
    }
    let seconds = seconds.unsigned_abs();
    match seconds {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} ago", time_unit(seconds / 60, "minute")),
        3600..86400 => format!("{} ago", time_unit(seconds / 3600, "hour")),
        86400..2_592_000 => format!("{} ago", time_unit(seconds / 86400, "day")),
        2_592_000..31_536_000 => {
            format!("{} ago", time_unit(seconds / 2_592_000, "month"))
        }
        _ => format!("{} ago", time_unit(seconds / 31_536_000, "year")),
    }
}

fn format_future_time(seconds: u64) -> String {
    match seconds {
        0..60 => "in less than a minute".to_owned(),
        60..3600 => format!("in {}", time_unit(seconds / 60, "minute")),
        3600..86400 => format!("in {}", time_unit(seconds / 3600, "hour")),
        86400..2_592_000 => format!("in {}", time_unit(seconds / 86400, "day")),
        2_592_000..31_536_000 => {
            format!("in {}", time_unit(seconds / 2_592_000, "month"))
        }
        _ => format!("in {}", time_unit(seconds / 31_536_000, "year")),
    }
}

fn time_unit(value: u64, unit: &str) -> String {
    if value == 1 {
        format!("1 {unit}")
    } else {
        format!("{value} {unit}s")
    }
}

struct DisplayTimestamp {
    datetime: String,
    absolute: String,
    relative: String,
}

fn display_client_timestamp(
    now: Timestamp,
    value: Option<&str>,
) -> Result<Option<DisplayTimestamp>> {
    let timestamp = value.map(str::parse::<Timestamp>).transpose()?;
    Ok(timestamp.map(|timestamp| display_timestamp(now, timestamp)))
}

fn display_timestamp(now: Timestamp, timestamp: Timestamp) -> DisplayTimestamp {
    let datetime = timestamp.to_string();
    let absolute = format!("{} UTC", datetime.trim_end_matches('Z').replace('T', " "));
    DisplayTimestamp {
        datetime,
        absolute,
        relative: format_relative_time(now, timestamp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_dataserver_storage_units() {
        assert_eq!(format_storage_size(0.0), "0.0 MiB");
        assert_eq!(format_storage_size(1536.0), "1.50 GiB");
        assert_eq!(format_storage_size(2.0 * 1024.0 * 1024.0), "2.00 TiB");
    }

    #[test]
    fn formats_session_activity_without_client_javascript() {
        let timestamp = |second| Timestamp::new(second, 0).expect("valid timestamp");
        assert_eq!(
            format_relative_time(timestamp(10_000), timestamp(10_000)),
            "just now"
        );
        assert_eq!(
            format_relative_time(timestamp(10_000), timestamp(9_880)),
            "2 minutes ago"
        );
        assert_eq!(
            format_relative_time(timestamp(10_000), timestamp(2_800)),
            "2 hours ago"
        );
        assert_eq!(
            format_relative_time(timestamp(10_000), timestamp(10_120)),
            "in 2 minutes"
        );
    }

    #[test]
    fn formats_exact_client_time_for_native_tooltips() {
        let now = Timestamp::new(1_786_176_000, 0).expect("valid timestamp");
        let timestamp = display_client_timestamp(now, Some("2026-08-08T09:30:00Z"))
            .expect("valid timestamp")
            .expect("present timestamp");
        assert_eq!(timestamp.datetime, "2026-08-08T09:30:00Z");
        assert_eq!(timestamp.absolute, "2026-08-08 09:30:00 UTC");
    }
}
