package com.lelloman.pezzottify.android.app.localdata

import com.lelloman.pezzottify.android.app.domain.LogoutOperation
import com.lelloman.pezzottify.android.localdata.LocalDb

class DeleteStaticsLogoutOperation(
    private val localDb: LocalDb,
) : LogoutOperation {
    override suspend fun invoke() {
        localDb.clearAllTables()
    }
}