package com.lelloman.simpleai;

interface ISimpleAI {

    /**
     * Get service version and capabilities status.
     */
    String getServiceInfo(int protocolVersion);

    /**
     * Classify text using NLU with a specific adapter.
     */
    String classify(
        int protocolVersion,
        String text,
        String adapterId,
        String adapterVersion,
        in ParcelFileDescriptor patchFd,
        in ParcelFileDescriptor headsFd,
        in ParcelFileDescriptor tokenizerFd,
        in ParcelFileDescriptor configFd
    );

    /**
     * Remove currently applied adapter and restore pristine model.
     */
    String clearAdapter(int protocolVersion);

    /**
     * Translate text between languages.
     */
    String translate(
        int protocolVersion,
        String text,
        String sourceLang,
        String targetLang
    );

    /**
     * Get list of downloaded translation languages.
     */
    String getTranslationLanguages(int protocolVersion);

    /**
     * Send chat request to cloud LLM endpoint.
     */
    String cloudChat(
        int protocolVersion,
        String messagesJson,
        String toolsJson,
        String systemPrompt,
        String authToken
    );

    /**
     * Generate text using local LLM.
     */
    String localGenerate(
        int protocolVersion,
        String prompt,
        int maxTokens,
        float temperature
    );

    /**
     * Chat using local LLM with tool support.
     */
    String localChat(
        int protocolVersion,
        String messagesJson,
        String toolsJson,
        String systemPrompt
    );
}
