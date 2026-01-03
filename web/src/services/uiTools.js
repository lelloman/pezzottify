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
 */

import { usePlayerStore } from '../store/player';
import { useUserStore } from '../store/user';
import { useStaticsStore } from '../store/statics';
import router from '../router';

/**
 * UI Tools definitions
 * Each tool has: name, description, inputSchema, execute(args)
 */
const tools = [
  // ============================================================================
  // PLAYBACK TOOLS
  // ============================================================================
  {
    name: 'ui.play',
    description: 'Start or resume playback. Optionally play a specific track by ID.',
    inputSchema: {
      type: 'object',
      properties: {
        trackId: {
          type: 'string',
          description: 'Optional track ID to play. If not provided, resumes current track.',
        },
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
    inputSchema: {
      type: 'object',
      properties: {},
    },
    execute: async () => {
      const playerStore = usePlayerStore();
      playerStore.setIsPlaying(false);
      return { success: true, message: 'Playback paused' };
    },
  },
  {
    name: 'ui.playPause',
    description: 'Toggle play/pause.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
    execute: async () => {
      const playerStore = usePlayerStore();
      playerStore.playPause();
      return { success: true, playing: playerStore.isPlaying };
    },
  },
  {
    name: 'ui.next',
    description: 'Skip to the next track in the queue.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
    execute: async () => {
      const playerStore = usePlayerStore();
      playerStore.skipNextTrack();
      return { success: true, message: 'Skipped to next track' };
    },
  },
  {
    name: 'ui.previous',
    description: 'Go to the previous track in the queue.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
    execute: async () => {
      const playerStore = usePlayerStore();
      playerStore.skipPreviousTrack();
      return { success: true, message: 'Went to previous track' };
    },
  },
  {
    name: 'ui.queue',
    description: 'Add one or more tracks to the current playback queue.',
    inputSchema: {
      type: 'object',
      properties: {
        trackIds: {
          type: 'array',
          items: { type: 'string' },
          description: 'Array of track IDs to add to the queue.',
        },
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
    description: 'Set the playback volume.',
    inputSchema: {
      type: 'object',
      properties: {
        volume: {
          type: 'number',
          description: 'Volume level from 0.0 (muted) to 1.0 (max).',
          minimum: 0,
          maximum: 1,
        },
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
    description: 'Get information about the currently playing track.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
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
    description: 'Play an album from the beginning.',
    inputSchema: {
      type: 'object',
      properties: {
        albumId: {
          type: 'string',
          description: 'The album ID to play.',
        },
      },
      required: ['albumId'],
    },
    execute: async (args) => {
      const playerStore = usePlayerStore();
      await playerStore.setAlbumId(args.albumId);
      return { success: true, message: 'Album playback started' };
    },
  },

  // ============================================================================
  // NAVIGATION TOOLS
  // ============================================================================
  {
    name: 'ui.navigate',
    description: 'Navigate to a specific page in the app.',
    inputSchema: {
      type: 'object',
      properties: {
        type: {
          type: 'string',
          enum: ['album', 'artist', 'track', 'playlist', 'settings', 'home'],
          description: 'Type of page to navigate to.',
        },
        id: {
          type: 'string',
          description: 'ID of the content (required for album, artist, track, playlist).',
        },
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
    description: 'Navigate to search results for a query.',
    inputSchema: {
      type: 'object',
      properties: {
        query: {
          type: 'string',
          description: 'The search query.',
        },
      },
      required: ['query'],
    },
    execute: async (args) => {
      await router.push(`/search/${encodeURIComponent(args.query)}`);
      return { success: true, message: `Searching for: ${args.query}` };
    },
  },

  // ============================================================================
  // CONTENT TOOLS (Likes)
  // ============================================================================
  {
    name: 'ui.likeAlbum',
    description: 'Like an album.',
    inputSchema: {
      type: 'object',
      properties: {
        albumId: {
          type: 'string',
          description: 'The album ID to like.',
        },
      },
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
    description: 'Unlike an album.',
    inputSchema: {
      type: 'object',
      properties: {
        albumId: {
          type: 'string',
          description: 'The album ID to unlike.',
        },
      },
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
    description: 'Like an artist.',
    inputSchema: {
      type: 'object',
      properties: {
        artistId: {
          type: 'string',
          description: 'The artist ID to like.',
        },
      },
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
    description: 'Unlike an artist.',
    inputSchema: {
      type: 'object',
      properties: {
        artistId: {
          type: 'string',
          description: 'The artist ID to unlike.',
        },
      },
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
    description: 'Get lists of liked albums, artists, and tracks.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
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

  // ============================================================================
  // PLAYLIST TOOLS
  // ============================================================================
  {
    name: 'ui.getPlaylists',
    description:
      'Get the list of user playlists. Each playlist has an "id" (UUID) and "name". Use the "id" field for ui.addToPlaylist, NOT the name.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
    execute: async () => {
      const userStore = useUserStore();
      const playlists = userStore.playlistsData?.list || [];
      return {
        success: true,
        playlists: playlists.map((p) => ({
          id: p.id,
          name: p.name,
          _note: 'Use "id" for ui.addToPlaylist',
        })),
      };
    },
  },
  {
    name: 'ui.createPlaylist',
    description:
      'Create a new playlist. Returns the playlist ID (a UUID like "NCAOCh3llHRlVRb4") which MUST be used for subsequent operations like ui.addToPlaylist. Do NOT use the playlist name as an ID.',
    inputSchema: {
      type: 'object',
      properties: {
        name: {
          type: 'string',
          description: 'Optional name for the playlist.',
        },
      },
    },
    execute: async (args) => {
      const userStore = useUserStore();
      return new Promise((resolve) => {
        userStore.createPlaylist(async (newPlaylist) => {
          // Note: newPlaylist can be either:
          // - A string (the playlist ID) when freshly created from server
          // - An object { id, name, ... } when fetched from playlist list
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
            message: `Created playlist with ID "${playlistId}". Use this ID for ui.addToPlaylist.`,
          });
        });
      });
    },
  },
  {
    name: 'ui.addToPlaylist',
    description:
      'Add tracks to a playlist. IMPORTANT: playlistId must be the UUID returned by ui.createPlaylist or ui.getPlaylists (e.g. "NCAOCh3llHRlVRb4"), NOT the playlist name. trackIds must be track IDs from catalog.search results (from the "tracks" array), NOT album or artist IDs.',
    inputSchema: {
      type: 'object',
      properties: {
        playlistId: {
          type: 'string',
          description:
            'The playlist UUID (e.g. "NCAOCh3llHRlVRb4"). Must be an ID from ui.createPlaylist or ui.getPlaylists, NOT a playlist name.',
        },
        trackIds: {
          type: 'array',
          items: { type: 'string' },
          description:
            'Track IDs to add. Must be IDs from the "tracks" array of catalog.search, NOT album or artist IDs.',
        },
      },
      required: ['playlistId', 'trackIds'],
    },
    execute: async (args) => {
      const userStore = useUserStore();

      // Validate playlistId doesn't look like a name
      if (!args.playlistId || args.playlistId.includes(' ') || args.playlistId === 'undefined') {
        return {
          success: false,
          error: `Invalid playlistId "${args.playlistId}". Use the UUID from ui.createPlaylist or ui.getPlaylists, not the playlist name.`,
        };
      }

      // Validate trackIds
      if (!args.trackIds || args.trackIds.length === 0) {
        return { success: false, error: 'No track IDs provided' };
      }

      return new Promise((resolve) => {
        userStore.addTracksToPlaylist(args.playlistId, args.trackIds, (success) => {
          resolve({
            success,
            message: success
              ? `Added ${args.trackIds.length} track(s) to playlist`
              : 'Failed to add tracks. Verify: 1) playlistId is a valid UUID from ui.createPlaylist, 2) trackIds are from the "tracks" array of catalog.search (not albums or artists)',
          });
        });
      });
    },
  },

  // ============================================================================
  // SETTINGS TOOLS
  // ============================================================================
  {
    name: 'ui.getSetting',
    description: 'Get a user setting value.',
    inputSchema: {
      type: 'object',
      properties: {
        key: {
          type: 'string',
          description: 'The setting key to retrieve.',
        },
      },
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
    description: 'Set a user setting value.',
    inputSchema: {
      type: 'object',
      properties: {
        key: {
          type: 'string',
          description: 'The setting key.',
        },
        value: {
          description: 'The setting value (string, number, or boolean).',
        },
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
const toolsMap = new Map(tools.map(t => [t.name, t]));

/**
 * Get all UI tools in unified format for LLM
 */
export function getTools() {
  return tools.map(t => ({
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
