/*
 * Copyright 2018 The Android Open Source Project
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * JavaScript adaptation of AndroidX Palette's ColorCutQuantizer and default
 * filter. Uses the dominant swatch, not the optional six target profiles.
 * Source: https://android.googlesource.com/platform/frameworks/support/+/androidx-main/palette/palette/src/main/java/androidx/palette/graphics/
 * Browser image decoding/resampling can differ from Android Bitmap decoding.
 */

export const PALETTE_FALLBACK = "#191414";
const components = (color) => [color >> 10, (color >> 5) & 31, color & 31];
const rgb = (color) => components(color).map((v) => v << 3);
const hex = (values) =>
  "#" + values.map((v) => v.toString(16).padStart(2, "0")).join("");

// Android's default filter excludes nearly black/white and low-saturation
// colors near the red I-line. Float32 operations match ColorUtils' HSL math.
function allowed(values) {
  const [r, g, b] = values.map((v) => Math.fround(v / 255));
  const max = Math.max(r, g, b),
    min = Math.min(r, g, b);
  const delta = Math.fround(max - min);
  const light = Math.fround(Math.fround(max + min) / 2);
  let hue = 0,
    saturation = 0;
  if (delta !== 0) {
    hue =
      max === r
        ? Math.fround(Math.fround(g - b) / delta) % 6
        : max === g
          ? Math.fround(Math.fround(Math.fround(b - r) / delta) + 2)
          : Math.fround(Math.fround(Math.fround(r - g) / delta) + 4);
    saturation = Math.fround(
      delta /
        Math.fround(1 - Math.abs(Math.fround(Math.fround(2 * light) - 1))),
    );
  }
  hue = Math.fround(hue * 60) % 360;
  if (hue < 0) hue += 360;
  return (
    light > Math.fround(0.05) &&
    light < Math.fround(0.95) &&
    !(hue >= 10 && hue <= 37 && saturation <= Math.fround(0.82))
  );
}

// Preserve Java PriorityQueue's tie handling and heap iteration order:
// sorting an array of boxes by volume changes results when volumes tie.
class BoxQueue {
  items = [];
  push(box) {
    let i = this.items.length;
    this.items.push(box);
    while (i > 0) {
      const parent = (i - 1) >> 1;
      if (box.volume <= this.items[parent].volume) break;
      this.items[i] = this.items[parent];
      i = parent;
    }
    this.items[i] = box;
  }
  pop() {
    const result = this.items[0],
      last = this.items.pop();
    if (!this.items.length) return result;
    let i = 0;
    while (i < this.items.length >> 1) {
      let child = i * 2 + 1;
      if (
        child + 1 < this.items.length &&
        this.items[child + 1].volume > this.items[child].volume
      )
        child++;
      if (last.volume >= this.items[child].volume) break;
      this.items[i] = this.items[child];
      i = child;
    }
    this.items[i] = last;
    return result;
  }
}

/** AndroidX default quantization of RGBA pixels; ignores transparent pixels. */
export function quantizePalette(pixels) {
  const histogram = new Uint32Array(32768);
  for (let i = 0; i < pixels.length; i += 4) {
    if (pixels[i + 3] === 0) continue;
    histogram[
      ((pixels[i] >> 3) << 10) |
        ((pixels[i + 1] >> 3) << 5) |
        (pixels[i + 2] >> 3)
    ]++;
  }
  const colors = [];
  for (let c = 0; c < histogram.length; c++) {
    if (histogram[c] && allowed(rgb(c))) colors.push(c);
  }
  if (colors.length <= 16) {
    return colors.map((c) => ({
      color: hex(rgb(c)),
      population: histogram[c],
    }));
  }
  function box(lower, upper) {
    const min = [31, 31, 31],
      max = [0, 0, 0];
    let population = 0;
    for (let i = lower; i <= upper; i++) {
      population += histogram[colors[i]];
      components(colors[i]).forEach((v, axis) => {
        min[axis] = Math.min(min[axis], v);
        max[axis] = Math.max(max[axis], v);
      });
    }
    const ranges = max.map((v, i) => v - min[i]);
    return {
      lower,
      upper,
      population,
      ranges,
      volume: ranges.reduce((v, r) => v * (r + 1), 1),
    };
  }
  const queue = new BoxQueue();
  queue.push(box(0, colors.length - 1));
  while (queue.items.length < 16) {
    const current = queue.pop();
    if (current.lower === current.upper) break;
    const axis = current.ranges.indexOf(Math.max(...current.ranges));
    const packed = (c) => {
      const [r, g, b] = components(c);
      return axis === 0
        ? c
        : axis === 1
          ? (g << 10) | (r << 5) | b
          : (b << 10) | (g << 5) | r;
    };
    const sorted = colors
      .slice(current.lower, current.upper + 1)
      .sort((a, b) => packed(a) - packed(b));
    colors.splice(current.lower, sorted.length, ...sorted);
    let count = 0,
      split = current.lower;
    for (let i = current.lower; i <= current.upper; i++) {
      count += histogram[colors[i]];
      if (count >= Math.floor(current.population / 2)) {
        split = Math.min(current.upper - 1, i);
        break;
      }
    }
    queue.push(box(split + 1, current.upper));
    queue.push(box(current.lower, split));
  }
  return queue.items.flatMap((item) => {
    const sums = [0, 0, 0];
    for (let i = item.lower; i <= item.upper; i++) {
      components(colors[i]).forEach((v, axis) => {
        sums[axis] += v * histogram[colors[i]];
      });
    }
    const mean = sums.map(
      (v) => Math.round(Math.fround(v / item.population)) << 3,
    );
    return allowed(mean)
      ? [{ color: hex(mean), population: item.population }]
      : [];
  });
}

export function dominantPaletteColor(pixels) {
  return (
    quantizePalette(pixels).reduce(
      (best, swatch) =>
        !best || swatch.population > best.population ? swatch : best,
      null,
    )?.color ?? PALETTE_FALLBACK
  );
}

const cache = new Map();
/** Reuse the decoded artwork: no second fetch, bounded to Palette's 112² area. */
export function artworkPaletteColor(image, source) {
  if (cache.has(source)) return cache.get(source);
  const ratio = Math.min(
    1,
    Math.sqrt(12544 / (image.naturalWidth * image.naturalHeight)),
  );
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.ceil(image.naturalWidth * ratio));
  canvas.height = Math.max(1, Math.ceil(image.naturalHeight * ratio));
  const context = canvas.getContext("2d", { willReadFrequently: true });
  // Palette's Bitmap.createScaledBitmap uses filter=false.
  context.imageSmoothingEnabled = false;
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  const color = dominantPaletteColor(
    context.getImageData(0, 0, canvas.width, canvas.height).data,
  );
  if (cache.size >= 256) cache.delete(cache.keys().next().value);
  cache.set(source, color);
  return color;
}
