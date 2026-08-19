#![allow(unused_imports)]

mod http_cache;
mod random_slowdown;
mod rate_limit;
mod requests_logging;

pub use http_cache::http_cache;
#[cfg(feature = "slowdown")]
pub use random_slowdown::slowdown_request;
pub use rate_limit::{
    extract_login_account_for_rate_limit, extract_user_id_for_rate_limit,
    AnalyticsDeviceKeyExtractor, IpKeyExtractor, LoginAccountKeyExtractor, UserOrIpKeyExtractor,
};
pub use rate_limit::{
    ANALYTICS_PER_DEVICE_PER_MINUTE, CONTENT_READ_PER_MINUTE, GLOBAL_PER_MINUTE, LOGIN_PER_HOUR,
    LOGIN_PER_MINUTE, LOGIN_SUSTAINED_REPLENISH_MILLIS, SEARCH_PER_MINUTE, STREAM_PER_MINUTE,
    WRITE_PER_MINUTE,
};
pub use requests_logging::{log_requests, RequestsLoggingLevel};
