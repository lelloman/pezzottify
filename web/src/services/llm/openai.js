/**
 * OpenAI API adapter
 *
 * Uses the Chat Completions API with streaming and function calling.
 * https://platform.openai.com/docs/api-reference/chat
 */

const API_URL = 'https://api.openai.com/v1/chat/completions';

export const MODELS = [
  { id: 'gpt-4o', name: 'GPT-4o' },
  { id: 'gpt-4o-mini', name: 'GPT-4o Mini' },
  { id: 'gpt-4-turbo', name: 'GPT-4 Turbo' },
  { id: 'gpt-3.5-turbo', name: 'GPT-3.5 Turbo' },
];

/**
 * Convert our unified message format to OpenAI format
 */
function toOpenAIMessages(messages) {
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
function toOpenAITools(tools) {
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
 * Stream chat completion from OpenAI API
 */
export async function* streamChat(config, messages, tools = []) {
  const { apiKey, model = 'gpt-4o' } = config;

  if (!apiKey) {
    throw new Error('OpenAI API key is required');
  }

  const body = {
    model,
    messages: toOpenAIMessages(messages),
    stream: true,
  };

  if (tools.length > 0) {
    body.tools = toOpenAITools(tools);
  }

  const response = await fetch(API_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${apiKey}`,
    },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({}));
    throw new Error(error.error?.message || `OpenAI API error: ${response.status}`);
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  // Track tool calls being built up across deltas
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

        // Text content
        if (delta.content) {
          yield { type: 'text', content: delta.content };
        }

        // Tool calls
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

        // Check for finish reason
        const finishReason = event.choices?.[0]?.finish_reason;
        if (finishReason === 'tool_calls') {
          // Emit all collected tool calls
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
 * Test connection to OpenAI API
 */
export async function testConnection(config) {
  const { apiKey, model = 'gpt-4o' } = config;

  const response = await fetch(API_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${apiKey}`,
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
