use super::*;
use codex_protocol::protocol::ReviewDecision;

#[tokio::test]
async fn abort_is_reported_as_cancelled_not_user_rejected() {
    let err = ToolOrchestrator::reject_if_not_approved(
        /*session*/ None,
        /*guardian_review_id*/ None,
        ReviewDecision::Abort,
    )
    .await
    .expect_err("abort should reject execution");

    assert!(
        matches!(err, ToolError::Rejected(message) if message == "approval request was cancelled before the user approved or rejected it")
    );
}
