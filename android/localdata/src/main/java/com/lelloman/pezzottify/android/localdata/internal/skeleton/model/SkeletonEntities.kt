package com.lelloman.pezzottify.android.localdata.internal.skeleton.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * Skeleton artist entity - stores just the artist ID.
 */
@Entity(tableName = SkeletonArtist.TABLE_NAME)
internal data class SkeletonArtist(
    @PrimaryKey
    @ColumnInfo(name = COLUMN_ID)
    val id: String
) {
    companion object {
        const val TABLE_NAME = "skeleton_artists"
        const val COLUMN_ID = "id"
    }
}

/**
 * Skeleton album entity - stores just the album ID.
 */
@Entity(tableName = SkeletonAlbum.TABLE_NAME)
internal data class SkeletonAlbum(
    @PrimaryKey
    @ColumnInfo(name = COLUMN_ID)
    val id: String
) {
    companion object {
        const val TABLE_NAME = "skeleton_albums"
        const val COLUMN_ID = "id"
    }
}

/**
 * Junction table for album-artist relationships.
 */
@Entity(
    tableName = SkeletonAlbumArtist.TABLE_NAME,
    primaryKeys = [SkeletonAlbumArtist.COLUMN_ALBUM_ID, SkeletonAlbumArtist.COLUMN_ARTIST_ID],
    foreignKeys = [
        ForeignKey(
            entity = SkeletonAlbum::class,
            parentColumns = [SkeletonAlbum.COLUMN_ID],
            childColumns = [SkeletonAlbumArtist.COLUMN_ALBUM_ID],
            onDelete = ForeignKey.CASCADE
        ),
        ForeignKey(
            entity = SkeletonArtist::class,
            parentColumns = [SkeletonArtist.COLUMN_ID],
            childColumns = [SkeletonAlbumArtist.COLUMN_ARTIST_ID],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index(SkeletonAlbumArtist.COLUMN_ARTIST_ID)]
)
internal data class SkeletonAlbumArtist(
    @ColumnInfo(name = COLUMN_ALBUM_ID)
    val albumId: String,
    @ColumnInfo(name = COLUMN_ARTIST_ID)
    val artistId: String
) {
    companion object {
        const val TABLE_NAME = "skeleton_album_artists"
        const val COLUMN_ALBUM_ID = "album_id"
        const val COLUMN_ARTIST_ID = "artist_id"
    }
}

/**
 * Skeleton track entity - stores track ID and its album relationship.
 */
@Entity(
    tableName = SkeletonTrack.TABLE_NAME,
    foreignKeys = [
        ForeignKey(
            entity = SkeletonAlbum::class,
            parentColumns = [SkeletonAlbum.COLUMN_ID],
            childColumns = [SkeletonTrack.COLUMN_ALBUM_ID],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index(SkeletonTrack.COLUMN_ALBUM_ID)]
)
internal data class SkeletonTrack(
    @PrimaryKey
    @ColumnInfo(name = COLUMN_ID)
    val id: String,
    @ColumnInfo(name = COLUMN_ALBUM_ID)
    val albumId: String
) {
    companion object {
        const val TABLE_NAME = "skeleton_tracks"
        const val COLUMN_ID = "id"
        const val COLUMN_ALBUM_ID = "album_id"
    }
}

/**
 * Skeleton metadata storage (version, checksum).
 */
@Entity(tableName = SkeletonMeta.TABLE_NAME)
internal data class SkeletonMeta(
    @PrimaryKey
    @ColumnInfo(name = COLUMN_KEY)
    val key: String,
    @ColumnInfo(name = COLUMN_VALUE)
    val value: String
) {
    companion object {
        const val TABLE_NAME = "skeleton_meta"
        const val COLUMN_KEY = "key"
        const val COLUMN_VALUE = "value"

        const val KEY_VERSION = "version"
        const val KEY_CHECKSUM = "checksum"
    }
}
