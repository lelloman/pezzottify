package com.lelloman.pezzottify.android.ui.model

/**
 * UI representation of user permissions.
 */
enum class Permission {
    AccessCatalog,
    LikeContent,
    OwnPlaylists,
    EditCatalog,
    ManagePermissions,
    IssueContentDownload,
    RebootServer,
    ViewAnalytics;

    val displayName: String
        get() = when (this) {
            AccessCatalog -> "Access Catalog"
            LikeContent -> "Like Content"
            OwnPlaylists -> "Own Playlists"
            EditCatalog -> "Edit Catalog"
            ManagePermissions -> "Manage Permissions"
            IssueContentDownload -> "Download Content"
            RebootServer -> "Reboot Server"
            ViewAnalytics -> "View Analytics"
        }

    val description: String
        get() = when (this) {
            AccessCatalog -> "Browse and play music from the catalog."
            LikeContent -> "Like albums, artists, and tracks."
            OwnPlaylists -> "Create and manage personal playlists."
            EditCatalog -> "Add, edit, or remove catalog content."
            ManagePermissions -> "Grant or revoke permissions for other users."
            IssueContentDownload -> "Request downloads of missing content."
            RebootServer -> "Restart the server remotely."
            ViewAnalytics -> "View listening statistics and analytics."
        }
}
