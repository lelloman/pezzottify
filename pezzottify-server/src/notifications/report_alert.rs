//! Notifies report triagers when a user submits a bug report or feedback.

use tracing::warn;

use crate::db_executor::{DbHandle, DbPriority};
use crate::user::{Permission, UserManager};

use super::models::{BugReportSubmittedData, NotificationType};
use super::service::NotificationService;

/// Users who should hear about a new report: everyone with `TriageReports`,
/// except the reporter (they already know).
pub fn report_triager_ids(manager: &UserManager, reporter_id: usize) -> anyhow::Result<Vec<usize>> {
    let mut ids = Vec::new();
    for handle in manager.get_all_user_handles()? {
        let Some(user_id) = manager.get_user_id(&handle)? else {
            continue;
        };
        if user_id == reporter_id {
            continue;
        }
        if manager
            .get_user_permissions(user_id)?
            .contains(&Permission::TriageReports)
        {
            ids.push(user_id);
        }
    }
    Ok(ids)
}

fn notification_text(data: &BugReportSubmittedData) -> (String, String) {
    let title = if data.kind == "feature" {
        "New feedback".to_string()
    } else {
        "New bug report".to_string()
    };
    let summary = data
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or("(untitled)");
    let body = format!("{} · {}: {}", data.reporter_handle, data.category, summary);
    (title, body)
}

/// Create a `BugReportSubmitted` notification for every triager. Failures are
/// logged and never propagate: the report itself is already stored.
pub async fn notify_report_triagers(
    user_manager: DbHandle<UserManager>,
    service: NotificationService,
    reporter_id: usize,
    data: BugReportSubmittedData,
) {
    let recipients = match user_manager
        .run(DbPriority::Background, move |m| {
            report_triager_ids(m, reporter_id)
        })
        .await
    {
        Ok(ids) => ids,
        Err(error) => {
            warn!("Failed to resolve report triagers: {error}");
            return;
        }
    };
    let (title, body) = notification_text(&data);
    let payload = match serde_json::to_value(&data) {
        Ok(v) => v,
        Err(error) => {
            warn!("Failed to serialize report notification data: {error}");
            return;
        }
    };
    for user_id in recipients {
        if let Err(error) = service
            .create_notification(
                user_id,
                NotificationType::BugReportSubmitted,
                title.clone(),
                Some(body.clone()),
                payload.clone(),
            )
            .await
        {
            warn!(
                "Failed to notify user {user_id} about report {}: {error}",
                data.report_id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(kind: &str, title: Option<&str>) -> BugReportSubmittedData {
        BugReportSubmittedData {
            report_id: "r1".into(),
            kind: kind.into(),
            category: "playback".into(),
            title: title.map(Into::into),
            reporter_handle: "alice".into(),
        }
    }

    #[test]
    fn text_distinguishes_bugs_and_feedback() {
        let (title, body) = notification_text(&data("bug", Some("Skips tracks")));
        assert_eq!(title, "New bug report");
        assert_eq!(body, "alice · playback: Skips tracks");

        let (title, body) = notification_text(&data("feature", Some("  ")));
        assert_eq!(title, "New feedback");
        assert_eq!(body, "alice · playback: (untitled)");
    }
}
