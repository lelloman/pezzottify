/**
 * User-friendly descriptions for tool calls
 *
 * Transforms technical tool calls into human-readable descriptions
 */

/**
 * Tool description generators
 * Each function takes the tool input and returns a friendly description
 */
const TOOL_DESCRIPTIONS = {
  // Catalog tools
  'catalog.search': (input) => {
    const query = input?.query || input?.q || 'music';
    return `Searching for "${query}"...`;
  },
  'catalog.get': (input) => {
    const type = input?.query_type || input?.type || 'content';
    if (type === 'artist') return 'Loading artist details...';
    if (type === 'album') return 'Loading album details...';
    if (type === 'track') return 'Loading track details...';
    if (type === 'recent') return 'Loading recent additions...';
    if (type === 'stats') return 'Loading catalog stats...';
    return 'Loading content...';
  },

  // UI playback tools
  'ui.play': () => 'Starting playback...',
  'ui.pause': () => 'Pausing playback...',
  'ui.playPause': () => 'Toggling playback...',
  'ui.resume': () => 'Resuming playback...',
  'ui.stop': () => 'Stopping playback...',
  'ui.next': () => 'Skipping to next track...',
  'ui.previous': () => 'Going to previous track...',
  'ui.seek': (input) => `Seeking to ${formatTime(input?.position)}...`,
  'ui.setVolume': (input) => `Setting volume to ${Math.round((input?.volume || 0) * 100)}%...`,
  'ui.playTrack': () => 'Playing track...',
  'ui.playAlbum': () => 'Playing album...',
  'ui.playPlaylist': () => 'Playing playlist...',
  'ui.queue': () => 'Adding to queue...',
  'ui.queueTrack': () => 'Adding to queue...',
  'ui.queueAlbum': () => 'Adding album to queue...',

  // UI navigation tools
  'ui.navigate': (input) => {
    const path = input?.path || '';
    if (path.includes('artist')) return 'Opening artist page...';
    if (path.includes('album')) return 'Opening album page...';
    if (path.includes('playlist')) return 'Opening playlist...';
    if (path.includes('search')) return 'Opening search...';
    if (path.includes('settings')) return 'Opening settings...';
    return 'Navigating...';
  },
  'ui.search': (input) => {
    const query = input?.query || input?.q;
    return query ? `Searching for "${query}"...` : 'Opening search...';
  },
  'ui.goToArtist': () => 'Opening artist page...',
  'ui.goToAlbum': () => 'Opening album page...',
  'ui.goToTrack': () => 'Opening track page...',
  'ui.goToPlaylist': () => 'Opening playlist...',

  // UI content management tools
  'ui.likeTrack': () => 'Liking track...',
  'ui.unlikeTrack': () => 'Removing from liked tracks...',
  'ui.likeAlbum': () => 'Liking album...',
  'ui.unlikeAlbum': () => 'Removing from liked albums...',
  'ui.likeArtist': () => 'Following artist...',
  'ui.unlikeArtist': () => 'Unfollowing artist...',
  'ui.getLikedContent': () => 'Loading liked content...',

  // UI playlist tools
  'ui.getPlaylists': () => 'Loading playlists...',
  'ui.createPlaylist': (input) => {
    const name = input?.name;
    return name ? `Creating playlist "${name}"...` : 'Creating playlist...';
  },
  'ui.deletePlaylist': () => 'Deleting playlist...',
  'ui.addToPlaylist': () => 'Adding to playlist...',
  'ui.removeFromPlaylist': () => 'Removing from playlist...',

  // UI settings tools
  'ui.getSetting': () => 'Loading setting...',
  'ui.getSettings': () => 'Loading settings...',
  'ui.setSetting': (input) => `Updating ${input?.key || 'setting'}...`,

  // UI info tools
  'ui.getCurrentTrack': () => 'Checking current track...',
  'ui.getQueue': () => 'Loading queue...',
  'ui.getPlayerState': () => 'Checking player status...',
  'ui.help': () => 'Loading help...',

  // Admin/analytics tools (less common in regular chat)
  'analytics.query': () => 'Loading analytics...',
  'users.query': () => 'Loading user info...',
  'users.mutate': () => 'Updating user...',
  'server.query': () => 'Checking server status...',
  'jobs.query': () => 'Loading jobs...',
  'jobs.action': () => 'Running job action...',
  'downloads.query': () => 'Checking downloads...',
  'downloads.action': () => 'Processing download request...',
};

/**
 * Format seconds into mm:ss
 */
function formatTime(seconds) {
  if (!seconds || typeof seconds !== 'number') return '0:00';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

/**
 * Get a user-friendly description for a tool call
 *
 * @param {string} toolName - The tool name (e.g., 'catalog.search')
 * @param {object} input - The tool input parameters
 * @returns {string} A friendly description
 */
export function getToolDescription(toolName, input) {
  const describer = TOOL_DESCRIPTIONS[toolName];
  if (describer) {
    try {
      return describer(input);
    } catch {
      // Fall through to default
    }
  }

  // Default: convert tool name to readable format
  // e.g., 'catalog.search' -> 'Catalog search...'
  const parts = toolName.split('.');
  const action = parts[parts.length - 1];
  const readable = action
    .replace(/([A-Z])/g, ' $1')
    .replace(/^./, str => str.toUpperCase())
    .trim();

  return `${readable}...`;
}

/**
 * Get a user-friendly result description
 *
 * @param {string} toolName - The tool name
 * @param {object|string} result - The tool result
 * @returns {string} A friendly result description
 */
export function getToolResultDescription(toolName, result) {
  // Parse result if it's a string
  let parsed = result;
  if (typeof result === 'string') {
    try {
      parsed = JSON.parse(result);
    } catch {
      parsed = { raw: result };
    }
  }

  // Check for success/error
  if (parsed?.success === true || parsed?.success === 'true') {
    return 'Done';
  }
  if (parsed?.error) {
    return `Error: ${parsed.error}`;
  }

  // Tool-specific result descriptions
  if (toolName === 'catalog.search') {
    const artists = parsed?.artists?.length || 0;
    const albums = parsed?.albums?.length || 0;
    const tracks = parsed?.tracks?.length || 0;
    const total = artists + albums + tracks;
    if (total === 0) return 'No results found';
    const parts = [];
    if (artists) parts.push(`${artists} artist${artists > 1 ? 's' : ''}`);
    if (albums) parts.push(`${albums} album${albums > 1 ? 's' : ''}`);
    if (tracks) parts.push(`${tracks} track${tracks > 1 ? 's' : ''}`);
    return `Found ${parts.join(', ')}`;
  }

  if (toolName === 'ui.createPlaylist' && parsed?.playlistId) {
    return 'Playlist created';
  }

  if (toolName === 'ui.getCurrentTrack' && parsed?.track) {
    return `Now playing: ${parsed.track.name || 'Unknown'}`;
  }

  // Default
  return 'Done';
}
