package com.lelloman.pezzottify.android.localdata

import javax.inject.Qualifier

/**
 * Whether the user may change the server URL. False when the build pins `server.baseUrl`:
 * the [DefaultHostUrl] is then always used and saved hosts are ignored.
 */
@Qualifier
annotation class ServerConfigurable
