/**
 * Google Gemini API adapter
 *
 * Uses the generateContent API with streaming and function calling.
 * https://ai.google.dev/gemini-api/docs/function-calling
 */

const API_BASE = 'https://generativelanguage.googleapis.com/v1beta/models';

export const MODELS = [
  { id: 'gemini-2.0-flash', name: 'Gemini 2.0 Flash' },
  { id: 'gemini-1.5-pro', name: 'Gemini 1.5 Pro' },
  { id: 'gemini-1.5-flash', name: 'Gemini 1.5 Flash' },
];

/**
 * Convert our unified message format to Gemini format
 */
function toGeminiContents(messages) {
  const contents = [];

  for (const msg of messages) {
    if (msg.role === 'user') {
      contents.push({
        role: 'user',
        parts: [{ text: msg.content }],
      });
    } else if (msg.role === 'assistant') {
      const parts = [];
      if (msg.content) {
        parts.push({ text: msg.content });
      }
      if (msg.toolCalls && msg.toolCalls.length > 0) {
        for (const tc of msg.toolCalls) {
          parts.push({
            functionCall: {
              name: tc.name,
              args: tc.input,
            },
          });
        }
      }
      contents.push({ role: 'model', parts });
    } else if (msg.role === 'tool') {
      contents.push({
        role: 'user',
        parts: [{
          functionResponse: {
            name: msg.toolName,
            response: typeof msg.content === 'string' ? { result: msg.content } : msg.content,
          },
        }],
      });
    }
  }

  return contents;
}

/**
 * Convert our unified tool format to Gemini format
 */
function toGeminiTools(tools) {
  if (tools.length === 0) return undefined;

  return [{
    functionDeclarations: tools.map(tool => ({
      name: tool.name,
      description: tool.description,
      parameters: tool.inputSchema,
    })),
  }];
}

/**
 * Stream chat completion from Gemini API
 */
export async function* streamChat(config, messages, tools = []) {
  const { apiKey, model = 'gemini-2.0-flash' } = config;

  if (!apiKey) {
    throw new Error('Google API key is required');
  }

  const apiUrl = `${API_BASE}/${model}:streamGenerateContent?key=${apiKey}&alt=sse`;

  const body = {
    contents: toGeminiContents(messages),
  };

  const geminiTools = toGeminiTools(tools);
  if (geminiTools) {
    body.tools = geminiTools;
  }

  const response = await fetch(apiUrl, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({}));
    throw new Error(error.error?.message || `Gemini API error: ${response.status}`);
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';

    for (const line of lines) {
      if (!line.startsWith('data: ')) continue;

      const data = line.slice(6);

      try {
        const event = JSON.parse(data);
        const candidates = event.candidates;

        if (!candidates || candidates.length === 0) continue;

        const parts = candidates[0].content?.parts || [];

        for (const part of parts) {
          if (part.text) {
            yield { type: 'text', content: part.text };
          }

          if (part.functionCall) {
            yield {
              type: 'tool_use',
              id: `gemini_${Date.now()}_${Math.random().toString(36).slice(2)}`,
              name: part.functionCall.name,
              input: part.functionCall.args || {},
            };
          }
        }
      } catch (e) {
        console.error('Failed to parse SSE event:', data, e);
      }
    }
  }
}

/**
 * Test connection to Gemini API
 */
export async function testConnection(config) {
  const { apiKey, model = 'gemini-2.0-flash' } = config;

  const apiUrl = `${API_BASE}/${model}:generateContent?key=${apiKey}`;

  const response = await fetch(apiUrl, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      contents: [{ role: 'user', parts: [{ text: 'Hi' }] }],
    }),
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({}));
    throw new Error(error.error?.message || `Connection failed: ${response.status}`);
  }

  return true;
}
