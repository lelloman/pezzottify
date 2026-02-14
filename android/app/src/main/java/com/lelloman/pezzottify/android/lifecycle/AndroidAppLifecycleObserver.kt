package com.lelloman.pezzottify.android.lifecycle

import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import com.lelloman.pezzottify.android.domain.lifecycle.AppLifecycleObserver
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

class AndroidAppLifecycleObserver : AppLifecycleObserver, DefaultLifecycleObserver {

    private val _isInForeground = MutableStateFlow(false)
    override val isInForeground: StateFlow<Boolean> = _isInForeground.asStateFlow()

    private val _isKeptAliveExternally = MutableStateFlow(false)
    override val isKeptAliveExternally: StateFlow<Boolean> = _isKeptAliveExternally.asStateFlow()

    fun setKeptAliveExternally(value: Boolean) {
        _isKeptAliveExternally.value = value
    }

    init {
        ProcessLifecycleOwner.get().lifecycle.addObserver(this)
    }

    override fun onStart(owner: LifecycleOwner) {
        _isInForeground.value = true
    }

    override fun onStop(owner: LifecycleOwner) {
        _isInForeground.value = false
    }
}
