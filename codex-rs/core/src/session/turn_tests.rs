use super::*;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputContributor;
use codex_extension_api::TurnItemContributor;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::user_input::UserInput;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

struct RewriteAgentMessageContributor;

struct PendingTurnInputContributor;

impl TurnItemContributor for RewriteAgentMessageContributor {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        item: &'a mut TurnItem,
    ) -> codex_extension_api::ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if let TurnItem::AgentMessage(agent_message) = item {
                agent_message.content = vec![AgentMessageContent::Text {
                    text: "plan contributed assistant text".to_string(),
                }];
            }
            Ok(())
        })
    }
}

impl TurnInputContributor for PendingTurnInputContributor {
    fn contribute<'a>(
        &'a self,
        _input: TurnInputContext,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
    ) -> codex_extension_api::ExtensionFuture<
        'a,
        Vec<Box<dyn codex_extension_api::ContextualUserFragment + Send>>,
    > {
        Box::pin(std::future::pending())
    }
}

fn assistant_output_text(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: Some("msg-1".to_string()),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

#[tokio::test]
async fn run_turn_runs_pending_session_start_hooks_before_cancellable_setup() {
    let (mut session, turn_context) = crate::session::tests::make_session_and_context().await;
    {
        let mut state = session.state.lock().await;
        state.queue_pending_session_start_source(codex_hooks::SessionStartSource::Startup);
    }
    let mut builder = ExtensionRegistryBuilder::new();
    builder.turn_input_contributor(Arc::new(PendingTurnInputContributor));
    session.services.extensions = Arc::new(builder.build());
    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context);
    let cancellation_token = CancellationToken::new();
    cancellation_token.cancel();

    let result = run_turn(
        Arc::clone(&session),
        turn_context,
        Arc::new(ExtensionData::new("turn".to_string())),
        vec![TurnInput::UserInput {
            content: vec![UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            client_id: None,
        }],
        /*prewarmed_client_session*/ None,
        cancellation_token,
    )
    .await;

    assert_eq!(
        result.expect("turn should exit without model sampling"),
        None
    );
    assert!(session.take_pending_session_start_source().await.is_none());
}

#[tokio::test]
async fn plan_mode_uses_contributed_turn_item_for_last_agent_message() {
    let (mut session, turn_context) = crate::session::tests::make_session_and_context().await;
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_item_contributor(Arc::new(RewriteAgentMessageContributor));
    session.services.extensions = Arc::new(builder.build());
    let turn_store = ExtensionData::new(turn_context.sub_id.clone());
    let mut state = PlanModeStreamState::new(&turn_context.sub_id);
    let mut last_agent_message = None;
    let item = assistant_output_text("original assistant text");

    let handled = handle_assistant_item_done_in_plan_mode(
        &session,
        &turn_context,
        &turn_store,
        &item,
        &mut state,
        /*previously_active_item*/ None,
        &mut last_agent_message,
    )
    .await;

    assert!(handled);
    assert_eq!(
        last_agent_message.as_deref(),
        Some("plan contributed assistant text")
    );
}
