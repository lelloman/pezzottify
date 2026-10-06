// Deliberately fictional catalog: stable IDs, original lyrics, locally drawn covers.
export const artists = [
  "Mira Sol",
  "The Night Trains",
  "Juniper Coast",
  "Atlas Quartet",
  "Velvet Circuit",
  "Luca Moretti",
].map((name, i) => ({
  id: `artist-${i + 1}`,
  name,
  genres: [
    ["soul", "jazz"],
    ["indie rock"],
    ["ambient"],
    ["classical"],
    ["electronic"],
    ["jazz"],
  ][i],
  available: true,
  image_id: `artist-${i + 1}`,
  biography: `${name} explores the space between melody, texture and rhythm. This fictional artist is part of the local design catalog.`,
}));
export const albums = [
  "Golden Hour",
  "Last Train Home",
  "Tidal Patterns",
  "Rooms of Light",
  "Afterimage",
  "Sunday in Rome",
  "Blue Windows",
  "Signals at Dawn",
  "The Quiet Season",
  "Nocturnes",
  "Electric Gardens",
  "A Little Further",
].map((name, i) => ({
  id: `album-${i + 1}`,
  name,
  album_type: "album",
  release_date: `${2025 - i}-06-15`,
  year: 2025 - i,
  availability: i === 11 ? "missing" : "complete",
  artists_ids: [artists[i % artists.length].id],
  artist_names: [artists[i % artists.length].name],
  artists_ids_names: [
    [artists[i % artists.length].id, artists[i % artists.length].name],
  ],
  image_id: `album-${i + 1}`,
  total_tracks: 6,
}));
export const tracks = albums.flatMap((album, i) =>
  [
    "First Light",
    "Open Windows",
    "Slow Motion",
    "Between the Lines",
    "Stay a Little Longer",
    "Home Again",
  ].map((title, j) => ({
    id: `track-${i * 6 + j + 1}`,
    name: i === 0 ? title : `${title} / ${i + 1}`,
    album_id: album.id,
    duration_ms: 90000,
    duration: 90000,
    track_number: j + 1,
    disc_number: 1,
    artists_ids: album.artists_ids,
    artists_ids_names: album.artists_ids_names,
    availability:
      album.availability === "missing" ? "unavailable" : "available",
    image_id: album.id,
    explicit: false,
  })),
);
export const work = {
  id: "work-1",
  title: "First Light",
  kind: "song",
  creators: ["Mira Sol"],
  creator_artist_ids: ["artist-1"],
  composition_year: 2025,
};
export const permissions = [
  "ManagePermissions",
  "ViewAnalytics",
  "ServerAdmin",
  "DownloadManagerAdmin",
  "EditCatalog",
  "RequestDownload",
  "UploadContent",
];
export const genres = [...new Set(artists.flatMap((a) => a.genres))].map(
  (name) => ({ name, track_count: 24 }),
);
export const screens = [
  ["Home", "/"],
  ["Search", "/search/Light"],
  ["Album", "/album/album-1"],
  ["Missing album", "/album/album-12"],
  ["Artist", "/artist/artist-1"],
  ["Track / lyrics", "/track/track-1"],
  ["Composition", "/work/work-1"],
  ["Playlist", "/playlist/playlist-1"],
  ["Genres", "/genres"],
  ["Genre", "/genre/jazz"],
  ["Now playing", "/now-playing"],
  ["Steering", "/steering"],
  ["Devices", "/devices"],
  ["Settings", "/settings"],
  ["Requests", "/requests"],
  ...[
    "users",
    "analytics",
    "server",
    "downloads",
    "batches",
    "bug-reports",
    "ingestion",
    "push",
  ].map((s) => [`Admin: ${s}`, `/admin/${s}`]),
  ["Login", "/login"],
];
export function freshState() {
  return {
    scenario: "populated",
    loggedIn: true,
    seq: 1,
    likes: {
      albums: ["album-1", "album-3", "album-6"],
      artists: ["artist-1", "artist-4"],
      tracks: ["track-1", "track-4"],
    },
    playlists: [
      {
        id: "playlist-1",
        name: "Late night discoveries",
        tracks: tracks.slice(0, 18).map((t) => t.id),
      },
      {
        id: "playlist-2",
        name: "A softer morning",
        tracks: tracks.slice(18, 30).map((t) => t.id),
      },
    ],
    settings: [],
    notifications: [],
    permissions,
    features: { proxy_streaming: true },
  };
}
export const resolvedTrack = (t) => ({
  track: t,
  album: albums.find((a) => a.id === t.album_id),
  artists: artists
    .filter((a) => t.artists_ids.includes(a.id))
    .map((artist) => ({ artist, role: "MainArtist" })),
});
export const resolvedAlbum = (a) => ({
  album: a,
  artists: artists.filter((b) => a.artists_ids.includes(b.id)),
  discs: [{ number: 1, tracks: tracks.filter((t) => t.album_id === a.id) }],
  display_image: { id: a.id },
});
export function artwork(id) {
  const i = Number(id.split("-").at(-1)) || 1;
  const title =
    [...albums, ...artists].find((x) => x.id === id)?.name || "Pezzottify";
  const colors = [
    "#de8b45",
    "#4658a2",
    "#217b77",
    "#927684",
    "#b73265",
    "#798344",
  ];
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 600"><defs><linearGradient id="g" x2="1" y2="1"><stop stop-color="${colors[(i - 1) % 6]}"/><stop offset="1" stop-color="#101823"/></linearGradient></defs><rect width="600" height="600" fill="url(#g)"/><circle cx="${150 + ((i * 17) % 300)}" cy="245" r="190" fill="#ffffff18"/><circle cx="380" cy="300" r="145" fill="none" stroke="#ffffff55" stroke-width="2"/><path d="M0 450 Q150 100 600 350 L600 600H0" fill="#0003"/><text x="42" y="475" fill="white" font-family="sans-serif" font-size="38">${title}</text><text x="44" y="525" fill="#ffffffaa" font-family="sans-serif" font-size="17" letter-spacing="5">LOCAL SESSIONS · ${String(i).padStart(2, "0")}</text></svg>`;
}
// A quiet original test tone, not a real recording. Range requests allow seeking.
export function audio() {
  const rate = 8000,
    seconds = 90,
    b = Buffer.alloc(44 + rate * seconds * 2);
  b.write("RIFF");
  b.writeUInt32LE(b.length - 8, 4);
  b.write("WAVEfmt ", 8);
  b.writeUInt32LE(16, 16);
  b.writeUInt16LE(1, 20);
  b.writeUInt16LE(1, 22);
  b.writeUInt32LE(rate, 24);
  b.writeUInt32LE(rate * 2, 28);
  b.writeUInt16LE(2, 32);
  b.writeUInt16LE(16, 34);
  b.write("data", 36);
  b.writeUInt32LE(b.length - 44, 40);
  for (let i = 0; i < rate * seconds; i++)
    b.writeInt16LE(
      Math.round(
        Math.sin((i / rate) * Math.PI * 440) * 180 * Math.min(1, i / rate),
      ),
      44 + i * 2,
    );
  return b;
}
