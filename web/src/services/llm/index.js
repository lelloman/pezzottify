/**
 * Unified LLM Provider Service
 *
 * Provides a consistent interface for different LLM providers.
 */

import * as anthropic from './anthropic.js';
import * as openai from './openai.js';
import * as ollama from './ollama.js';
import * as google from './google.js';
import * as openrouter from './openrouter.js';

export const PROVIDERS = {
  anthropic: {
    name: 'Anthropic',
    models: anthropic.MODELS,
    requiresApiKey: true,
    adapter: anthropic,
  },
  openai: {
    name: 'OpenAI',
    models: openai.MODELS,
    requiresApiKey: true,
    adapter: openai,
  },
  ollama: {
    name: 'Ollama',
    models: ollama.MODELS,
    requiresApiKey: false,
    requiresBaseUrl: true,
    defaultBaseUrl: 'http://localhost:11434',
    adapter: ollama,
  },
  google: {
    name: 'Google',
    models: google.MODELS,
    requiresApiKey: true,
    adapter: google,
  },
  openrouter: {
    name: 'OpenRouter',
    models: openrouter.MODELS,
    requiresApiKey: true,
    adapter: openrouter,
  },
};

/**
 * Get provider info
 */
export function getProvider(providerId) {
  return PROVIDERS[providerId];
}

/**
 * Get all provider IDs
 */
export function getProviderIds() {
  return Object.keys(PROVIDERS);
}

/**
 * Stream chat completion from the configured provider
 *
 * @param {string} providerId - Provider ID (anthropic, openai, etc.)
 * @param {Object} config - Provider config { apiKey, model, baseUrl?, ... }
 * @param {Array} messages - Message history in unified format
 * @param {Array} tools - Available tools in unified format
 * @yields {{ type: 'text', content: string } | { type: 'tool_use', id: string, name: string, input: object } | { type: 'error', message: string }}
 */
export async function* streamChat(providerId, config, messages, tools = []) {
  const provider = PROVIDERS[providerId];
  if (!provider) {
    yield { type: 'error', message: `Unknown provider: ${providerId}` };
    return;
  }

  try {
    yield* provider.adapter.streamChat(config, messages, tools);
  } catch (error) {
    yield { type: 'error', message: error.message };
  }
}

/**
 * Test connection to the provider
 */
export async function testConnection(providerId, config) {
  const provider = PROVIDERS[providerId];
  if (!provider) {
    throw new Error(`Unknown provider: ${providerId}`);
  }

  return provider.adapter.testConnection(config);
}

/**
 * Get models for a provider (some providers support dynamic model listing)
 */
export async function getModels(providerId, config) {
  const provider = PROVIDERS[providerId];
  if (!provider) {
    throw new Error(`Unknown provider: ${providerId}`);
  }

  // If provider has dynamic model fetching, use it
  if (provider.adapter.fetchModels) {
    try {
      return await provider.adapter.fetchModels(config);
    } catch (e) {
      console.warn(`Failed to fetch models for ${providerId}:`, e);
    }
  }

  // Fall back to static model list
  return provider.models;
}
