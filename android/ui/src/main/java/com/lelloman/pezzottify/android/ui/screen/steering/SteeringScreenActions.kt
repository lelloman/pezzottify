package com.lelloman.pezzottify.android.ui.screen.steering

interface SteeringScreenActions {
    fun setSmartContinuationEnabled(enabled: Boolean)
    fun setStepsRemaining(remaining: Int)
    fun clearDestination()
    fun setDestinationComponentWeight(reference: SteeringReference, weight: Float)
    fun removeDestinationComponent(reference: SteeringReference)
    fun resetSourceToQueue()
    fun removeSourceReference(reference: SteeringReference)
    fun setRecencyWeight(value: Float)
    fun setDiversity(value: Float)
    fun setRandomness(value: Float)
    fun setMode(mode: String)
    fun setCriterionWeight(namespace: String, weight: Float)
    fun removeAway(reference: SteeringReference)
    fun resetKnobs()
    fun openSearch(target: SteeringSearchTarget)
    fun updateSearchQuery(query: String)
    fun pickSearchResult(reference: SteeringReference)
    fun closeSearch()
}
