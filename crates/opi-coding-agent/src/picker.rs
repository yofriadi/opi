//! Picker integration: bridges provider registry and session listing to
//! SelectItem for the SelectList widget (task 3.11).

use std::path::Path;

use opi_agent::session_branch::{BranchInfo, SessionTree};
use opi_tui::select_list::SelectItem;

/// Collect SelectItem entries from all registered providers' model lists.
///
/// Each entry's `id` is the fully-qualified `provider:model` spec, `display`
/// is the model's display name, and `metadata` is the provider id.
pub fn model_picker_items(registry: &opi_ai::registry::ProviderRegistry) -> Vec<SelectItem> {
    registry
        .all_models()
        .into_iter()
        .map(|(provider_id, model)| SelectItem {
            id: format!("{provider_id}:{}", model.id),
            display: model.display_name.clone(),
            metadata: provider_id.to_string(),
        })
        .collect()
}

/// Collect SelectItem entries from one provider's advertised model list.
pub fn model_picker_items_from_provider(
    provider: &dyn opi_ai::provider::Provider,
) -> Vec<SelectItem> {
    let provider_id = provider.id();
    provider
        .models()
        .iter()
        .map(|model| SelectItem {
            id: format!("{provider_id}:{}", model.id),
            display: model.display_name.clone(),
            metadata: provider_id.to_string(),
        })
        .collect()
}

/// Collect SelectItem entries from a reconstructed session branch tree.
pub fn branch_picker_items(tree: &SessionTree) -> Vec<SelectItem> {
    let active_index = tree.active_branch_index();
    tree.branches()
        .iter()
        .enumerate()
        .map(|(index, branch)| branch_picker_item(branch, index, active_index == Some(index)))
        .collect()
}

fn branch_picker_item(branch: &BranchInfo, index: usize, is_active: bool) -> SelectItem {
    let name = if index == 0 && branch.fork_point.is_none() {
        "Trunk".to_owned()
    } else {
        format!("Branch {}", index + 1)
    };
    let display = match branch.summary.as_deref() {
        Some(summary) if !summary.is_empty() => format!("{name}: {summary}"),
        _ => name,
    };
    let mut metadata = format!(
        "{} entries, depth {}, tip {}",
        branch.entry_count, branch.depth, branch.tip_id
    );
    if is_active {
        metadata.push_str(", active");
    }
    SelectItem {
        id: branch.tip_id.clone(),
        display,
        metadata,
    }
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
                let start = s.cwd.floor_char_boundary(s.cwd.len() - 37);
                format!("...{}", &s.cwd[start..])
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
