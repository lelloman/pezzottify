package com.lelloman.pezzottify.android.ui.screen.about

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow

class AboutViewModel : ViewModel() {

    private val mutableState = MutableStateFlow<AboutScreenState>(AboutScreenState())
}