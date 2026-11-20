//! Interpret a completed local snapshot separately from the first pending read.
//! Missing or malformed responses are unavailable, never an empty configuration.
use crate::json::Json;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadState {
    Loading,
    Unavailable,
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayState {
    Loading,
    Unavailable,
    Offline,
    Healthy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewAction {
    Wait,
    Retry,
    StartGateway,
    AddProvider,
    ConnectTools,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogState {
    Loading,
    Unavailable,
    Empty,
    Available,
}

fn section_state(snapshot: &Json, name: &str) -> LoadState {
    if matches!(snapshot, Json::Null) {
        return LoadState::Loading;
    }
    let section = snapshot.get(name);
    if !matches!(section, Json::Object(_)) {
        return LoadState::Unavailable;
    }
    match section.get("error") {
        Json::Null => LoadState::Ready,
        Json::String(error) if error.trim().is_empty() => LoadState::Ready,
        _ => LoadState::Unavailable,
    }
}

pub fn collection_state(snapshot: &Json, section: &str, field: &str) -> LoadState {
    match section_state(snapshot, section) {
        LoadState::Ready if !matches!(snapshot.get(section).get(field), Json::Array(_)) => {
            LoadState::Unavailable
        }
        state => state,
    }
}

pub fn privacy_state(snapshot: &Json) -> LoadState {
    match section_state(snapshot, "privacy") {
        LoadState::Ready => {
            let privacy = snapshot.get("privacy");
            if matches!(privacy.get("mode"), Json::String(mode) if !mode.trim().is_empty())
                || matches!(privacy.get("enabled"), Json::Bool(_))
            {
                LoadState::Ready
            } else {
                LoadState::Unavailable
            }
        }
        state => state,
    }
}

pub fn gateway_state(snapshot: &Json) -> GatewayState {
    match section_state(snapshot, "status") {
        LoadState::Loading => GatewayState::Loading,
        LoadState::Unavailable => GatewayState::Unavailable,
        LoadState::Ready => match snapshot.get("status").get("healthy") {
            Json::Bool(true) => GatewayState::Healthy,
            Json::Bool(false) => GatewayState::Offline,
            _ => GatewayState::Unavailable,
        },
    }
}

pub fn endpoint(snapshot: &Json) -> Option<&str> {
    match snapshot.get("status").get("listen") {
        Json::String(endpoint) if !endpoint.trim().is_empty() => Some(endpoint),
        _ => None,
    }
}

pub fn overview_action(snapshot: &Json) -> OverviewAction {
    match gateway_state(snapshot) {
        GatewayState::Loading => OverviewAction::Wait,
        GatewayState::Unavailable => OverviewAction::Retry,
        GatewayState::Offline => OverviewAction::StartGateway,
        GatewayState::Healthy => match collection_state(snapshot, "providers", "providers") {
            LoadState::Loading => OverviewAction::Wait,
            LoadState::Unavailable => OverviewAction::Retry,
            LoadState::Ready
                if snapshot
                    .get("providers")
                    .get("providers")
                    .array()
                    .is_empty() =>
            {
                OverviewAction::AddProvider
            }
            LoadState::Ready => OverviewAction::ConnectTools,
        },
    }
}

pub fn catalog_state(snapshot: &Json) -> CatalogState {
    match collection_state(snapshot, "models", "data") {
        LoadState::Loading => CatalogState::Loading,
        LoadState::Unavailable => CatalogState::Unavailable,
        LoadState::Ready => {
            let routes = snapshot.get("models").get("data").array();
            if routes.is_empty() {
                CatalogState::Empty
            } else if routes
                .iter()
                .any(|route| matches!(route.get("id"), Json::String(id) if !id.trim().is_empty()))
            {
                CatalogState::Available
            } else {
                CatalogState::Unavailable
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    #[test]
    fn initial_read_does_not_claim_a_disconnection_or_an_empty_configuration() {
        assert_eq!(gateway_state(&Json::Null), GatewayState::Loading);
        assert_eq!(overview_action(&Json::Null), OverviewAction::Wait);
        assert_eq!(
            collection_state(&Json::Null, "providers", "providers"),
            LoadState::Loading
        );
        assert_eq!(privacy_state(&Json::Null), LoadState::Loading);
        assert_eq!(catalog_state(&Json::Null), CatalogState::Loading);
        assert_eq!(endpoint(&Json::Null), None);
    }

    #[test]
    fn failed_or_malformed_status_is_unknown_instead_of_offline() {
        for body in [
            r#"{}"#,
            r#"{"status":null}"#,
            r#"{"status":{}}"#,
            r#"{"status":{"healthy":"false"}}"#,
            r#"{"status":{"error":"invalid configuration","healthy":false}}"#,
            r#"{"status":{"error":{"message":"permission denied"},"healthy":true}}"#,
        ] {
            let snapshot = parse(body).unwrap();
            assert_eq!(
                gateway_state(&snapshot),
                GatewayState::Unavailable,
                "{body}"
            );
            assert_eq!(overview_action(&snapshot), OverviewAction::Retry, "{body}");
        }
    }

    #[test]
    fn explicit_health_results_distinguish_offline_from_connected() {
        let offline = parse(r#"{"status":{"healthy":false}}"#).unwrap();
        let healthy = parse(r#"{"status":{"healthy":true}}"#).unwrap();
        assert_eq!(gateway_state(&offline), GatewayState::Offline);
        assert_eq!(overview_action(&offline), OverviewAction::StartGateway);
        assert_eq!(gateway_state(&healthy), GatewayState::Healthy);
    }

    #[test]
    fn endpoint_comes_only_from_the_observed_status() {
        for body in [
            r#"{}"#,
            r#"{"status":{"listen":" "}}"#,
            r#"{"status":{"listen":9415}}"#,
        ] {
            assert_eq!(endpoint(&parse(body).unwrap()), None);
        }
        let snapshot = parse(r#"{"status":{"listen":"127.0.0.1:3425"}}"#).unwrap();
        assert_eq!(endpoint(&snapshot), Some("127.0.0.1:3425"));
    }

    #[test]
    fn only_a_successful_array_can_be_shown_as_an_empty_list() {
        for body in [
            r#"{}"#,
            r#"{"providers":null}"#,
            r#"{"providers":{}}"#,
            r#"{"providers":{"providers":null}}"#,
            r#"{"providers":{"error":"read failed","providers":[]}}"#,
        ] {
            assert_eq!(
                collection_state(&parse(body).unwrap(), "providers", "providers"),
                LoadState::Unavailable,
                "{body}"
            );
        }
        let snapshot = parse(r#"{"providers":{"providers":[]}}"#).unwrap();
        assert_eq!(
            collection_state(&snapshot, "providers", "providers"),
            LoadState::Ready
        );
    }

    #[test]
    fn overview_retries_failed_provider_reads_instead_of_starting_setup() {
        let failed =
            parse(r#"{"status":{"healthy":true},"providers":{"error":"read failed"}}"#).unwrap();
        let empty = parse(r#"{"status":{"healthy":true},"providers":{"providers":[]}}"#).unwrap();
        let configured =
            parse(r#"{"status":{"healthy":true},"providers":{"providers":[{"name":"local"}]}}"#)
                .unwrap();
        assert_eq!(overview_action(&failed), OverviewAction::Retry);
        assert_eq!(overview_action(&empty), OverviewAction::AddProvider);
        assert_eq!(overview_action(&configured), OverviewAction::ConnectTools);
    }

    #[test]
    fn catalog_failure_and_invalid_responses_are_not_empty_catalogs() {
        for body in [
            r#"{}"#,
            r#"{"models":null}"#,
            r#"{"models":{"error":"catalog unavailable","data":[]}}"#,
            r#"{"models":{"data":null}}"#,
            r#"{"models":{"data":[{"id":" "}]}}"#,
            r#"{"models":{"data":[{"id":42}]}}"#,
        ] {
            assert_eq!(
                catalog_state(&parse(body).unwrap()),
                CatalogState::Unavailable,
                "{body}"
            );
        }
        let empty = parse(r#"{"models":{"data":[]}}"#).unwrap();
        let available = parse(r#"{"models":{"data":[{"id":"public-route"}]}}"#).unwrap();
        assert_eq!(catalog_state(&empty), CatalogState::Empty);
        assert_eq!(catalog_state(&available), CatalogState::Available);
    }

    #[test]
    fn unknown_privacy_state_is_not_reported_as_disabled() {
        for body in [
            r#"{}"#,
            r#"{"privacy":{}}"#,
            r#"{"privacy":{"error":"unavailable"}}"#,
        ] {
            assert_eq!(privacy_state(&parse(body).unwrap()), LoadState::Unavailable);
        }
        for body in [
            r#"{"privacy":{"mode":"off"}}"#,
            r#"{"privacy":{"enabled":false}}"#,
        ] {
            assert_eq!(privacy_state(&parse(body).unwrap()), LoadState::Ready);
        }
    }
}
