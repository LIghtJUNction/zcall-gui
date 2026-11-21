//! Read local accounting only on first open or an explicit refresh.
use crate::*;

pub(super) const READ_LABEL: &str = "Read usage";
pub(super) const ENABLE_LABEL: &str = "Enable usage recording";
pub(super) const DISABLE_LABEL: &str = "Disable usage recording";

#[derive(Debug, PartialEq, Eq)]
struct Summary {
    requests: u64,
    failed: u64,
    input: u64,
    output: u64,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    reasoning: Option<u64>,
    recording_configured: Option<bool>,
}

impl Summary {
    fn parse(value: &Json) -> Option<Self> {
        let number = |key| match value.get(key) {
            Json::Number(number) => number.parse::<u64>().ok(),
            _ => None,
        };
        let optional_number = |key| match value.get(key) {
            Json::Null => Some(None),
            Json::Number(number) => number.parse::<u64>().ok().map(Some),
            _ => None,
        };
        let summary = Self {
            requests: number("requests")?,
            failed: number("failed")?,
            input: number("input_tokens")?,
            output: number("output_tokens")?,
            cache_read: optional_number("cache_read_tokens")?,
            cache_write: optional_number("cache_write_tokens")?,
            reasoning: optional_number("reasoning_tokens")?,
            recording_configured: match value.get("recording_configured") {
                Json::Bool(enabled) => Some(*enabled),
                Json::Null => None,
                _ => return None,
            },
        };
        if summary.failed > summary.requests || value.get("error") != &Json::Null {
            None
        } else {
            Some(summary)
        }
    }

    fn succeeded(&self) -> u64 {
        self.requests - self.failed
    }

    fn success_rate(&self) -> String {
        if self.requests == 0 {
            "—".into()
        } else {
            format!(
                "{:.1}%",
                self.succeeded() as f64 * 100. / self.requests as f64
            )
        }
    }
}

fn count(value: u64) -> String {
    let digits = value.to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    result
}

impl App {
    pub(super) fn refresh_usage(&mut self) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.failed = false;
        self.notice = tr("usage.loading").into();
        self.usage = Json::Null;
        let backend = self.backend.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send(Reply {
                label: READ_LABEL.into(),
                result: backend.run(&words(&["usage", "summary"]), None),
                refresh: false,
                starts_gateway: false,
            });
        });
    }

    pub(super) fn accept_usage(&mut self, result: Result<Json, String>) {
        match result.and_then(|data| {
            Summary::parse(&data)
                .map(|_| data)
                .ok_or_else(|| tr("error.invalid_response").to_owned())
        }) {
            Ok(data) => {
                self.usage = data;
                self.notice = tr(if self.usage_restart_pending {
                    "usage.restart_required"
                } else {
                    "usage.refreshed"
                })
                .into();
                self.failed = false;
            }
            Err(error) => {
                self.notice = i18n::message("error.action", &[("error", &error)]);
                self.usage = Json::object([("error".into(), Json::String(error))]);
                self.failed = true;
            }
        }
    }

    fn set_usage_recording(&mut self, enable: bool) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.failed = false;
        self.notice = tr("usage.saving").into();
        let backend = self.backend.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send(Reply {
                label: if enable { ENABLE_LABEL } else { DISABLE_LABEL }.into(),
                result: backend.run(
                    &words(&["usage", if enable { "enable" } else { "disable" }]),
                    None,
                ),
                refresh: false,
                starts_gateway: false,
            });
        });
    }

    pub(super) fn accept_usage_recording(&mut self, result: Result<Json, String>) {
        let result = result.and_then(|data| {
            match (
                data.get("ok"),
                data.get("recording_configured"),
                data.get("restart_required"),
            ) {
                (Json::Bool(true), Json::Bool(enabled), Json::Bool(restart)) => {
                    Ok((*enabled, *restart))
                }
                _ => Err(tr("error.invalid_response").to_owned()),
            }
        });
        match result {
            Ok((enabled, restart)) => {
                if let Json::Object(summary) = &mut self.usage {
                    summary.insert("recording_configured".into(), Json::Bool(enabled));
                }
                self.usage_restart_pending |= restart;
                self.notice = tr(if self.usage_restart_pending {
                    "usage.restart_required"
                } else {
                    "usage.saved"
                })
                .into();
                self.failed = false;
            }
            Err(error) => {
                self.notice = i18n::message("error.action", &[("error", &error)]);
                self.failed = true;
            }
        }
    }

    fn ask_usage_restart(&mut self) {
        self.ask(
            tr("action.restart_service_confirm"),
            words(&["service", "restart"]),
        );
    }

    pub(super) fn usage_page(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        if self.usage == Json::Null {
            empty(
                c,
                Rect::new(x, 140., width, 172.),
                tr("usage.loading"),
                tr("usage.loading_help"),
            );
            return;
        }
        let Some(summary) = Summary::parse(&self.usage) else {
            empty(
                c,
                Rect::new(x, 140., width, 196.),
                tr("error.section"),
                value(&self.usage, "error", tr("error.invalid_response")),
            );
            if self.ui.button(
                c,
                800,
                Rect::new(x + 28., 270., 132., 40.),
                tr("action.retry"),
                Style::Secondary,
                self.enabled(),
            ) {
                self.refresh_usage();
            }
            return;
        };

        if summary.requests == 0 {
            let (title, help, button) =
                match (summary.recording_configured, self.usage_restart_pending) {
                    (None, _) => (
                        "usage.empty",
                        "usage.recording_unknown_help",
                        "usage.open_updates",
                    ),
                    (_, true) => (
                        "usage.saved",
                        "usage.restart_required",
                        "action.restart_service",
                    ),
                    (Some(false), false) => (
                        "usage.recording_off",
                        "usage.recording_off_help",
                        "usage.enable",
                    ),
                    (Some(true), false) => {
                        ("usage.empty", "usage.empty_help", "action.connect_tools")
                    }
                };
            empty(c, Rect::new(x, 140., width, 204.), tr(title), tr(help));
            if self.ui.button(
                c,
                801,
                Rect::new(x + 28., 274., 194., 40.),
                tr(button),
                Style::Primary,
                self.enabled(),
            ) {
                match (summary.recording_configured, self.usage_restart_pending) {
                    (None, _) => {
                        self.page = UPDATES_PAGE;
                        self.repaint = true;
                    }
                    (_, true) => self.ask_usage_restart(),
                    (Some(false), false) => self.set_usage_recording(true),
                    (Some(true), false) => {
                        self.page = 2;
                        self.scroll = 0;
                        self.repaint = true;
                    }
                }
            }
            if summary.recording_configured == Some(true)
                && !self.usage_restart_pending
                && self.ui.button(
                    c,
                    805,
                    Rect::new(x + 240., 274., 174., 40.),
                    tr("action.restart_service"),
                    Style::Quiet,
                    self.enabled(),
                )
            {
                self.ask_usage_restart();
            }
            self.usage_recording_controls(c, x, 372., width, &summary);
            return;
        }

        self.usage_recording_controls(c, x, 112., width, &summary);
        let gap = 16.;
        let tile_width = (width - gap * 2.) / 3.;
        let totals = [
            ("usage.requests", count(summary.requests), TEXT),
            ("usage.succeeded", count(summary.succeeded()), TEXT),
            (
                "usage.failed",
                count(summary.failed),
                if summary.failed == 0 { MUTED } else { RED },
            ),
        ];
        for (index, (label, number, color)) in totals.iter().enumerate() {
            let left = x + index as f32 * (tile_width + gap);
            c.rect(Rect::new(left, 154., tile_width, 125.), 10., SURFACE);
            c.ellipsis(
                tr(label),
                left + 22.,
                176.,
                tile_width - 44.,
                13,
                false,
                MUTED,
            );
            c.ellipsis(number, left + 22., 212., tile_width - 44., 30, true, *color);
        }

        c.text(tr("usage.tokens"), x, 316., 19, true, TEXT);
        let total = u128::from(summary.input) + u128::from(summary.output);
        let input_share = if total == 0 {
            0.
        } else {
            summary.input as f64 / total as f64
        };
        let bar = Rect::new(x, 356., width, 8.);
        c.rect(bar, 4., LINE);
        if total > 0 {
            c.rect(bar, 4., BLUE_PALE);
            let input_width = (width as f64 * input_share) as f32;
            if input_width > 0. {
                c.rect(Rect::new(x, 356., input_width, 8.), 4., BLUE);
            }
        }
        for (index, (label, tokens, color)) in [
            ("usage.input", summary.input, BLUE),
            ("usage.output", summary.output, MUTED),
        ]
        .iter()
        .enumerate()
        {
            let left = x + index as f32 * (width / 2.);
            c.rect(Rect::new(left, 392., 6., 6.), 3., *color);
            c.text(tr(label), left + 16., 385., 13, false, MUTED);
            c.ellipsis(
                &count(*tokens),
                left,
                413.,
                width / 2. - 24.,
                27,
                true,
                TEXT,
            );
        }

        let details = [
            ("usage.cache_read", summary.cache_read),
            ("usage.cache_write", summary.cache_write),
            ("usage.reasoning", summary.reasoning),
        ];
        let has_details = details.iter().any(|(_, value)| value.is_some());
        if has_details {
            for (index, (label, value)) in details.iter().enumerate() {
                let left = x + index as f32 * (tile_width + gap);
                c.ellipsis(tr(label), left, 475., tile_width, 12, false, MUTED);
                let number = value.map(count).unwrap_or_else(|| "—".into());
                c.ellipsis(&number, left, 500., tile_width, 20, true, TEXT);
            }
        }
        let bottom = if has_details { 546. } else { 490. };
        c.rect(Rect::new(x, bottom, width, 1.), 0., LINE);
        c.text(tr("usage.success_rate"), x, bottom + 24., 14, false, MUTED);
        let rate = summary.success_rate();
        let rate_width = c.measure(&rate, 17, true);
        c.text(&rate, x + width - rate_width, bottom + 22., 17, true, TEXT);
        c.ellipsis(
            tr(if has_details {
                "usage.details_help"
            } else {
                "usage.recorded_help"
            }),
            x,
            bottom + 69.,
            width,
            13,
            false,
            MUTED,
        );
        c.ellipsis(
            tr("usage.refresh_help"),
            x,
            bottom + 94.,
            width,
            13,
            false,
            MUTED,
        );
    }

    fn usage_recording_controls(
        &mut self,
        c: &mut Canvas<'_>,
        x: f32,
        y: f32,
        width: f32,
        summary: &Summary,
    ) {
        let label = if self.usage_restart_pending {
            "usage.restart_required"
        } else {
            match summary.recording_configured {
                Some(true) => "usage.recording_configured",
                Some(false) => "usage.recording_off",
                None => "usage.recording_unknown",
            }
        };
        let controls_width = if self.usage_restart_pending {
            342.
        } else {
            174.
        };
        c.ellipsis(
            tr(label),
            x,
            y + 9.,
            (width - controls_width - 16.).max(80.),
            12,
            false,
            MUTED,
        );
        if let Some(enabled) = summary.recording_configured
            && self.ui.button(
                c,
                802,
                Rect::new(x + width - 168., y, 168., 32.),
                tr(if enabled {
                    "usage.disable"
                } else {
                    "usage.enable"
                }),
                if enabled {
                    Style::Quiet
                } else {
                    Style::Secondary
                },
                self.enabled(),
            )
        {
            self.set_usage_recording(!enabled);
        }
        if self.usage_restart_pending
            && self.ui.button(
                c,
                803,
                Rect::new(x + width - 342., y, 168., 32.),
                tr("action.restart_service"),
                Style::Primary,
                self.enabled(),
            )
        {
            self.ask_usage_restart();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_configuration_is_unknown_for_older_gateways() {
        let mut data =
            json::parse(r#"{"requests":0,"failed":0,"input_tokens":0,"output_tokens":0}"#).unwrap();
        assert_eq!(Summary::parse(&data).unwrap().recording_configured, None);
        if let Json::Object(fields) = &mut data {
            fields.insert("recording_configured".into(), Json::Bool(false));
        }
        assert_eq!(
            Summary::parse(&data).unwrap().recording_configured,
            Some(false)
        );
    }

    #[test]
    fn saving_recording_requires_explicit_restart_and_keeps_history() {
        let mut app = App::unloaded(
            Backend {
                binary: "never-executed-zcalld".into(),
                config: None,
            },
            settings::Preferences::test_defaults(),
        );
        app.usage = json::parse(r#"{"requests":7,"failed":1,"input_tokens":100,"output_tokens":20,"recording_configured":false}"#).unwrap();
        app.sender
            .send(Reply {
                label: ENABLE_LABEL.into(),
                result: json::parse(
                    r#"{"ok":true,"recording_configured":true,"restart_required":true}"#,
                ),
                refresh: false,
                starts_gateway: false,
            })
            .unwrap();
        assert!(app.poll());
        assert!(!app.busy);
        assert!(app.usage_restart_pending);
        assert_eq!(
            Summary::parse(&app.usage).unwrap().recording_configured,
            Some(true)
        );
        assert!(app.receiver.try_recv().is_err());
        // Idempotent saving does not erase an already pending restart.
        app.accept_usage_recording(json::parse(
            r#"{"ok":true,"recording_configured":true,"restart_required":false}"#,
        ));
        assert!(app.usage_restart_pending);
        let summary = app.usage.clone();
        app.accept_usage(Ok(summary));
        assert!(app.usage_restart_pending);
        app.sender
            .send(Reply {
                label: "Restart gateway service".into(),
                result: json::parse(r#"{"ok":true}"#),
                refresh: false,
                starts_gateway: true,
            })
            .unwrap();
        assert!(app.poll());
        assert!(!app.usage_restart_pending);
        app.accept_usage_recording(json::parse(
            r#"{"ok":true,"recording_configured":false,"restart_required":true}"#,
        ));
        let summary = Summary::parse(&app.usage).unwrap();
        assert_eq!(summary.recording_configured, Some(false));
        assert_eq!(
            (summary.requests, summary.input, summary.output),
            (7, 100, 20)
        );
        assert!(app.usage_restart_pending);
        app.accept_usage_recording(Err("settings unavailable".into()));
        assert!(app.failed);
        assert_eq!(Summary::parse(&app.usage).unwrap(), summary);
    }

    #[test]
    fn malformed_or_failed_accounting_is_not_zero_usage() {
        for body in [
            "{}",
            r#"{"requests":"12","failed":0,"input_tokens":100,"output_tokens":50}"#,
            r#"{"requests":1,"failed":2,"input_tokens":100,"output_tokens":50}"#,
            r#"{"requests":1,"failed":0,"input_tokens":-1,"output_tokens":50}"#,
            r#"{"requests":1,"failed":0,"input_tokens":100,"output_tokens":50,"error":"read failed"}"#,
        ] {
            assert!(
                Summary::parse(&json::parse(body).unwrap()).is_none(),
                "{body}"
            );
        }
    }

    #[test]
    fn request_outcomes_and_large_token_counts_remain_exact() {
        let data = json::parse(r#"{"requests":1250,"failed":5,"input_tokens":18446744073709551615,"output_tokens":123456789}"#).unwrap();
        let summary = Summary::parse(&data).unwrap();
        assert_eq!(summary.succeeded(), 1245);
        assert_eq!(summary.success_rate(), "99.6%");
        assert_eq!(summary.input, u64::MAX);
        assert_eq!(count(summary.output), "123,456,789");
        assert_eq!(count(0), "0");
    }

    #[test]
    fn empty_history_has_no_invented_success_rate() {
        let data =
            json::parse(r#"{"requests":0,"failed":0,"input_tokens":0,"output_tokens":0}"#).unwrap();
        assert_eq!(Summary::parse(&data).unwrap().success_rate(), "—");
    }

    #[test]
    fn optional_details_preserve_older_gateways_and_do_not_change_totals() {
        let older =
            json::parse(r#"{"requests":1,"failed":0,"input_tokens":100,"output_tokens":50}"#)
                .unwrap();
        assert_eq!(Summary::parse(&older).unwrap().cache_read, None);
        let current = json::parse(r#"{"requests":1,"failed":0,"input_tokens":100,"output_tokens":50,"cache_read_tokens":80,"cache_write_tokens":20,"reasoning_tokens":30}"#).unwrap();
        let summary = Summary::parse(&current).unwrap();
        assert_eq!(summary.input + summary.output, 150);
        assert_eq!(summary.cache_read, Some(80));
        assert_eq!(summary.reasoning, Some(30));
    }

    #[test]
    fn accounting_replies_preserve_gateway_state_and_allow_a_failed_read_to_recover() {
        let mut app = App::unloaded(
            Backend {
                binary: "never-executed-zcalld".into(),
                config: None,
            },
            settings::Preferences::test_defaults(),
        );
        let snapshot = json::parse(r#"{"status":{"healthy":true}}"#).unwrap();
        app.state = snapshot.clone();
        app.sender
            .send(Reply {
                label: READ_LABEL.into(),
                result: Err("history is temporarily unavailable".into()),
                refresh: false,
                starts_gateway: false,
            })
            .unwrap();
        assert!(app.poll());
        assert!(app.failed);
        assert!(!app.busy);
        assert_eq!(app.state, snapshot);
        assert_eq!(
            app.usage.get("error").text(),
            "history is temporarily unavailable"
        );
        app.sender
            .send(Reply {
                label: READ_LABEL.into(),
                result: json::parse(
                    r#"{"requests":5,"failed":1,"input_tokens":100,"output_tokens":20}"#,
                ),
                refresh: false,
                starts_gateway: false,
            })
            .unwrap();
        assert!(app.poll());
        assert!(!app.failed);
        assert!(!app.busy);
        assert_eq!(app.state, snapshot);
        assert_eq!(Summary::parse(&app.usage).unwrap().succeeded(), 4);
    }
}
