//! Model listing helpers should consume ProviderRegistry::all_models().

use opi_ai::provider::{EventStream, ModelInfo, Provider, ProviderError, Request};
use opi_ai::stream::AssistantStreamEvent;
use opi_coding_agent::model_listing::model_entries_from_registry;

struct TestProvider {
    id: String,
    models: Vec<ModelInfo>,
}

impl Provider for TestProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn stream(&self, _request: Request) -> EventStream {
        let stream: Vec<Result<AssistantStreamEvent, ProviderError>> = Vec::new();
        Box::pin(futures_util::stream::iter(stream))
    }
}

fn model(id: &str, display_name: &str) -> ModelInfo {
    ModelInfo {
        id: id.into(),
        display_name: display_name.into(),
        context_window: 100_000,
        max_output_tokens: 4_096,
        supports_images: false,
        supports_streaming: true,
        supports_thinking: false,
    }
}

#[test]
fn model_entries_from_registry_include_overrides() {
    let mut registry = opi_ai::ProviderRegistry::new();
    registry
        .register_provider(Box::new(TestProvider {
            id: "provider-a".into(),
            models: vec![model("base", "Base")],
        }))
        .unwrap();
    registry
        .register_model("provider-a", model("extra", "Extra"))
        .unwrap();

    let entries = model_entries_from_registry(&registry);

    assert!(entries.iter().any(|entry| entry.provider_id == "provider-a"
        && entry.model_id == "base"
        && entry.display_name == "Base"));
    assert!(entries.iter().any(|entry| entry.provider_id == "provider-a"
        && entry.model_id == "extra"
        && entry.display_name == "Extra"));
}
