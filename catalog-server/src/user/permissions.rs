use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    AccessCatalog,
    LikeContent,
    OwnPlaylists,
    EditCatalog,
    ManagePermissions,
    ServerAdmin,
    ViewAnalytics,
    RequestContent,
    DownloadManagerAdmin,
}

impl Permission {
    pub fn as_int(self) -> i32 {
        match self {
            Permission::AccessCatalog => 1,
            Permission::LikeContent => 2,
            Permission::OwnPlaylists => 3,
            Permission::EditCatalog => 4,
            Permission::ManagePermissions => 5,
            Permission::ServerAdmin => 7,
            Permission::ViewAnalytics => 8,
            Permission::RequestContent => 9,
            Permission::DownloadManagerAdmin => 10,
        }
    }

    pub fn from_int(value: i32) -> Option<Self> {
        match value {
            1 => Some(Permission::AccessCatalog),
            2 => Some(Permission::LikeContent),
            3 => Some(Permission::OwnPlaylists),
            4 => Some(Permission::EditCatalog),
            5 => Some(Permission::ManagePermissions),
            7 => Some(Permission::ServerAdmin),
            8 => Some(Permission::ViewAnalytics),
            9 => Some(Permission::RequestContent),
            10 => Some(Permission::DownloadManagerAdmin),
            _ => None,
        }
    }
}

const ADMIN_PERMISSIONS: &[Permission] = &[
    Permission::AccessCatalog,
    Permission::EditCatalog,
    Permission::ManagePermissions,
    Permission::ServerAdmin,
    Permission::ViewAnalytics,
    Permission::RequestContent,
    Permission::DownloadManagerAdmin,
];
const REGULAR_PERMISSIONS: &[Permission] = &[
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

    pub fn as_str(self) -> &'static str {
        match self {
            UserRole::Admin => "Admin",
            UserRole::Regular => "Regular",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "admin" => Some(UserRole::Admin),
            "regular" => Some(UserRole::Regular),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_to_int_all_variants() {
        assert_eq!(Permission::AccessCatalog.as_int(), 1);
        assert_eq!(Permission::LikeContent.as_int(), 2);
        assert_eq!(Permission::OwnPlaylists.as_int(), 3);
        assert_eq!(Permission::EditCatalog.as_int(), 4);
        assert_eq!(Permission::ManagePermissions.as_int(), 5);
        assert_eq!(Permission::ServerAdmin.as_int(), 7);
        assert_eq!(Permission::ViewAnalytics.as_int(), 8);
        assert_eq!(Permission::RequestContent.as_int(), 9);
        assert_eq!(Permission::DownloadManagerAdmin.as_int(), 10);
    }

    #[test]
    fn permission_from_int_valid_values() {
        assert_eq!(Permission::from_int(1), Some(Permission::AccessCatalog));
        assert_eq!(Permission::from_int(2), Some(Permission::LikeContent));
        assert_eq!(Permission::from_int(3), Some(Permission::OwnPlaylists));
        assert_eq!(Permission::from_int(4), Some(Permission::EditCatalog));
        assert_eq!(Permission::from_int(5), Some(Permission::ManagePermissions));
        assert_eq!(Permission::from_int(6), None);
        assert_eq!(Permission::from_int(7), Some(Permission::ServerAdmin));
        assert_eq!(Permission::from_int(8), Some(Permission::ViewAnalytics));
        assert_eq!(Permission::from_int(9), Some(Permission::RequestContent));
        assert_eq!(
            Permission::from_int(10),
            Some(Permission::DownloadManagerAdmin)
        );
    }

    #[test]
    fn permission_from_int_invalid_values() {
        assert_eq!(Permission::from_int(0), None);
        assert_eq!(Permission::from_int(11), None);
        assert_eq!(Permission::from_int(-1), None);
        assert_eq!(Permission::from_int(100), None);
        assert_eq!(Permission::from_int(i32::MAX), None);
        assert_eq!(Permission::from_int(i32::MIN), None);
    }

    #[test]
    fn permission_roundtrip() {
        let permissions = [
            Permission::AccessCatalog,
            Permission::LikeContent,
            Permission::OwnPlaylists,
            Permission::EditCatalog,
            Permission::ManagePermissions,
            Permission::ServerAdmin,
            Permission::ViewAnalytics,
            Permission::RequestContent,
            Permission::DownloadManagerAdmin,
        ];

        for permission in &permissions {
            let int_val = permission.as_int();
            let recovered = Permission::from_int(int_val);
            assert_eq!(recovered, Some(*permission));
        }
    }

    #[test]
    fn user_role_admin_permissions() {
        let admin_perms = UserRole::Admin.permissions();

        assert_eq!(admin_perms.len(), 7);
        assert!(admin_perms.contains(&Permission::AccessCatalog));
        assert!(admin_perms.contains(&Permission::EditCatalog));
        assert!(admin_perms.contains(&Permission::ManagePermissions));
        assert!(admin_perms.contains(&Permission::ServerAdmin));
        assert!(admin_perms.contains(&Permission::ViewAnalytics));
        assert!(admin_perms.contains(&Permission::RequestContent));
        assert!(admin_perms.contains(&Permission::DownloadManagerAdmin));

        assert!(!admin_perms.contains(&Permission::LikeContent));
        assert!(!admin_perms.contains(&Permission::OwnPlaylists));
    }

    #[test]
    fn user_role_regular_permissions() {
        let regular_perms = UserRole::Regular.permissions();

        assert_eq!(regular_perms.len(), 3);
        assert!(regular_perms.contains(&Permission::AccessCatalog));
        assert!(regular_perms.contains(&Permission::LikeContent));
        assert!(regular_perms.contains(&Permission::OwnPlaylists));

        assert!(!regular_perms.contains(&Permission::EditCatalog));
        assert!(!regular_perms.contains(&Permission::ManagePermissions));
        assert!(!regular_perms.contains(&Permission::ServerAdmin));
        assert!(!regular_perms.contains(&Permission::ViewAnalytics));
        assert!(!regular_perms.contains(&Permission::RequestContent));
        assert!(!regular_perms.contains(&Permission::DownloadManagerAdmin));
    }

    #[test]
    fn user_role_as_str() {
        assert_eq!(UserRole::Admin.as_str(), "Admin");
        assert_eq!(UserRole::Regular.as_str(), "Regular");
    }

    #[test]
    fn user_role_from_str_valid() {
        assert_eq!(UserRole::from_str("Admin"), Some(UserRole::Admin));
        assert_eq!(UserRole::from_str("Regular"), Some(UserRole::Regular));
    }

    #[test]
    fn user_role_from_str_invalid() {
        // Note: from_str is case-insensitive, so "admin", "ADMIN" etc are valid
        assert_eq!(UserRole::from_str(""), None);
        assert_eq!(UserRole::from_str("User"), None);
        assert_eq!(UserRole::from_str("SuperAdmin"), None);
        assert_eq!(UserRole::from_str("moderator"), None);
        assert_eq!(UserRole::from_str("guest"), None);
    }

    #[test]
    fn user_role_from_str_case_insensitive() {
        // Verify case-insensitive parsing works
        assert_eq!(UserRole::from_str("admin"), Some(UserRole::Admin));
        assert_eq!(UserRole::from_str("Admin"), Some(UserRole::Admin));
        assert_eq!(UserRole::from_str("ADMIN"), Some(UserRole::Admin));
        assert_eq!(UserRole::from_str("regular"), Some(UserRole::Regular));
        assert_eq!(UserRole::from_str("Regular"), Some(UserRole::Regular));
        assert_eq!(UserRole::from_str("REGULAR"), Some(UserRole::Regular));
    }

    #[test]
    fn user_role_roundtrip() {
        let admin = UserRole::Admin;
        assert_eq!(UserRole::from_str(admin.as_str()), Some(admin));

        let regular = UserRole::Regular;
        assert_eq!(UserRole::from_str(regular.as_str()), Some(regular));
    }

    #[test]
    fn permission_grant_by_role() {
        let grant = PermissionGrant::ByRole(UserRole::Admin);

        match grant {
            PermissionGrant::ByRole(role) => {
                assert_eq!(role, UserRole::Admin);
            }
            _ => panic!("Expected ByRole variant"),
        }
    }

    #[test]
    fn permission_grant_extra_with_end_time() {
        let start = SystemTime::now();
        let end = start + std::time::Duration::from_secs(3600);

        let grant = PermissionGrant::Extra {
            start_time: start,
            end_time: Some(end),
            permission: Permission::EditCatalog,
            countdown: None,
        };

        match grant {
            PermissionGrant::Extra {
                start_time,
                end_time,
                permission,
                countdown,
            } => {
                assert_eq!(start_time, start);
                assert_eq!(end_time, Some(end));
                assert_eq!(permission, Permission::EditCatalog);
                assert_eq!(countdown, None);
            }
            _ => panic!("Expected Extra variant"),
        }
    }

    #[test]
    fn permission_grant_extra_with_countdown() {
        let start = SystemTime::now();

        let grant = PermissionGrant::Extra {
            start_time: start,
            end_time: None,
            permission: Permission::ServerAdmin,
            countdown: Some(5),
        };

        match grant {
            PermissionGrant::Extra {
                start_time,
                end_time,
                permission,
                countdown,
            } => {
                assert_eq!(start_time, start);
                assert_eq!(end_time, None);
                assert_eq!(permission, Permission::ServerAdmin);
                assert_eq!(countdown, Some(5));
            }
            _ => panic!("Expected Extra variant"),
        }
    }

    #[test]
    fn permission_grant_extra_with_both_time_and_countdown() {
        let start = SystemTime::now();
        let end = start + std::time::Duration::from_secs(7200);

        let grant = PermissionGrant::Extra {
            start_time: start,
            end_time: Some(end),
            permission: Permission::ViewAnalytics,
            countdown: Some(10),
        };

        match grant {
            PermissionGrant::Extra {
                start_time,
                end_time,
                permission,
                countdown,
            } => {
                assert_eq!(start_time, start);
                assert_eq!(end_time, Some(end));
                assert_eq!(permission, Permission::ViewAnalytics);
                assert_eq!(countdown, Some(10));
            }
            _ => panic!("Expected Extra variant"),
        }
    }
}
