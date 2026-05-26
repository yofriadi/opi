//! Picker integration: bridges provider registry and session listing to
//! SelectItem for the SelectList widget (task 3.11).

use std::path::Path;

use opi_tui::select_list::SelectItem;

/// Collect SelectItem entries from all registered providers' model lists.
///
/// Each entry's `id` is the fully-qualified `provider:model` spec, `display`
/// is the model's display name, and `metadata` is the provider id.
pub fn model_picker_items(registry: &opi_ai::registry::ProviderRegistry) -> Vec<SelectItem> {
    let mut items = Vec::new();
    for provider_id in registry.provider_ids() {
        let Some(provider) = registry.get_provider(provider_id) else {
            continue;
        };
        for model in provider.models() {
            items.push(SelectItem {
                id: format!("{provider_id}:{}", model.id),
                display: model.display_name.clone(),
                metadata: provider_id.to_string(),
            });
        }
    }
    items
}

/// Collect SelectItem entries from session listing in the given directory.
///
/// Each entry's `id` is the session id, `display` is the cwd (truncated if
/// needed), and `metadata` is the timestamp.
pub fn session_picker_items(dir: &Path) -> Result<Vec<SelectItem>, std::io::Error> {
    let sessions = crate::session_cli::list_sessions(dir).unwrap_or_default();
    Ok(sessions
        .into_iter()
        .map(|s| {
            let cwd_short = if s.cwd.len() > 40 {
                format!("...{}", &s.cwd[s.cwd.len() - 37..])
            } else {
                s.cwd
            };
            SelectItem {
                id: s.id,
                display: cwd_short,
                metadata: s.timestamp,
            }
        })
        .collect())
}
