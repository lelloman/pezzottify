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
    ServerAdmin,
    ViewAnalytics,
    RequestContent,
    DownloadManagerAdmin;

    val displayName: String
        get() = when (this) {
            AccessCatalog -> "Access Catalog"
            LikeContent -> "Like Content"
            OwnPlaylists -> "Own Playlists"
            EditCatalog -> "Edit Catalog"
            ManagePermissions -> "Manage Permissions"
            IssueContentDownload -> "Download Content"
            ServerAdmin -> "Server Admin"
            ViewAnalytics -> "View Analytics"
            RequestContent -> "Request Content"
            DownloadManagerAdmin -> "Download Manager Admin"
        }

    val description: String
        get() = when (this) {
            AccessCatalog -> "Browse and play music from the catalog."
            LikeContent -> "Like albums, artists, and tracks."
            OwnPlaylists -> "Create and manage personal playlists."
            EditCatalog -> "Add, edit, or remove catalog content."
            ManagePermissions -> "Grant or revoke permissions for other users."
            IssueContentDownload -> "Request downloads of missing content."
            ServerAdmin -> "Server administration (reboot, etc.)."
            ViewAnalytics -> "View listening statistics and analytics."
            RequestContent -> "Search external music provider and request content downloads."
            DownloadManagerAdmin -> "Manage download queue, view audit logs, and retry failed downloads."
        }
}
