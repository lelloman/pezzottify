// LRCLIB uses LRC timestamps; a line may have several timestamps (repeated verses).
export function parseSyncedLyrics(text) {
  if (!text) return [];
  const offset = Number(text.match(/\[offset:([+-]?\d+)\]/i)?.[1] || 0) / 1000;
  const lines = [];
  for (const raw of text.split(/\r?\n/)) {
    const stamps = [...raw.matchAll(/\[(\d+):([0-5]\d)(?:\.(\d{1,3}))?\]/g)];
    const content = raw.replace(/\[[^\]]*\]/g, "").trim();
    for (const stamp of stamps) {
      lines.push({
        time: Math.max(
          0,
          Number(stamp[1]) * 60 +
            Number(stamp[2]) +
            Number(`0.${stamp[3] || 0}`) -
            offset,
        ),
        text: content,
      });
    }
  }
  return lines.sort((a, b) => a.time - b.time);
}

export function activeLyricIndex(lines, seconds) {
  if (!Number.isFinite(seconds)) return -1;
  let index = -1;
  for (let i = 0; i < lines.length && lines[i].time <= seconds; i++) index = i;
  return index;
}
