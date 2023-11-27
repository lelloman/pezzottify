package com.lelloman.pezzottify.android.app.ui.dashboard.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.app.player.PlayerManager
import com.lelloman.pezzottify.android.localdata.StaticsDao
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class HomeViewModel @Inject constructor(
    private val staticsDao: StaticsDao,
    private val playerManager: PlayerManager,
) : ViewModel() {

    val items = staticsDao.getAlbums()
        .map { albums ->
            albums.map {
                ListItem(albumId = it.id, name = it.name)
            }
        }

    fun onItemClicked(item: ListItem) {
        viewModelScope.launch launcho@{
            val album = staticsDao.getAlbum(item.albumId) ?: return@launcho
            playerManager.play(album)
        }
    }

    data class ListItem(
        val albumId: String,
        val name: String,
    )
}