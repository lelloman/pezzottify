/**
 * Ollama API adapter
 *
 * Ollama provides an OpenAI-compatible API at /v1/chat/completions
 * https://ollama.com/blog/openai-compatibility
 */

export const MODELS = [
  { id: 'llama3.3', name: 'Llama 3.3' },
  { id: 'llama3.2', name: 'Llama 3.2' },
  { id: 'qwen2.5', name: 'Qwen 2.5' },
  { id: 'mistral', name: 'Mistral' },
  { id: 'mixtral', name: 'Mixtral' },
  { id: 'codellama', name: 'Code Llama' },
];

/**
 * Convert our unified message format to OpenAI format (Ollama uses OpenAI format)
 */
function toOllamaMessages(messages) {
  return messages.map(msg => {
    if (msg.role === 'user') {
      return { role: 'user', content: msg.content };
    }

    if (msg.role === 'assistant') {
      const result = { role: 'assistant', content: msg.content || null };
      if (msg.toolCalls && msg.toolCalls.length > 0) {
        result.tool_calls = msg.toolCalls.map(tc => ({
          id: tc.id,
          type: 'function',
          function: {
            name: tc.name,
            arguments: JSON.stringify(tc.input),
          },
        }));
      }
      return result;
    }

    if (msg.role === 'tool') {
      return {
        role: 'tool',
        tool_call_id: msg.toolCallId,
        content: typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content),
      };
    }

    return msg;
  });
}

/**
 * Convert our unified tool format to OpenAI format
 */
function toOllamaTools(tools) {
  return tools.map(tool => ({
    type: 'function',
    function: {
      name: tool.name,
      description: tool.description,
      parameters: tool.inputSchema,
    },
  }));
}

/**
 * Stream chat completion from Ollama API
 */
export async function* streamChat(config, messages, tools = []) {
  const { baseUrl = 'http://localhost:11434', model = 'llama3.3' } = config;

  const apiUrl = `${baseUrl.replace(/\/$/, '')}/v1/chat/completions`;

  const body = {
    model,
    messages: toOllamaMessages(messages),
    stream: true,
  };

  if (tools.length > 0) {
    body.tools = toOllamaTools(tools);
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
    throw new Error(error.error?.message || `Ollama API error: ${response.status}`);
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  const toolCalls = {};

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';

    for (const line of lines) {
      if (!line.startsWith('data: ')) continue;

      const data = line.slice(6);
      if (data === '[DONE]') continue;

      try {
        const event = JSON.parse(data);
        const delta = event.choices?.[0]?.delta;

        if (!delta) continue;

        if (delta.content) {
          yield { type: 'text', content: delta.content };
        }

        if (delta.tool_calls) {
          for (const tc of delta.tool_calls) {
            const index = tc.index;
            if (!toolCalls[index]) {
              toolCalls[index] = { id: '', name: '', arguments: '' };
            }
            if (tc.id) toolCalls[index].id = tc.id;
            if (tc.function?.name) toolCalls[index].name = tc.function.name;
            if (tc.function?.arguments) toolCalls[index].arguments += tc.function.arguments;
          }
        }

        const finishReason = event.choices?.[0]?.finish_reason;
        if (finishReason === 'tool_calls') {
          for (const tc of Object.values(toolCalls)) {
            try {
              const input = tc.arguments ? JSON.parse(tc.arguments) : {};
              yield { type: 'tool_use', id: tc.id, name: tc.name, input };
            } catch {
              console.error('Failed to parse tool arguments:', tc.arguments);
            }
          }
        }
      } catch (e) {
        console.error('Failed to parse SSE event:', data, e);
      }
    }
  }
}

/**
 * Fetch available models from Ollama
 */
export async function fetchModels(config) {
  const { baseUrl = 'http://localhost:11434' } = config;

  try {
    const response = await fetch(`${baseUrl.replace(/\/$/, '')}/api/tags`);
    if (!response.ok) {
      throw new Error(`Failed to fetch models: ${response.status}`);
    }

    const data = await response.json();
    return data.models?.map(m => ({
      id: m.name,
      name: m.name,
    })) || MODELS;
  } catch (e) {
    console.warn('Failed to fetch Ollama models:', e);
    return MODELS;
  }
}

/**
 * Test connection to Ollama API
 */
export async function testConnection(config) {
  const { baseUrl = 'http://localhost:11434', model = 'llama3.3' } = config;

  // First check if Ollama is running
  const healthResponse = await fetch(`${baseUrl.replace(/\/$/, '')}/api/tags`);
  if (!healthResponse.ok) {
    throw new Error(`Cannot connect to Ollama at ${baseUrl}`);
  }

  // Then try a simple chat completion
  const apiUrl = `${baseUrl.replace(/\/$/, '')}/v1/chat/completions`;
  const response = await fetch(apiUrl, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      model,
      max_tokens: 10,
      messages: [{ role: 'user', content: 'Hi' }],
    }),
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({}));
    throw new Error(error.error?.message || `Connection failed: ${response.status}`);
  }

  return true;
}
