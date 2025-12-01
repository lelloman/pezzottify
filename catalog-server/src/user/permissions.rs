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
    ViewAnalytics,
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
            Permission::ViewAnalytics => 8,
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
            8 => Some(Permission::ViewAnalytics),
            _ => None,
        }
    }
}

const ADMIN_PERMISSIONS: &'static [Permission] = &[
    Permission::AccessCatalog,
    Permission::EditCatalog,
    Permission::ManagePermissions,
    Permission::IssueContentDownload,
    Permission::RebootServer,
    Permission::ViewAnalytics,
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
        assert_eq!(Permission::AccessCatalog.to_int(), 1);
        assert_eq!(Permission::LikeContent.to_int(), 2);
        assert_eq!(Permission::OwnPlaylists.to_int(), 3);
        assert_eq!(Permission::EditCatalog.to_int(), 4);
        assert_eq!(Permission::ManagePermissions.to_int(), 5);
        assert_eq!(Permission::IssueContentDownload.to_int(), 6);
        assert_eq!(Permission::RebootServer.to_int(), 7);
        assert_eq!(Permission::ViewAnalytics.to_int(), 8);
    }

    #[test]
    fn permission_from_int_valid_values() {
        assert_eq!(Permission::from_int(1), Some(Permission::AccessCatalog));
        assert_eq!(Permission::from_int(2), Some(Permission::LikeContent));
        assert_eq!(Permission::from_int(3), Some(Permission::OwnPlaylists));
        assert_eq!(Permission::from_int(4), Some(Permission::EditCatalog));
        assert_eq!(Permission::from_int(5), Some(Permission::ManagePermissions));
        assert_eq!(Permission::from_int(6), Some(Permission::IssueContentDownload));
        assert_eq!(Permission::from_int(7), Some(Permission::RebootServer));
        assert_eq!(Permission::from_int(8), Some(Permission::ViewAnalytics));
    }

    #[test]
    fn permission_from_int_invalid_values() {
        assert_eq!(Permission::from_int(0), None);
        assert_eq!(Permission::from_int(9), None);
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
            Permission::IssueContentDownload,
            Permission::RebootServer,
            Permission::ViewAnalytics,
        ];

        for permission in &permissions {
            let int_val = permission.to_int();
            let recovered = Permission::from_int(int_val);
            assert_eq!(recovered, Some(*permission));
        }
    }

    #[test]
    fn user_role_admin_permissions() {
        let admin_perms = UserRole::Admin.permissions();

        assert_eq!(admin_perms.len(), 6);
        assert!(admin_perms.contains(&Permission::AccessCatalog));
        assert!(admin_perms.contains(&Permission::EditCatalog));
        assert!(admin_perms.contains(&Permission::ManagePermissions));
        assert!(admin_perms.contains(&Permission::IssueContentDownload));
        assert!(admin_perms.contains(&Permission::RebootServer));
        assert!(admin_perms.contains(&Permission::ViewAnalytics));

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
        assert!(!regular_perms.contains(&Permission::IssueContentDownload));
        assert!(!regular_perms.contains(&Permission::RebootServer));
        assert!(!regular_perms.contains(&Permission::ViewAnalytics));
    }

    #[test]
    fn user_role_to_string() {
        assert_eq!(UserRole::Admin.to_string(), "Admin");
        assert_eq!(UserRole::Regular.to_string(), "Regular");
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
        assert_eq!(UserRole::from_str(admin.to_string()), Some(admin));

        let regular = UserRole::Regular;
        assert_eq!(UserRole::from_str(regular.to_string()), Some(regular));
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
            permission: Permission::RebootServer,
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
                assert_eq!(permission, Permission::RebootServer);
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
            permission: Permission::IssueContentDownload,
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
                assert_eq!(permission, Permission::IssueContentDownload);
                assert_eq!(countdown, Some(10));
            }
            _ => panic!("Expected Extra variant"),
        }
    }
}
