export const MAX_TOOL_ROUNDS = 10;

// One owner for streaming, language detection and compaction. Invalidated work
// must check this token after every await and before executing side effects.
export class ChatLifecycle {
  current = null;
  begin() {
    this.cancel();
    this.current = new AbortController();
    return this.current.signal;
  }
  cancel() {
    this.current?.abort();
    this.current = null;
  }
  check(signal) {
    signal.throwIfAborted();
    if (this.current?.signal !== signal) throw new DOMException('Cancelled', 'AbortError');
  }
  owns(signal) {
    return this.current?.signal === signal && !signal.aborted;
  }
}

export function requiresConfirmation(name) {
  return ['ui.deletePlaylist', 'catalog.mutate', 'users.mutate', 'jobs.action'].includes(name);
}

// Always pair every assistant tool call with a result, including budget refusals.
export async function runToolLoop({ stream, execute, append, onText, check, maxRounds = MAX_TOOL_ROUNDS }) {
  for (let round = 0; ; round++) {
    check();
    let content = '';
    const calls = [];
    for await (const event of stream()) {
      check();
      if (event.type === 'text') {
        content += event.content;
        onText(content);
      } else if (event.type === 'tool_use') {
        calls.push({ id: event.id, name: event.name, input: event.input });
      } else if (event.type === 'error') throw new Error(event.message);
    }
    check();
    append({ role: 'assistant', content, toolCalls: calls.length ? calls : undefined });
    onText('');
    if (!calls.length) return;
    for (const call of calls) {
      check();
      let result;
      try {
        result = round >= maxRounds
          ? { error: 'Tool iteration limit reached; action not executed.' }
          : await execute(call.name, call.input);
      } catch (error) {
        check();
        result = { error: error.message };
      }
      check();
      append({ role: 'tool', toolCallId: call.id, toolName: call.name,
        content: typeof result === 'string' ? result : JSON.stringify(result) });
    }
    if (round >= maxRounds) {
      append({ role: 'assistant', content: 'Stopped after reaching the tool limit. Please send another message to continue.' });
      return;
    }
  }
}
