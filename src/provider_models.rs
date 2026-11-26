//! Browse each provider's catalog through the gateway's management CLI.
use std::collections::BTreeSet;

use crate::*;

struct Model {
    id: String,
    label: String,
    detail: String,
}

pub(super) struct Browser {
    provider: String,
    label: String,
    models: Option<Vec<Model>>,
    error: Option<String>,
    configured: bool,
    query: String,
    pub(super) scroll: usize,
}

fn catalog(data: &Json) -> Option<Vec<Model>> {
    let data = data.get("catalog");
    let rows = ["data", "models", "items"]
        .iter()
        .find_map(|key| match data.get(key) {
            Json::Array(rows) => Some(rows.as_slice()),
            _ => None,
        })
        .or(match data {
            Json::Array(rows) => Some(rows.as_slice()),
            _ => None,
        })?;
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for row in rows {
        let id = value(
            row,
            "id",
            value(row, "name", value(row, "model", row.text())),
        );
        if id.is_empty() || !seen.insert(id) {
            continue;
        }
        let label = value(row, "display_name", value(row, "displayName", id));
        let detail = value(row, "description", value(row, "owned_by", ""));
        items.push(Model {
            id: id.into(),
            label: label.into(),
            detail: detail.into(),
        });
    }
    items.sort_by(|left, right| left.id.cmp(&right.id));
    Some(items)
}

impl Browser {
    pub(super) fn accept(&mut self, result: Result<Json, String>) -> bool {
        match result.and_then(|data| {
            let models = catalog(&data).ok_or_else(|| tr("error.invalid_response").to_string())?;
            Ok((models, data.get("source").text() == "configured_catalog"))
        }) {
            Ok((models, configured)) => {
                self.models = Some(models);
                self.configured = configured;
                self.error = None;
                self.scroll = 0;
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }
}

impl App {
    pub(super) fn open_provider_models(&mut self, provider: String, label: String) {
        self.provider_models = Some(Browser {
            provider,
            label,
            models: None,
            error: None,
            configured: false,
            query: String::new(),
            scroll: 0,
        });
        self.ui = Ui::default();
        self.fetch_provider_models();
    }

    fn fetch_provider_models(&mut self) {
        if let Some(browser) = self.provider_models.as_mut() {
            browser.error = None;
            let provider = browser.provider.clone();
            self.action(
                "Fetch provider models",
                words(&["providers", "models", &provider]),
                None,
            );
        }
    }

    pub(super) fn provider_models_dialog(&mut self, c: &mut Canvas<'_>) {
        let r = crate::dialogs::sheet(c, tr("providers.browse_models"));
        let x = r.x + 32.;
        let width = r.w - 64.;
        if self.ui.button(
            c,
            9600,
            Rect::new(r.x + r.w - 66., 25., 40., 40.),
            "×",
            Style::Quiet,
            true,
        ) {
            self.provider_models = None;
            self.ui = Ui::default();
            self.repaint = true;
            return;
        }
        if self.ui.button(
            c,
            9601,
            Rect::new(x + width - 94., 86., 94., 36.),
            tr("action.refresh"),
            Style::Secondary,
            !self.busy,
        ) {
            self.fetch_provider_models();
        }
        let Some(browser) = self.provider_models.as_mut() else {
            return;
        };
        c.ellipsis(&browser.label, x, 93., width - 108., 22, true, TEXT);
        if self.ui.input(
            c,
            9602,
            Rect::new(x, 139., width, 42.),
            &mut browser.query,
            tr("providers.search_models"),
            false,
            true,
        ) {
            browser.scroll = 0;
        }
        if let Some(error) = &browser.error {
            c.ellipsis(error, x, 200., width, 13, false, RED);
        } else if self.busy {
            c.text(tr("status.loading"), x, 200., 13, false, MUTED);
        } else {
            c.text(
                tr(if browser.configured {
                    "providers.catalog_local"
                } else {
                    "providers.catalog_upstream"
                }),
                x,
                200.,
                13,
                false,
                MUTED,
            );
        }
        let Some(models) = &browser.models else {
            return;
        };
        let query = browser.query.trim().to_lowercase();
        let matches: Vec<_> = models
            .iter()
            .filter(|m| {
                m.id.to_lowercase().contains(&query) || m.label.to_lowercase().contains(&query)
            })
            .collect();
        if matches.is_empty() {
            c.text(tr("providers.models_empty"), x, 244., 15, false, MUTED);
            return;
        }
        let rows = ((r.h - 292.) / 60.).max(1.) as usize;
        browser.scroll = browser.scroll.min(matches.len().saturating_sub(rows));
        for (index, model) in matches.iter().skip(browser.scroll).take(rows).enumerate() {
            let y = 232. + index as f32 * 60.;
            c.rect(Rect::new(x, y, width, 52.), 8., BG);
            c.ellipsis(&model.label, x + 12., y + 6., width - 93., 15, true, TEXT);
            c.ellipsis(
                if model.label != model.id {
                    &model.id
                } else {
                    &model.detail
                },
                x + 12.,
                y + 29.,
                width - 93.,
                12,
                false,
                MUTED,
            );
            if self.ui.button(
                c,
                9650 + index as u64,
                Rect::new(x + width - 70., y + 6., 70., 32.),
                tr("providers.copy_model"),
                Style::Quiet,
                true,
            ) {
                self.ui.clipboard = Some(model.id.clone());
            }
        }
        if self.ui.button(
            c,
            9604,
            Rect::new(x + width - 88., r.h - 49., 40., 33.),
            "‹",
            Style::Secondary,
            browser.scroll > 0,
        ) {
            browser.scroll = browser.scroll.saturating_sub(rows);
            self.repaint = true;
        }
        if self.ui.button(
            c,
            9605,
            Rect::new(x + width - 41., r.h - 49., 40., 33.),
            "›",
            Style::Secondary,
            browser.scroll + rows < matches.len(),
        ) {
            browser.scroll += rows;
            self.repaint = true;
        }
        c.ellipsis(
            &i18n::message(
                "providers.models_range",
                &[
                    ("start", &(browser.scroll + 1).to_string()),
                    (
                        "end",
                        &(browser.scroll + rows).min(matches.len()).to_string(),
                    ),
                    ("total", &matches.len().to_string()),
                ],
            ),
            x,
            r.h - 43.,
            width - 99.,
            12,
            false,
            MUTED,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_distinct_upstream_model_shapes() {
        let payload = json::parse(r#"{"catalog":{"models":[{"name":"models/flash","displayName":"Flash"},{"id":"fast"},{"id":"fast"},{"model":"local"}]}}"#).unwrap();
        let models = catalog(&payload).unwrap();
        assert_eq!(
            models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["fast", "local", "models/flash"]
        );
        assert_eq!(models[2].label, "Flash");
        assert!(catalog(&Json::Null).is_none());
    }
}
