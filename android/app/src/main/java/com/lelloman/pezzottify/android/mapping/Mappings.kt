package com.lelloman.pezzottify.android.mapping

import com.github.lelloman.duckmapper.DuckMap
import com.github.lelloman.duckmapper.DuckImplement
import com.lelloman.pezzottify.android.domain.settings.ThemeMode as DomainThemeMode
import com.lelloman.pezzottify.android.domain.settings.ColorPalette as DomainColorPalette
import com.lelloman.pezzottify.android.domain.settings.AppFontFamily as DomainAppFontFamily
import com.lelloman.pezzottify.android.domain.storage.StoragePressureLevel as DomainStoragePressureLevel
import com.lelloman.pezzottify.android.domain.storage.StorageInfo as DomainStorageInfo
import com.lelloman.pezzottify.android.domain.statics.TrackAvailability as DomainTrackAvailability
import com.lelloman.pezzottify.android.domain.statics.AlbumAvailability as DomainAlbumAvailability
import com.lelloman.pezzottify.android.domain.sync.Permission as DomainPermission
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent.ContentType as DomainLikedContentType
import com.lelloman.pezzottify.android.ui.theme.ThemeMode as UiThemeMode
import com.lelloman.pezzottify.android.ui.theme.ColorPalette as UiColorPalette
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily as UiAppFontFamily
import com.lelloman.pezzottify.android.ui.model.StoragePressureLevel as UiStoragePressureLevel
import com.lelloman.pezzottify.android.ui.model.StorageInfo as UiStorageInfo
import com.lelloman.pezzottify.android.ui.content.TrackAvailability as UiTrackAvailability
import com.lelloman.pezzottify.android.ui.content.AlbumAvailability as UiAlbumAvailability
import com.lelloman.pezzottify.android.ui.model.Permission as UiPermission
import com.lelloman.pezzottify.android.ui.model.LikedContent.ContentType as UiLikedContentType
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState as DomainConnectionState
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext as DomainPlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist as DomainPlaybackPlaylist
import com.lelloman.pezzottify.android.ui.component.ConnectionState as UiConnectionState
import com.lelloman.pezzottify.android.ui.model.PlaybackPlaylistContext as UiPlaybackPlaylistContext
import com.lelloman.pezzottify.android.ui.model.PlaybackPlaylist as UiPlaybackPlaylist
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent as DomainLikedContent
import com.lelloman.pezzottify.android.ui.model.LikedContent as UiLikedContent

// Enums
@DuckMap(DomainThemeMode::class, UiThemeMode::class)
@DuckMap(DomainColorPalette::class, UiColorPalette::class)
@DuckMap(DomainAppFontFamily::class, UiAppFontFamily::class)
@DuckMap(DomainStoragePressureLevel::class, UiStoragePressureLevel::class)
@DuckMap(DomainTrackAvailability::class, UiTrackAvailability::class)
@DuckMap(DomainAlbumAvailability::class, UiAlbumAvailability::class)
@DuckMap(DomainPermission::class, UiPermission::class)
@DuckMap(DomainLikedContentType::class, UiLikedContentType::class)

// Data classes
@DuckMap(DomainStorageInfo::class, UiStorageInfo::class)

// Sealed interfaces
@DuckMap(DomainConnectionState::class, UiConnectionState::class)
@DuckMap(DomainPlaybackPlaylistContext::class, UiPlaybackPlaylistContext::class)
@DuckMap(DomainPlaybackPlaylist::class, UiPlaybackPlaylist::class)

object Mappings
