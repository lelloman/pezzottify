/**
 * UI Tools for AI Chat
 *
 * Local tools that control the web interface:
 * - Playback (play, pause, skip, queue, volume)
 * - Navigation (navigate to pages, search)
 * - Content (like/unlike, playlists)
 * - Settings (read/write user settings)
 *
 * These tools are executed locally in the browser and call Pinia store methods.
 *
 * Tools use minimal descriptions to reduce context size.
 * Use ui.help to get detailed documentation for a category.
 */

import { usePlayerStore } from '../store/player';
import { useUserStore } from '../store/user';
import { useStaticsStore } from '../store/statics';
import router from '../router';

// ============================================================================
// DETAILED DOCUMENTATION (returned by ui.help)
// ============================================================================

const detailedDocs = {
  playback: {
    description: 'Control music playback - play, pause, skip, volume, queue tracks',
    tools: {
      'ui.play': {
        description: 'Start or resume playback. Optionally play a specific track by ID.',
        params: {
          trackId: 'Optional track ID to play. If not provided, resumes current track.',
        },
        example: '{ "trackId": "6Dp8OZXAfmhnEEbn9WQmlh" }',
      },
      'ui.pause': {
        description: 'Pause the current playback.',
        params: {},
      },
      'ui.playPause': {
        description: 'Toggle between play and pause states.',
        params: {},
      },
      'ui.next': {
        description: 'Skip to the next track in the current queue.',
        params: {},
      },
      'ui.previous': {
        description: 'Go back to the previous track in the queue.',
        params: {},
      },
      'ui.queue': {
        description: 'Add tracks to the end of the current playback queue.',
        params: {
          trackIds: 'Array of track IDs to add. Get these from catalog.search "tracks" array.',
        },
        example: '{ "trackIds": ["id1", "id2", "id3"] }',
      },
      'ui.setVolume': {
        description: 'Set the playback volume level.',
        params: {
          volume: 'Number from 0.0 (muted) to 1.0 (max volume).',
        },
        example: '{ "volume": 0.5 }',
      },
      'ui.getCurrentTrack': {
        description: 'Get info about the currently playing track including progress and volume.',
        params: {},
        returns: '{ currentTrack: { id, name, isPlaying, progressPercent, progressSec, volume, muted } }',
      },
      'ui.playAlbum': {
        description: 'Start playing an entire album from the beginning.',
        params: {
          albumId: 'The album ID to play. Get from catalog.search "albums" array.',
        },
        example: '{ "albumId": "4aawyAB9vmqN3uQ7FjRGTy" }',
      },
      'ui.playPlaylist': {
        description: 'Start playing a user playlist from the beginning.',
        params: {
          playlistId: 'The playlist UUID. Get from ui.createPlaylist or ui.getPlaylists.',
        },
        example: '{ "playlistId": "NCAOCh3llHRlVRb4" }',
        notes: 'Use the playlist ID (UUID), NOT the playlist name.',
      },
    },
  },
  navigation: {
    description: 'Navigate to different pages in the app',
    tools: {
      'ui.navigate': {
        description: 'Navigate to a specific page in the app.',
        params: {
          type: 'One of: "album", "artist", "track", "playlist", "settings", "home"',
          id: 'Required for album/artist/track/playlist. The content ID.',
        },
        example: '{ "type": "album", "id": "4aawyAB9vmqN3uQ7FjRGTy" }',
      },
      'ui.search': {
        description: 'Navigate to search results page for a query.',
        params: {
          query: 'The search query string.',
        },
        example: '{ "query": "jazz piano" }',
      },
    },
  },
  content: {
    description: 'Like/unlike albums and artists, view liked content',
    tools: {
      'ui.likeAlbum': {
        description: 'Add an album to liked albums.',
        params: { albumId: 'The album ID to like.' },
      },
      'ui.unlikeAlbum': {
        description: 'Remove an album from liked albums.',
        params: { albumId: 'The album ID to unlike.' },
      },
      'ui.likeArtist': {
        description: 'Add an artist to liked artists.',
        params: { artistId: 'The artist ID to like.' },
      },
      'ui.unlikeArtist': {
        description: 'Remove an artist from liked artists.',
        params: { artistId: 'The artist ID to unlike.' },
      },
      'ui.getLikedContent': {
        description: 'Get lists of all liked albums, artists, and tracks.',
        params: {},
        returns: '{ likedAlbums: [...], likedArtists: [...], likedTracks: [...] }',
      },
    },
  },
  playlists: {
    description: 'Create and manage user playlists',
    tools: {
      'ui.getPlaylists': {
        description: 'Get all user playlists with their IDs and names.',
        params: {},
        returns: '{ playlists: [{ id: "UUID", name: "Playlist Name" }, ...] }',
        notes: 'Use the "id" field for other playlist operations, NOT the name.',
      },
      'ui.createPlaylist': {
        description: 'Create a new empty playlist.',
        params: {
          name: 'Optional name for the playlist. Defaults to "New Playlist".',
        },
        returns: '{ playlistId: "UUID", name: "..." }',
        notes: 'IMPORTANT: Save the returned playlistId to use with ui.addToPlaylist.',
        example: '{ "name": "My Jazz Mix" }',
      },
      'ui.addToPlaylist': {
        description: 'Add tracks to an existing playlist.',
        params: {
          playlistId: 'The playlist UUID from ui.createPlaylist or ui.getPlaylists.',
          trackIds: 'Array of track IDs from catalog.search "tracks" array.',
        },
        example: '{ "playlistId": "NCAOCh3llHRlVRb4", "trackIds": ["track1", "track2"] }',
        notes:
          'CRITICAL: playlistId must be a UUID, NOT the playlist name. trackIds must be from the "tracks" array of catalog.search, NOT album or artist IDs.',
      },
      'ui.deletePlaylist': {
        description: 'Delete a playlist permanently.',
        params: {
          playlistId: 'The playlist UUID from ui.getPlaylists.',
        },
        example: '{ "playlistId": "NCAOCh3llHRlVRb4" }',
        notes: 'This action cannot be undone.',
      },
    },
  },
  settings: {
    description: 'Read and write user settings',
    tools: {
      'ui.getSetting': {
        description: 'Get a user setting value by key.',
        params: { key: 'The setting key to retrieve.' },
      },
      'ui.setSetting': {
        description: 'Set a user setting value.',
        params: {
          key: 'The setting key.',
          value: 'The setting value (string, number, or boolean).',
        },
      },
    },
  },
};

// ============================================================================
// TOOL DEFINITIONS (minimal descriptions)
// ============================================================================

const tools = [
  // META TOOL - Get detailed documentation
  {
    name: 'ui.help',
    description:
      'Get detailed documentation for UI tools. Categories: playback, navigation, content, playlists, settings. Call this BEFORE using unfamiliar ui.* tools.',
    inputSchema: {
      type: 'object',
      properties: {
        category: {
          type: 'string',
          enum: ['playback', 'navigation', 'content', 'playlists', 'settings', 'all'],
          description: 'Category to get help for, or "all" for everything.',
        },
      },
      required: ['category'],
    },
    execute: async (args) => {
      if (args.category === 'all') {
        return { success: true, documentation: detailedDocs };
      }
      const docs = detailedDocs[args.category];
      if (!docs) {
        return {
          success: false,
          error: `Unknown category "${args.category}". Use: playback, navigation, content, playlists, settings, or all.`,
        };
      }
      return { success: true, category: args.category, documentation: docs };
    },
  },

  // PLAYBACK TOOLS
  {
    name: 'ui.play',
    description: 'Play or resume. Optional trackId param.',
    inputSchema: {
      type: 'object',
      properties: {
        trackId: { type: 'string' },
      },
    },
    execute: async (args) => {
      const playerStore = usePlayerStore();
      if (args.trackId) {
        const staticsStore = useStaticsStore();
        const track = await staticsStore.waitTrackData(args.trackId);
        if (track) {
          playerStore.setTrack(track);
          return { success: true, message: `Now playing: ${track.name}` };
        }
        return { success: false, error: 'Track not found' };
      }
      playerStore.setIsPlaying(true);
      return { success: true, message: 'Playback resumed' };
    },
  },
  {
    name: 'ui.pause',
    description: 'Pause playback.',
    inputSchema: { type: 'object', properties: {} },
    execute: async () => {
      const playerStore = usePlayerStore();
      playerStore.setIsPlaying(false);
      return { success: true, message: 'Playback paused' };
    },
  },
  {
    name: 'ui.playPause',
    description: 'Toggle play/pause.',
    inputSchema: { type: 'object', properties: {} },
    execute: async () => {
      const playerStore = usePlayerStore();
      playerStore.playPause();
      return { success: true, playing: playerStore.isPlaying };
    },
  },
  {
    name: 'ui.next',
    description: 'Skip to next track.',
    inputSchema: { type: 'object', properties: {} },
    execute: async () => {
      const playerStore = usePlayerStore();
      playerStore.skipNextTrack();
      return { success: true, message: 'Skipped to next track' };
    },
  },
  {
    name: 'ui.previous',
    description: 'Go to previous track.',
    inputSchema: { type: 'object', properties: {} },
    execute: async () => {
      const playerStore = usePlayerStore();
      playerStore.skipPreviousTrack();
      return { success: true, message: 'Went to previous track' };
    },
  },
  {
    name: 'ui.queue',
    description: 'Add tracks to queue. Requires trackIds array.',
    inputSchema: {
      type: 'object',
      properties: {
        trackIds: { type: 'array', items: { type: 'string' } },
      },
      required: ['trackIds'],
    },
    execute: async (args) => {
      const playerStore = usePlayerStore();
      if (!args.trackIds || args.trackIds.length === 0) {
        return { success: false, error: 'No track IDs provided' };
      }
      playerStore.addTracksToPlaylist(args.trackIds);
      return { success: true, message: `Added ${args.trackIds.length} track(s) to queue` };
    },
  },
  {
    name: 'ui.setVolume',
    description: 'Set volume (0.0-1.0).',
    inputSchema: {
      type: 'object',
      properties: {
        volume: { type: 'number', minimum: 0, maximum: 1 },
      },
      required: ['volume'],
    },
    execute: async (args) => {
      const playerStore = usePlayerStore();
      playerStore.setVolume(args.volume);
      return { success: true, volume: args.volume };
    },
  },
  {
    name: 'ui.getCurrentTrack',
    description: 'Get current track info.',
    inputSchema: { type: 'object', properties: {} },
    execute: async () => {
      const playerStore = usePlayerStore();
      const staticsStore = useStaticsStore();

      if (!playerStore.currentTrackId) {
        return { success: true, currentTrack: null, message: 'No track currently playing' };
      }

      const track = await staticsStore.waitTrackData(playerStore.currentTrackId);
      return {
        success: true,
        currentTrack: {
          id: playerStore.currentTrackId,
          name: track?.name,
          isPlaying: playerStore.isPlaying,
          progressPercent: playerStore.progressPercent,
          progressSec: playerStore.progressSec,
          volume: playerStore.volume,
          muted: playerStore.muted,
        },
      };
    },
  },
  {
    name: 'ui.playAlbum',
    description: 'Play an album. Requires albumId.',
    inputSchema: {
      type: 'object',
      properties: {
        albumId: { type: 'string' },
      },
      required: ['albumId'],
    },
    execute: async (args) => {
      const playerStore = usePlayerStore();
      await playerStore.setAlbumId(args.albumId);
      return { success: true, message: 'Album playback started' };
    },
  },
  {
    name: 'ui.playPlaylist',
    description: 'Play a user playlist. Requires playlistId (UUID, not name).',
    inputSchema: {
      type: 'object',
      properties: {
        playlistId: { type: 'string' },
      },
      required: ['playlistId'],
    },
    execute: async (args) => {
      const userStore = useUserStore();
      const playerStore = usePlayerStore();

      const playlist = userStore.playlistsData?.by_id?.[args.playlistId];
      if (!playlist) {
        return {
          success: false,
          error: `Playlist not found. Use ui.help({ category: "playlists" }) for correct usage.`,
        };
      }

      if (!playlist.tracks || playlist.tracks.length === 0) {
        return { success: false, error: 'Playlist is empty' };
      }

      await playerStore.setUserPlaylist(playlist);
      return {
        success: true,
        message: `Now playing playlist "${playlist.name}" with ${playlist.tracks.length} track(s)`,
      };
    },
  },

  // NAVIGATION TOOLS
  {
    name: 'ui.navigate',
    description: 'Navigate to page. Requires type (album/artist/track/playlist/settings/home), id for content.',
    inputSchema: {
      type: 'object',
      properties: {
        type: { type: 'string', enum: ['album', 'artist', 'track', 'playlist', 'settings', 'home'] },
        id: { type: 'string' },
      },
      required: ['type'],
    },
    execute: async (args) => {
      const routes = {
        album: (id) => `/album/${id}`,
        artist: (id) => `/artist/${id}`,
        track: (id) => `/track/${id}`,
        playlist: (id) => `/playlist/${id}`,
        settings: () => '/settings',
        home: () => '/',
      };

      const routeFn = routes[args.type];
      if (!routeFn) {
        return { success: false, error: `Unknown page type: ${args.type}` };
      }

      if (['album', 'artist', 'track', 'playlist'].includes(args.type) && !args.id) {
        return { success: false, error: `ID required for ${args.type}` };
      }

      const path = routeFn(args.id);
      await router.push(path);
      return { success: true, navigatedTo: path };
    },
  },
  {
    name: 'ui.search',
    description: 'Navigate to search results. Requires query.',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string' },
      },
      required: ['query'],
    },
    execute: async (args) => {
      await router.push(`/search/${encodeURIComponent(args.query)}`);
      return { success: true, message: `Searching for: ${args.query}` };
    },
  },

  // CONTENT TOOLS (Likes)
  {
    name: 'ui.likeAlbum',
    description: 'Like an album. Requires albumId.',
    inputSchema: {
      type: 'object',
      properties: { albumId: { type: 'string' } },
      required: ['albumId'],
    },
    execute: async (args) => {
      const userStore = useUserStore();
      await userStore.setAlbumIsLiked(args.albumId, true);
      return { success: true, message: 'Album liked' };
    },
  },
  {
    name: 'ui.unlikeAlbum',
    description: 'Unlike an album. Requires albumId.',
    inputSchema: {
      type: 'object',
      properties: { albumId: { type: 'string' } },
      required: ['albumId'],
    },
    execute: async (args) => {
      const userStore = useUserStore();
      await userStore.setAlbumIsLiked(args.albumId, false);
      return { success: true, message: 'Album unliked' };
    },
  },
  {
    name: 'ui.likeArtist',
    description: 'Like an artist. Requires artistId.',
    inputSchema: {
      type: 'object',
      properties: { artistId: { type: 'string' } },
      required: ['artistId'],
    },
    execute: async (args) => {
      const userStore = useUserStore();
      await userStore.setArtistIsLiked(args.artistId, true);
      return { success: true, message: 'Artist liked' };
    },
  },
  {
    name: 'ui.unlikeArtist',
    description: 'Unlike an artist. Requires artistId.',
    inputSchema: {
      type: 'object',
      properties: { artistId: { type: 'string' } },
      required: ['artistId'],
    },
    execute: async (args) => {
      const userStore = useUserStore();
      await userStore.setArtistIsLiked(args.artistId, false);
      return { success: true, message: 'Artist unliked' };
    },
  },
  {
    name: 'ui.getLikedContent',
    description: 'Get liked albums, artists, and tracks.',
    inputSchema: { type: 'object', properties: {} },
    execute: async () => {
      const userStore = useUserStore();
      return {
        success: true,
        likedAlbums: userStore.likedAlbumIds || [],
        likedArtists: userStore.likedArtistsIds || [],
        likedTracks: userStore.likedTrackIds || [],
      };
    },
  },

  // PLAYLIST TOOLS
  {
    name: 'ui.getPlaylists',
    description: 'Get user playlists. Returns id and name for each.',
    inputSchema: { type: 'object', properties: {} },
    execute: async () => {
      const userStore = useUserStore();
      const playlists = userStore.playlistsData?.list || [];
      return {
        success: true,
        playlists: playlists.map((p) => ({ id: p.id, name: p.name })),
      };
    },
  },
  {
    name: 'ui.createPlaylist',
    description: 'Create playlist. Optional name. Returns playlistId (UUID) for use with ui.addToPlaylist.',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string' },
      },
    },
    execute: async (args) => {
      const userStore = useUserStore();
      return new Promise((resolve) => {
        userStore.createPlaylist(async (newPlaylist) => {
          const playlistId = typeof newPlaylist === 'string' ? newPlaylist : newPlaylist?.id;

          if (!playlistId) {
            resolve({ success: false, error: 'Failed to create playlist' });
            return;
          }

          const playlistName = args.name || 'New Playlist';
          if (args.name) {
            await new Promise((nameResolve) => {
              userStore.updatePlaylistName(playlistId, args.name, nameResolve);
            });
          }
          resolve({
            success: true,
            playlistId: playlistId,
            name: playlistName,
            message: `Created playlist "${playlistName}" with ID "${playlistId}". Use this ID for ui.addToPlaylist.`,
          });
        });
      });
    },
  },
  {
    name: 'ui.addToPlaylist',
    description: 'Add tracks to playlist. Requires playlistId (UUID) and trackIds array.',
    inputSchema: {
      type: 'object',
      properties: {
        playlistId: { type: 'string' },
        trackIds: { type: 'array', items: { type: 'string' } },
      },
      required: ['playlistId', 'trackIds'],
    },
    execute: async (args) => {
      const userStore = useUserStore();

      if (!args.playlistId || args.playlistId.includes(' ') || args.playlistId === 'undefined') {
        return {
          success: false,
          error: `Invalid playlistId. Use ui.help({ category: "playlists" }) for correct usage.`,
        };
      }

      if (!args.trackIds || args.trackIds.length === 0) {
        return { success: false, error: 'No track IDs provided' };
      }

      return new Promise((resolve) => {
        userStore.addTracksToPlaylist(args.playlistId, args.trackIds, (success) => {
          resolve({
            success,
            message: success
              ? `Added ${args.trackIds.length} track(s) to playlist`
              : 'Failed to add tracks. Use ui.help({ category: "playlists" }) for correct usage.',
          });
        });
      });
    },
  },
  {
    name: 'ui.deletePlaylist',
    description: 'Delete a playlist. Requires playlistId (UUID).',
    inputSchema: {
      type: 'object',
      properties: {
        playlistId: { type: 'string' },
      },
      required: ['playlistId'],
    },
    execute: async (args) => {
      const userStore = useUserStore();

      if (!args.playlistId || args.playlistId.includes(' ') || args.playlistId === 'undefined') {
        return {
          success: false,
          error: `Invalid playlistId. Use ui.getPlaylists to get valid playlist IDs.`,
        };
      }

      const playlist = userStore.playlistsData?.by_id?.[args.playlistId];
      const playlistName = playlist?.name || args.playlistId;

      return new Promise((resolve) => {
        userStore.deletePlaylist(args.playlistId, (success) => {
          resolve({
            success,
            message: success
              ? `Deleted playlist "${playlistName}"`
              : 'Failed to delete playlist. Make sure the playlist ID is correct.',
          });
        });
      });
    },
  },

  // SETTINGS TOOLS
  {
    name: 'ui.getSetting',
    description: 'Get a setting value. Requires key.',
    inputSchema: {
      type: 'object',
      properties: { key: { type: 'string' } },
      required: ['key'],
    },
    execute: async (args) => {
      const userStore = useUserStore();
      const value = userStore.settings[args.key];
      return { success: true, key: args.key, value };
    },
  },
  {
    name: 'ui.setSetting',
    description: 'Set a setting value. Requires key and value.',
    inputSchema: {
      type: 'object',
      properties: {
        key: { type: 'string' },
        value: {},
      },
      required: ['key', 'value'],
    },
    execute: async (args) => {
      const userStore = useUserStore();
      userStore.setSetting(args.key, args.value);
      return { success: true, key: args.key, value: args.value };
    },
  },
];

// Build tools map for quick lookup
const toolsMap = new Map(tools.map((t) => [t.name, t]));

/**
 * Get all UI tools in unified format for LLM
 */
export function getTools() {
  return tools.map((t) => ({
    name: t.name,
    description: t.description,
    inputSchema: t.inputSchema,
  }));
}

/**
 * Execute a UI tool by name
 */
export async function callTool(name, args) {
  const tool = toolsMap.get(name);
  if (!tool) {
    return { success: false, error: `Unknown UI tool: ${name}` };
  }

  try {
    return await tool.execute(args || {});
  } catch (error) {
    console.error(`UI tool ${name} failed:`, error);
    return { success: false, error: error.message };
  }
}

/**
 * Check if a tool name is a UI tool
 */
export function isUiTool(name) {
  return name.startsWith('ui.');
}

export const uiTools = {
  getTools,
  callTool,
  isUiTool,
};
