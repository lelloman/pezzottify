/** Pezzottify-owned prompts and tool groupings. The library has no music knowledge. */
export const ASSISTANT_PROMPT = `You are the Pezzottify music assistant. Help users discover music, control playback, navigate the app and manage playlists.
Use only IDs actually returned by tools. catalog.search returns artists, albums and tracks; playlist additions require track IDs, never artist or album IDs. Use catalog.get to discover tracks. ui.createPlaylist returns a playlistId; use that ID for additions. Never invent IDs.
Be concise. Use the available tools to act on requests. Ask before destructive actions when prompted by the application.`;
export function assistantModes(tools) {
  const names = tools.map(tool => tool.name);
  const select = predicate => names.filter(predicate);
  return {
    id: 'general', name: 'General', prompt: 'Help with all available music and application features.', toolIds: names,
    children: [
      { id: 'catalog', name: 'Music Discovery', prompt: 'Focus on finding artists, albums and tracks. Use catalog search and details before playing an album.', toolIds: select(n => ['catalog.search', 'catalog.get', 'ui.search', 'ui.navigate', 'ui.playAlbum', 'ui.help'].includes(n)) },
      { id: 'playback', name: 'Playback Control', prompt: 'Control playback and the queue. Check current playback state when needed.', toolIds: select(n => ['ui.play', 'ui.pause', 'ui.playPause', 'ui.next', 'ui.previous', 'ui.queue', 'ui.setVolume', 'ui.getCurrentTrack', 'ui.playAlbum', 'ui.playPlaylist', 'ui.search', 'catalog.search', 'catalog.get', 'ui.help'].includes(n)) },
      { id: 'playlists', name: 'Playlist Management', prompt: 'Manage playlists using real playlist and track IDs. Never assume that a playlist name is its ID.', toolIds: select(n => ['ui.getPlaylists', 'ui.createPlaylist', 'ui.addToPlaylist', 'ui.deletePlaylist', 'ui.playPlaylist', 'catalog.search', 'catalog.get', 'ui.help'].includes(n)) },
      { id: 'help', name: 'App Help', prompt: 'Explain Pezzottify features. Switch to an action mode when the user wants to perform a task.', toolIds: [] },
    ],
  };
}
export function requiresConfirmation(name) {
  return ['ui.deletePlaylist', 'catalog.mutate', 'users.mutate', 'jobs.action'].includes(name);
}
