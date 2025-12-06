package com.lelloman.pezzottify.android.ui.model

data class StorageInfo(
    val totalBytes: Long,
    val availableBytes: Long,
    val usedBytes: Long,
    val pressureLevel: StoragePressureLevel
) {
    val usedPercentage: Double
        get() = if (totalBytes > 0) {
            usedBytes.toDouble() / totalBytes.toDouble()
        } else {
            0.0
        }

    val availablePercentage: Double
        get() = if (totalBytes > 0) {
            availableBytes.toDouble() / totalBytes.toDouble()
        } else {
            0.0
        }
}
