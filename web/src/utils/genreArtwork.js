// A first curated set. Unknown genres retain a stable, designed color treatment.
const posters = {
  soul: "soul",
  jazz: "jazz",
  "indie rock": "indie-rock",
  ambient: "ambient",
  classical: "classical",
  electronic: "electronic",
};
export function genreArtwork(name) {
  const key = String(name || "")
    .trim()
    .toLowerCase();
  const slug = Object.hasOwn(posters, key) ? posters[key] : null;
  return slug ? `/artwork/genres/${slug}.webp` : null;
}
export function genreBackground(name) {
  const hue = [
    ...String(name || "")
      .trim()
      .toLowerCase(),
  ].reduce((hash, letter) => (hash * 31 + letter.codePointAt(0)) % 360, 0);
  return `radial-gradient(ellipse at 80% 20%, hsl(${hue} 55% 44%), transparent 65%), linear-gradient(135deg, hsl(${(hue + 45) % 360} 40% 28%), #17171c)`;
}
