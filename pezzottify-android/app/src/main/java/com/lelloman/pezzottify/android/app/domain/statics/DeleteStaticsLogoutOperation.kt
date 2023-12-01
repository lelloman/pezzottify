package com.lelloman.pezzottify.android.app.domain.statics

import com.lelloman.pezzottify.android.app.domain.login.LogoutOperation
import javax.inject.Inject

class DeleteStaticsLogoutOperation @Inject constructor(private val staticsStore: StaticsStore) :
    LogoutOperation {
    override suspend fun invoke() = staticsStore.deleteStatics()
}