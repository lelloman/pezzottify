use std::time::SystemTime;

pub enum Permission {
    AccessCatalog,
    LikeContent,
    OwnPlaylists,
    EditCatalog,
    TransferPermission,
    IssueContentDownload,
    RebootServer,
}
const ADMIN_PERMISSIONS: &'static [Permission] = &[
    Permission::EditCatalog,
    Permission::TransferPermission,
    Permission::IssueContentDownload,
    Permission::RebootServer,
];
const REGULAR_PERMISSIONS: &'static [Permission] = &[
    Permission::AccessCatalog,
    Permission::LikeContent,
    Permission::OwnPlaylists,
];

pub enum UserRole {
    Admin,
    Regular,
}

impl UserRole {
    fn permissions(&self) -> &'static [Permission] {
        match self {
            UserRole::Admin => ADMIN_PERMISSIONS,
            UserRole::Regular => REGULAR_PERMISSIONS,
        }
    }
}

pub enum PermissionGrant {
    ByRole(UserRole),
    Extra(Permission),
    OneOff(Permission),
    Timed {
        start_time: SystemTime,
        end_time: SystemTime,
        permissions: Vec<Permission>,
    },
}
