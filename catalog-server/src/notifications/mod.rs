//! User notifications module

mod models;
mod store;

pub use models::{DownloadCompletedData, Notification, NotificationType};
pub use store::NotificationStore;
