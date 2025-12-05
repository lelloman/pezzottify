use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

// Validation constants
pub const DEVICE_UUID_MIN_LEN: usize = 8;
pub const DEVICE_UUID_MAX_LEN: usize = 64;
pub const DEVICE_NAME_MAX_LEN: usize = 100;
pub const OS_INFO_MAX_LEN: usize = 200;
