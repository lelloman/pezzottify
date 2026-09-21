# Pezzottify-player mark

`pezzottify-player.svg` is the canonical transparent mark on a 100 × 100 canvas.
It combines two rounded audio-level bars and a play triangle. The three flat
fills use LelloDesign's Android family: light `#b9ffd2`, medium `#3ddc84`, and
strong `#006c45`. Keep the geometry, spacing, and colors together when exporting.

The PNG is a preview on a white tile with a 22% corner radius; the tile is not
part of the canonical artwork. The design follows `lellodesign/icons/guidelines.md`
and was reviewed alongside its ScT, Talìa, and Crumbles examples at 16, 24, 32,
48, 100, and 192 pixels.

Android resources live in `android/player-app/src/main/res`:

- `ic_launcher_mark.xml` preserves the SVG coordinates for legacy launcher icons.
- `ic_launcher_foreground.xml` uniformly scales the mark by 0.8 and translates it
  by (14, 14) into a 108 dp adaptive canvas, keeping it within the safe region.
- `ic_launcher_monochrome.xml` uses the same geometry with opaque black fills so
  Android can tint all three pieces consistently for themed icons.
- Adaptive backgrounds and legacy rounded-square/circle backgrounds are white.

Pezzottify's streaming-app artwork is unchanged.
