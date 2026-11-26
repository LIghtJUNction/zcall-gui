use super::*;

pub(super) const STATS: &str = "privacy-stats";

fn number(value: &Json) -> u64 {
    value.text().parse().unwrap_or(0)
}

impl App {
    pub(super) fn refresh_privacy(&mut self) {
        if self.privacy_reading {
            return;
        }
        self.privacy_reading = true;
        self.privacy_request = Some(Instant::now());
        self.backend.spawn(
            self.sender.clone(),
            STATS.into(),
            words(&["privacy", "stats"]),
            None,
        );
    }

    pub(super) fn privacy_chart(&mut self, c: &mut Canvas<'_>, x: f32, y: f32, width: f32) {
        c.text(tr("privacy.stats"), x + 2., y, 19, true, TEXT);
        if self.ui.button(
            c,
            816,
            Rect::new(x + width - 85., y - 7., 85., 36.),
            tr("action.refresh"),
            Style::Quiet,
            self.enabled(),
        ) {
            self.refresh_privacy();
        }
        let stats = &self.privacy_stats;
        let rows = stats.get("days").array();
        if rows.is_empty() {
            let message = if stats == &Json::Null {
                tr("status.loading")
            } else if !stats.get("error").text().is_empty() || number(stats.get("skipped_days")) > 0
            {
                tr("privacy.stats_unavailable")
            } else {
                tr("privacy.stats_empty")
            };
            c.ellipsis(message, x + 2., y + 43., width - 4., 14, false, MUTED);
            self.privacy_events(c, x, y + 81., width);
            return;
        }
        let total = |key| {
            rows.iter()
                .map(|day| number(day.get(key)))
                .fold(0u64, u64::saturating_add)
                .to_string()
        };
        let checked = total("checked");
        let masked = total("masked");
        let text = i18n::message(
            "privacy.stats_totals",
            &[("checked", &checked), ("masked", &masked)],
        );
        c.ellipsis(&text, x + 2., y + 34., width - 4., 14, false, MUTED);
        let skipped = number(stats.get("skipped_days"));
        if skipped != 0 {
            c.ellipsis(
                tr("privacy.stats_partial"),
                x + 2.,
                y + 57.,
                width - 4.,
                12,
                false,
                MUTED,
            );
        }
        let chart_y = y + if skipped == 0 { 67. } else { 84. };
        let bottom = chart_y + 89.;
        let backdrop = Rect::new(x, chart_y, width, bottom - chart_y + 30.);
        c.rect(backdrop, 8., SURFACE);
        let top = chart_y + 23.;
        let span = (width - 32.) / rows.len() as f32;
        let peak = rows
            .iter()
            .map(|day| number(day.get("checked")))
            .max()
            .unwrap_or(0)
            .max(1) as f32;
        let bar_width = (span * 0.43).clamp(10., 66.);
        for (i, row) in rows.iter().enumerate() {
            let middle = x + 16. + (i as f32 + 0.5) * span;
            let shade = |key| (bottom - top) * number(row.get(key)) as f32 / peak;
            for (key, color) in [("checked", ACCENT_SOFT), ("masked", ACCENT)] {
                let height = shade(key);
                if height > 0. {
                    c.rect(
                        Rect::new(middle - bar_width / 2., bottom - height, bar_width, height),
                        4.,
                        color,
                    );
                }
            }
            let date = row.get("date").text().get(5..).unwrap_or_default();
            let label = format!(
                "{}/{}",
                number(row.get("checked")),
                number(row.get("masked"))
            );
            if span >= 45. {
                let label_y = (bottom - shade("checked") - 17.).max(chart_y + 1.);
                c.text(
                    &label,
                    middle - c.measure(&label, 11, false) / 2.,
                    label_y,
                    11,
                    false,
                    MUTED,
                );
            }
            c.text(
                date,
                middle - c.measure(date, 11, false) / 2.,
                bottom + 8.,
                11,
                false,
                MUTED,
            );
        }
        self.privacy_events(c, x, chart_y + 135., width);
    }

    fn privacy_events(&mut self, c: &mut Canvas<'_>, x: f32, y: f32, width: f32) {
        c.text(tr("privacy.events"), x + 2., y, 18, true, TEXT);
        let rows = self.privacy_stats.get("events").array();
        let available = ((c.height - y - 79.) / 31.).floor().max(1.) as usize;
        self.scroll = self.scroll.min(rows.len().saturating_sub(available));
        if rows.len() > available {
            for (id, offset, icon, enabled) in [
                (817, 84., "‹", self.scroll > 0),
                (818, 40., "›", self.scroll + available < rows.len()),
            ] {
                if self.ui.button(
                    c,
                    id,
                    Rect::new(x + width - offset, y - 5., 40., 30.),
                    icon,
                    Style::Quiet,
                    self.enabled() && enabled,
                ) {
                    if id == 817 {
                        self.scroll = self.scroll.saturating_sub(available);
                    } else {
                        self.scroll =
                            (self.scroll + available).min(rows.len().saturating_sub(available));
                    }
                }
            }
        }
        if rows.is_empty() {
            let empty = if self.privacy_stats == Json::Null {
                "status.loading"
            } else if self.privacy_stats.get("events_partial").boolean()
                || !self.privacy_stats.get("error").text().is_empty()
            {
                "privacy.events_unavailable"
            } else {
                "privacy.events_empty"
            };
            c.ellipsis(tr(empty), x + 2., y + 37., width - 4., 13, false, MUTED);
        }
        for (index, row) in rows.iter().skip(self.scroll).take(available).enumerate() {
            let top = y + 31. + index as f32 * 31.;
            c.rect(Rect::new(x, top, width, 27.), 6., SURFACE);
            let time = row.get("time").text();
            c.ellipsis(
                time.get(5..).unwrap_or_default(),
                x + 12.,
                top + 6.,
                121.,
                12,
                false,
                MUTED,
            );
            let count = number(row.get("replacements")).to_string();
            let result = i18n::message(
                if row.get("blocked").boolean() {
                    "privacy.event_blocked"
                } else {
                    "privacy.event_masked"
                },
                &[("count", &count)],
            );
            c.ellipsis(&result, x + 143., top + 6., width - 155., 12, true, TEXT);
        }
    }

    pub(super) fn privacy_options(&mut self, c: &mut Canvas<'_>) {
        let r = crate::dialogs::sheet(c, tr("privacy.advanced"));
        let x = r.x + 29.;
        let width = r.w - 58.;
        let enabled = !self.busy;
        if self.ui.button(
            c,
            823,
            Rect::new(r.x + r.w - 66., 25., 40., 40.),
            "×",
            Style::Quiet,
            enabled,
        ) {
            self.privacy_panel = false;
            self.repaint = true;
            return;
        }
        let state = self.state.get("privacy");
        let ready = !state.get("mode").text().is_empty();
        let word_count = number(state.get("custom_words")).to_string();
        let word_info = i18n::message("privacy.custom_word_count", &[("count", &word_count)]);
        c.ellipsis(&word_info, x, 91., width, 13, false, MUTED);
        let options = [
            (
                "model",
                "model_enabled",
                "privacy.option_model",
                "privacy.option_model_help",
            ),
            (
                "ignore-case",
                "words_any_case",
                "privacy.option_case",
                "privacy.option_case_help",
            ),
            (
                "whole-word",
                "words_whole",
                "privacy.option_whole",
                "privacy.option_whole_help",
            ),
            (
                "shape",
                "shape",
                "privacy.option_shape",
                "privacy.option_shape_help",
            ),
        ];
        for (index, (field, key, title, help)) in options.into_iter().enumerate() {
            let top = 129. + index as f32 * 78.;
            let on = state.get(key).boolean();
            c.ellipsis(tr(title), x, top + 3., width - 94., 15, true, TEXT);
            c.ellipsis(tr(help), x, top + 30., width - 94., 12, false, MUTED);
            if self.ui.button(
                c,
                824 + index as u64,
                Rect::new(x + width - 79., top, 79., 36.),
                tr(if on {
                    "status.enabled"
                } else {
                    "status.disabled"
                }),
                if on { Style::Primary } else { Style::Secondary },
                enabled && ready,
            ) {
                self.action(
                    tr(title),
                    words(&["privacy", "configure", field, if on { "off" } else { "on" }]),
                    None,
                );
                return;
            }
        }
    }
}
