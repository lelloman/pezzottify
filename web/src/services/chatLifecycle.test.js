import test from 'node:test';
import assert from 'node:assert/strict';
import { ChatLifecycle, runToolLoop, requiresConfirmation } from './chatLifecycle.js';

test('clearing invalidates old work without invalidating a new turn', () => {
  const lifecycle = new ChatLifecycle();
  const old = lifecycle.begin();
  const current = lifecycle.begin();
  assert.equal(old.aborted, true);
  assert.throws(() => lifecycle.check(old), { name: 'AbortError' });
  assert.doesNotThrow(() => lifecycle.check(current));
});

test('tool budgets count rounds, not messages, and all calls receive results', async () => {
  const messages = Array.from({ length: 60 }, () => ({ role: 'user', content: 'old' }));
  let executed = 0;
  let rounds = 0;
  await runToolLoop({
    stream: async function* () { yield { type: 'tool_use', id: `call-${++rounds}`, name: 'read', input: {} }; },
    execute: async () => { executed++; return 'ok'; },
    append: message => messages.push(message), onText() {}, check() {}, maxRounds: 2,
  });
  assert.equal(executed, 2);
  assert.equal(rounds, 3);
  assert.equal(messages.filter(m => m.role === 'tool').length, 3);
  assert.match(messages.at(-2).content, /not executed/);
});

test('cancelled stream cannot execute tools or append stale output', async () => {
  const lifecycle = new ChatLifecycle();
  const signal = lifecycle.begin();
  let executed = false;
  const messages = [];
  await assert.rejects(runToolLoop({
    stream: async function* () {
      lifecycle.cancel();
      yield { type: 'tool_use', id: 'old', name: 'delete', input: {} };
    },
    execute: async () => { executed = true; }, append: m => messages.push(m),
    onText() {}, check: () => lifecycle.check(signal),
  }), { name: 'AbortError' });
  assert.equal(executed, false);
  assert.deepEqual(messages, []);
});

test('tool failure is paired with a result before continuing', async () => {
  let round = 0;
  const messages = [];
  await runToolLoop({
    stream: async function* () {
      if (round++ === 0) yield { type: 'tool_use', id: 'one', name: 'read', input: {} };
      else yield { type: 'text', content: 'Failed safely' };
    },
    execute: async () => { throw new Error('unavailable'); }, append: m => messages.push(m),
    onText() {}, check() {},
  });
  assert.equal(messages[1].toolCallId, 'one');
  assert.match(messages[1].content, /unavailable/);
});

test('destructive and administrative tools require explicit confirmation', () => {
  for (const name of ['ui.deletePlaylist', 'catalog.mutate', 'users.mutate', 'jobs.action']) {
    assert.equal(requiresConfirmation(name), true);
  }
  for (const name of ['ui.play', 'catalog.search', 'ui.getPlaylists']) {
    assert.equal(requiresConfirmation(name), false);
  }
});
