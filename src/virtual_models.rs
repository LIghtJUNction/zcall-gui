//! Virtual models are gateway routing groups over configured provider routes.
use crate::*;

const HEADER: &str = "virtual.provider";

#[derive(Default, PartialEq, Eq)]
enum Page {
    #[default]
    List,
    Create,
    Metadata,
}

#[derive(Default)]
pub(super) struct Panel {
    page: Page,
    groups: Json,
    routes: Json,
    catalog: Json,
    name: String,
    rotate: bool,
    selected: Vec<String>,
    metadata: String,
    query: String,
    pub(super) scroll: usize,
    error: Option<String>,
    restart: bool,
}

impl Panel {
    fn switch(&mut self, page: Page) {
        self.page = page;
        self.query.clear();
        self.scroll = 0;
        self.error = None;
    }

    fn definition(&self) -> Json {
        let mut fields = vec![
            (
                "mode".into(),
                Json::String(if self.rotate { "rotate" } else { "order" }.into()),
            ),
            (
                "members".into(),
                Json::Array(self.selected.iter().cloned().map(Json::String).collect()),
            ),
        ];
        fields.push(("affinity".into(), Json::String("none".into())));
        if !self.metadata.is_empty() {
            fields.push(("catalog_model".into(), Json::String(self.metadata.clone())));
        }
        Json::object(fields)
    }
}

impl App {
    pub(super) fn open_virtual_models(&mut self) {
        self.virtual_models = Some(Panel::default());
        self.ui = Ui::default();
        self.load_virtual_models();
    }

    fn load_virtual_models(&mut self) {
        self.busy = true;
        let backend = self.backend.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let groups = backend.run(&words(&["groups", "list"]), None)?;
                let routes = backend.run(&words(&["routes", "list"]), None)?;
                if !matches!(groups.get("groups"), Json::Array(_))
                    || !matches!(routes.get("routes"), Json::Array(_))
                {
                    return Err(tr("error.invalid_response").into());
                }
                Ok(Json::object([
                    ("groups".into(), groups.get("groups").clone()),
                    ("routes".into(), routes.get("routes").clone()),
                ]))
            })();
            let _ = sender.send(Reply {
                label: "Virtual models".into(),
                result,
                refresh: false,
                starts_gateway: false,
            });
        });
    }

    pub(super) fn accept_virtual_models(&mut self, label: &str, result: Result<Json, String>) {
        let Some(panel) = &mut self.virtual_models else {
            return;
        };
        match result {
            Err(error) => {
                panel.error = Some(error);
                self.notice = tr("virtual.failed").into();
                self.failed = true;
            }
            Ok(data) => {
                self.failed = false;
                panel.error = None;
                match label {
                    "Virtual models" => {
                        panel.groups = data.get("groups").clone();
                        panel.routes = data.get("routes").clone();
                        if !panel.restart {
                            self.notice = tr("status.refreshed").into();
                        }
                    }
                    "Virtual metadata" => {
                        if !matches!(data.get("models"), Json::Array(_)) {
                            panel.error = Some(tr("error.invalid_response").into());
                            return;
                        }
                        panel.catalog = data.get("models").clone();
                        self.notice = tr("virtual.metadata").into();
                    }
                    _ => {
                        panel.restart = data.get("restart_required").boolean();
                        panel.name.clear();
                        panel.selected.clear();
                        panel.metadata.clear();
                        panel.switch(Page::List);
                        self.notice = tr("virtual.restart").into();
                        self.load_virtual_models();
                    }
                }
            }
        }
    }

    pub(super) fn virtual_models_dialog(&mut self, c: &mut Canvas<'_>) {
        let r = crate::dialogs::sheet(c, tr(HEADER));
        let x = r.x + 32.;
        let width = r.w - 64.;
        let enabled = !self.busy && self.confirm.is_none();
        if self.ui.button(
            c,
            9700,
            Rect::new(r.x + r.w - 66., 25., 40., 40.),
            "×",
            Style::Quiet,
            enabled,
        ) {
            self.virtual_models = None;
            self.ui = Ui::default();
            self.repaint = true;
            return;
        }
        let Some(panel) = &mut self.virtual_models else {
            return;
        };
        let (title, status) = match panel.page {
            Page::List => (
                "virtual.models",
                if panel.restart {
                    "virtual.restart"
                } else {
                    "virtual.description"
                },
            ),
            Page::Create => ("virtual.create", "virtual.order_help"),
            Page::Metadata => ("virtual.metadata", "virtual.metadata_help"),
        };
        c.ellipsis(tr(title), x, 95., width, 25, true, TEXT);
        c.ellipsis(tr(status), x, 136., width, 13, false, MUTED);
        if let Some(error) = &panel.error {
            c.ellipsis(error, x, r.h - 111., width, 13, false, RED);
        } else if self.busy {
            c.ellipsis(tr("status.loading"), x, r.h - 111., width, 13, false, MUTED);
        }
        match panel.page {
            Page::List => {
                if self.ui.button(
                    c,
                    9701,
                    Rect::new(x, 165., 145., 40.),
                    tr("virtual.create"),
                    Style::Primary,
                    enabled && !panel.routes.array().is_empty(),
                ) {
                    panel.switch(Page::Create);
                    self.repaint = true;
                    return;
                }
                let rows = ((r.h - 347.) / 64.).max(1.) as usize;
                let groups = panel.groups.array();
                panel.scroll = panel.scroll.min(groups.len().saturating_sub(rows));
                if groups.is_empty() && enabled && panel.error.is_none() {
                    c.ellipsis(tr("virtual.empty"), x, 237., width, 15, false, MUTED);
                }
                for (index, group) in groups.iter().skip(panel.scroll).take(rows).enumerate() {
                    let y = 225. + 64. * index as f32;
                    c.rect(Rect::new(x, y, width, 55.), 7., BG);
                    c.ellipsis(
                        group.get("name").text(),
                        x + 13.,
                        y + 8.,
                        width - 26.,
                        15,
                        true,
                        TEXT,
                    );
                    let mode = mode_label(group.get("mode").text());
                    let count = group.get("members").array().len().to_string();
                    c.ellipsis(
                        &i18n::message("virtual.members", &[("count", &count), ("mode", mode)]),
                        x + 13.,
                        y + 31.,
                        width - 26.,
                        12,
                        false,
                        MUTED,
                    );
                }
                self.repaint |=
                    page_buttons(c, &mut self.ui, &mut panel.scroll, groups.len(), rows, r);
            }
            Page::Create => {
                self.ui.input(
                    c,
                    9710,
                    Rect::new(x, 161., width, 38.),
                    &mut panel.name,
                    tr("virtual.name"),
                    false,
                    enabled,
                );
                for (idx, mode) in ["order", "rotate"].into_iter().enumerate() {
                    let button_width = (width - 10.) / 2.;
                    if self.ui.button(
                        c,
                        9711 + idx as u64,
                        Rect::new(
                            x + idx as f32 * (button_width + 10.),
                            209.,
                            button_width,
                            32.,
                        ),
                        mode_label(mode),
                        if panel.rotate == (idx == 1) {
                            Style::Primary
                        } else {
                            Style::Secondary
                        },
                        enabled,
                    ) {
                        panel.rotate = idx == 1;
                    }
                }
                let label = if panel.metadata.is_empty() {
                    tr("virtual.metadata")
                } else {
                    &panel.metadata
                };
                if self.ui.button(
                    c,
                    9713,
                    Rect::new(x, 251., width, 32.),
                    label,
                    Style::Secondary,
                    enabled,
                ) {
                    panel.switch(Page::Metadata);
                    self.repaint = true;
                    if panel.catalog == Json::Null {
                        self.action("Virtual metadata", words(&["model-catalog", "list"]), None);
                    }
                    return;
                }
                if self.ui.input(
                    c,
                    9714,
                    Rect::new(x, 293., width, 33.),
                    &mut panel.query,
                    tr("virtual.search_routes"),
                    false,
                    enabled,
                ) {
                    panel.scroll = 0;
                }
                let query = panel.query.to_lowercase();
                let routes: Vec<_> = panel
                    .routes
                    .array()
                    .iter()
                    .filter(|row| {
                        let name = row.get("name").text();
                        !panel
                            .groups
                            .array()
                            .iter()
                            .any(|group| group.get("name").text() == name)
                            && ["name", "provider", "model"]
                                .into_iter()
                                .any(|key| row.get(key).text().to_lowercase().contains(&query))
                    })
                    .collect();
                let rows = ((r.h - 458.) / 48.).max(1.) as usize;
                panel.scroll = panel.scroll.min(routes.len().saturating_sub(rows));
                for (index, route) in routes.iter().skip(panel.scroll).take(rows).enumerate() {
                    let y = 338. + 48. * index as f32;
                    let name = route.get("name").text();
                    let selection = panel.selected.iter().position(|entry| entry == name);
                    if self.ui.button(
                        c,
                        9800 + index as u64,
                        Rect::new(x, y, width, 44.),
                        "",
                        if selection.is_some() {
                            Style::Secondary
                        } else {
                            Style::Quiet
                        },
                        enabled,
                    ) {
                        if let Some(selection) = selection {
                            panel.selected.remove(selection);
                        } else {
                            panel.selected.push(name.into());
                        }
                        self.repaint = true;
                    }
                    let mark =
                        selection.map_or_else(|| "+".into(), |index| (index + 1).to_string());
                    c.text(&mark, x + 10., y + 13., 14, true, ACCENT);
                    c.ellipsis(name, x + 39., y + 6., width - 49., 13, true, TEXT);
                    c.ellipsis(
                        route.get("provider").text(),
                        x + 39.,
                        y + 24.,
                        width - 49.,
                        12,
                        false,
                        MUTED,
                    );
                }
                self.repaint |=
                    page_buttons(c, &mut self.ui, &mut panel.scroll, routes.len(), rows, r);
                if self.ui.button(
                    c,
                    9715,
                    Rect::new(x, r.h - 62., 90., 38.),
                    tr("action.back"),
                    Style::Quiet,
                    enabled,
                ) {
                    panel.switch(Page::List);
                    self.repaint = true;
                }
                if self.ui.button(
                    c,
                    9716,
                    Rect::new(x + width - 119., r.h - 62., 119., 38.),
                    tr("virtual.save"),
                    Style::Primary,
                    enabled && !panel.name.trim().is_empty() && !panel.selected.is_empty(),
                ) {
                    let args = words(&["groups", "set", panel.name.trim(), "--stdin"]);
                    let definition = panel.definition().stringify().into_bytes();
                    let name = panel.name.trim();
                    if panel
                        .routes
                        .array()
                        .iter()
                        .any(|route| route.get("name").text() == name)
                    {
                        panel.error = Some(tr("virtual.duplicate").into());
                        self.repaint = true;
                    } else {
                        self.action("Save virtual model", args, Some(definition));
                    }
                }
            }
            Page::Metadata => {
                if self.ui.input(
                    c,
                    9720,
                    Rect::new(x, 162., width - 94., 37.),
                    &mut panel.query,
                    tr("providers.search_models"),
                    false,
                    enabled,
                ) {
                    panel.scroll = 0;
                }
                if self.ui.button(
                    c,
                    9721,
                    Rect::new(x + width - 84., 162., 84., 37.),
                    tr("action.refresh"),
                    Style::Secondary,
                    enabled,
                ) {
                    self.action(
                        "Virtual metadata",
                        words(&["model-catalog", "refresh"]),
                        None,
                    );
                    return;
                }
                let query = panel.query.to_lowercase();
                let rows = ((r.h - 341.) / 48.).max(1.) as usize;
                let models: Vec<_> = panel
                    .catalog
                    .array()
                    .iter()
                    .filter(|m| {
                        ["id", "name", "type"]
                            .into_iter()
                            .any(|key| m.get(key).text().to_lowercase().contains(&query))
                    })
                    .collect();
                panel.scroll = panel.scroll.min(models.len().saturating_sub(rows));
                for (index, model) in models.iter().skip(panel.scroll).take(rows).enumerate() {
                    let y = 214. + index as f32 * 48.;
                    if self.ui.button(
                        c,
                        9900 + index as u64,
                        Rect::new(x, y, width, 44.),
                        "",
                        Style::Quiet,
                        enabled,
                    ) {
                        panel.metadata = model.get("id").text().into();
                        panel.switch(Page::Create);
                        self.repaint = true;
                        return;
                    }
                    c.ellipsis(
                        model.get("name").text(),
                        x + 10.,
                        y + 6.,
                        width - 20.,
                        14,
                        true,
                        TEXT,
                    );
                    c.ellipsis(
                        model.get("id").text(),
                        x + 10.,
                        y + 25.,
                        width - 20.,
                        12,
                        false,
                        MUTED,
                    );
                }
                self.repaint |=
                    page_buttons(c, &mut self.ui, &mut panel.scroll, models.len(), rows, r);
                if self.ui.button(
                    c,
                    9722,
                    Rect::new(x, r.h - 62., 90., 38.),
                    tr("action.back"),
                    Style::Quiet,
                    enabled,
                ) {
                    panel.switch(Page::Create);
                    self.repaint = true;
                }
                if !panel.metadata.is_empty()
                    && self.ui.button(
                        c,
                        9723,
                        Rect::new(x + width - 119., r.h - 62., 119., 38.),
                        tr("virtual.clear_metadata"),
                        Style::Secondary,
                        enabled,
                    )
                {
                    panel.metadata.clear();
                    panel.switch(Page::Create);
                    self.repaint = true;
                }
            }
        }
    }
}

fn mode_label(mode: &str) -> &str {
    match mode {
        "order" => tr("virtual.order"),
        "rotate" => tr("virtual.rotate"),
        other => other,
    }
}

fn page_buttons(
    c: &mut Canvas<'_>,
    ui: &mut Ui,
    scroll: &mut usize,
    len: usize,
    rows: usize,
    r: Rect,
) -> bool {
    let before = *scroll;
    let end = r.x + r.w - 32.;
    let y = r.h - 99.;
    if len <= rows {
        return false;
    }
    if ui.button(
        c,
        9790,
        Rect::new(end - 79., y, 36., 29.),
        "‹",
        Style::Quiet,
        *scroll > 0,
    ) {
        *scroll = scroll.saturating_sub(rows);
    }
    if ui.button(
        c,
        9791,
        Rect::new(end - 36., y, 36., 29.),
        "›",
        Style::Quiet,
        *scroll + rows < len,
    ) {
        *scroll = (*scroll + rows).min(len - rows);
    }
    before != *scroll
}
