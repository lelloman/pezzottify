use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    AccessCatalog,
    LikeContent,
    OwnPlaylists,
    EditCatalog,
    ManagePermissions,
    IssueContentDownload,
    RebootServer,
}

impl Permission {
    pub fn to_int(&self) -> i32 {
        match self {
            Permission::AccessCatalog => 1,
            Permission::LikeContent => 2,
            Permission::OwnPlaylists => 3,
            Permission::EditCatalog => 4,
            Permission::ManagePermissions => 5,
            Permission::IssueContentDownload => 6,
            Permission::RebootServer => 7,
        }
    }

    pub fn from_int(value: i32) -> Option<Self> {
        match value {
            1 => Some(Permission::AccessCatalog),
            2 => Some(Permission::LikeContent),
            3 => Some(Permission::OwnPlaylists),
            4 => Some(Permission::EditCatalog),
            5 => Some(Permission::ManagePermissions),
            6 => Some(Permission::IssueContentDownload),
            7 => Some(Permission::RebootServer),
            _ => None,
        }
    }
}

const ADMIN_PERMISSIONS: &'static [Permission] = &[
    Permission::AccessCatalog,
    Permission::LikeContent,
    Permission::OwnPlaylists,
    Permission::EditCatalog,
    Permission::ManagePermissions,
    Permission::IssueContentDownload,
    Permission::RebootServer,
];
const REGULAR_PERMISSIONS: &'static [Permission] = &[
    Permission::AccessCatalog,
    Permission::LikeContent,
    Permission::OwnPlaylists,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserRole {
    Admin,
    Regular,
}

impl UserRole {
    pub fn permissions(&self) -> &'static [Permission] {
        match self {
            UserRole::Admin => ADMIN_PERMISSIONS,
            UserRole::Regular => REGULAR_PERMISSIONS,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            UserRole::Admin => "Admin",
            UserRole::Regular => "Regular",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Admin" => Some(UserRole::Admin),
            "Regular" => Some(UserRole::Regular),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PermissionGrant {
    ByRole(UserRole),
    Extra {
        start_time: SystemTime,
        end_time: Option<SystemTime>,
        permission: Permission,
        countdown: Option<u64>,
    },
}
