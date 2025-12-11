package com.lelloman.pezzottify.android.ui.model

import androidx.annotation.StringRes
import com.lelloman.pezzottify.android.ui.R

/**
 * UI representation of user permissions.
 */
enum class Permission(
    @StringRes val displayNameRes: Int,
    @StringRes val descriptionRes: Int
) {
    AccessCatalog(R.string.permission_access_catalog, R.string.permission_access_catalog_desc),
    LikeContent(R.string.permission_like_content, R.string.permission_like_content_desc),
    OwnPlaylists(R.string.permission_own_playlists, R.string.permission_own_playlists_desc),
    EditCatalog(R.string.permission_edit_catalog, R.string.permission_edit_catalog_desc),
    ManagePermissions(R.string.permission_manage_permissions, R.string.permission_manage_permissions_desc),
    IssueContentDownload(R.string.permission_issue_content_download, R.string.permission_issue_content_download_desc),
    ServerAdmin(R.string.permission_server_admin, R.string.permission_server_admin_desc),
    ViewAnalytics(R.string.permission_view_analytics, R.string.permission_view_analytics_desc),
    RequestContent(R.string.permission_request_content, R.string.permission_request_content_desc),
    DownloadManagerAdmin(R.string.permission_download_manager_admin, R.string.permission_download_manager_admin_desc);
}
