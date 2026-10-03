// Plain-language "Along the way" controls for the steering page, mapped onto the
// continuation settings stored in the queue (`gravity.knobs`).

// Server-side defaults for continuation (see docs/radio-playback.md).
export const JOURNEY_DEFAULTS = Object.freeze({
  follow: 0.2,
  variety: 0.3,
});

export const RESET_JOURNEY = Object.freeze({
  recency_weight: null,
  criteria: null,
  diversity: null,
  randomness: null,
  mode: null,
  away: [],
});

const finite = (value) =>
  value == null || !Number.isFinite(Number(value)) ? null : Number(value);

// Every write from the steering page clears the radio "mode": exploring on purpose
// works against reaching a destination.
export const journeyPartial = (partial) => ({ ...partial, mode: null });

export const followPartial = (value) =>
  journeyPartial({ recency_weight: Number(value) });

// "Variety" drives both repeat avoidance and ranking noise with one value.
export const varietyPartial = (value) =>
  journeyPartial({ diversity: Number(value), randomness: Number(value) });

export const displayedFollow = (knobs, fallback = JOURNEY_DEFAULTS.follow) =>
  finite(knobs?.recency_weight) ?? fallback;

export const displayedVariety = (knobs, fallback = JOURNEY_DEFAULTS.variety) => {
  const values = [finite(knobs?.diversity), finite(knobs?.randomness)];
  const set = values.filter((value) => value != null);
  if (!set.length) return fallback;
  if (set.length === 1) return (set[0] + fallback) / 2;
  return (set[0] + set[1]) / 2;
};

export const listenForLabel = (namespace, fallback) => {
  if (namespace?.startsWith("musicfm.")) return "Overall sound";
  if (namespace?.startsWith("ast.audioset.")) return "Audio scene";
  if (namespace?.startsWith("ast.instruments.")) return "Instruments";
  return fallback || namespace;
};

export const hasCustomJourney = (knobs) =>
  Object.entries(knobs || {}).some(([key, value]) =>
    key === "away" ? value?.length : key !== "mode" && value != null,
  );
