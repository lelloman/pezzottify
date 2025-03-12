package com.lelloman.pezzottify.android.domain.user

import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import javax.inject.Inject

class LogViewedContentUseCase @Inject constructor(
    private val userDataStore: UserDataStore,
    private val timeProvider: TimeProvider,
) : UseCase() {

    operator fun invoke(contentId: String, type: ViewedContent.Type) {
        GlobalScope.launch(Dispatchers.IO) {
            userDataStore.addNewViewedContent(object : ViewedContent {
                override val type: ViewedContent.Type = type
                override val contentId: String = contentId
                override val created: Long = timeProvider.nowUtcMs()
                override val synced: Boolean = false
            })
        }
    }
}