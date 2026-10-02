// Smart continuation "gravity": where the queue's centre of gravity is anchored (Source),
// where it is being steered (Destination) and which queue tracks were appended automatically.
// Like radio continuation, it belongs to the playback session and travels inside the queue
// context as plain JSON, so everything here must stay JSON-safe and side-effect free.

export const AUTO_IDS_CAP = 1000;
export const SOURCE_SAMPLE_CAP = 200;
export const RECENT_COUNT = 5;
export const LEGACY_CONTEXT_COUNT = 10;
export const DEFAULT_STEPS_TOTAL = 20;

export const GRAVITY_DEFAULTS = Object.freeze({
  v: 1,
  source: Object.freeze({ kind: "queue" }),
  auto_track_ids: Object.freeze([]),
  destination: null,
  steps_total: DEFAULT_STEPS_TOTAL,
  steps_done: 0,
  knobs: Object.freeze({
    recency_weight: null,
    criteria: null,
    diversity: null,
    randomness: null,
    mode: null,
    away: Object.freeze([]),
  }),
  last_diagnostics: null,
});

const unique = (ids) => [...new Set(ids)];

export function create(overrides = {}) {
  return {
    ...GRAVITY_DEFAULTS,
    source: { ...GRAVITY_DEFAULTS.source },
    auto_track_ids: [],
    knobs: { ...GRAVITY_DEFAULTS.knobs, away: [] },
    ...overrides,
  };
}

// Fill missing keys on a persisted or remote object without losing what it carries.
export function normalize(gravity) {
  if (!gravity || typeof gravity !== "object") return create();
  return {
    ...create(),
    ...gravity,
    source: gravity.source?.kind ? gravity.source : { kind: "queue" },
    auto_track_ids: Array.isArray(gravity.auto_track_ids)
      ? gravity.auto_track_ids
      : [],
    knobs: { ...GRAVITY_DEFAULTS.knobs, away: [], ...(gravity.knobs || {}) },
  };
}

export function isAuto(gravity, trackId) {
  return Boolean(gravity?.auto_track_ids?.includes(trackId));
}

export function progress(gravity) {
  if (!gravity?.destination) return 0;
  const total = Math.max(1, gravity.steps_total || 1);
  return Math.min(1, (gravity.steps_done || 0) / total);
}

export function arrive(gravity) {
  if (!gravity.destination) return gravity;
  return {
    ...gravity,
    source: {
      kind: "references",
      references: [{ ...gravity.destination, weight: 1 }],
    },
    destination: null,
    steps_done: 0,
  };
}

export function appendAuto(gravity, trackIds) {
  const auto = unique([...gravity.auto_track_ids, ...trackIds]);
  const next = {
    ...gravity,
    auto_track_ids:
      auto.length > AUTO_IDS_CAP
        ? auto.slice(auto.length - AUTO_IDS_CAP)
        : auto,
  };
  if (!gravity.destination) return next;
  next.steps_done = (gravity.steps_done || 0) + unique(trackIds).length;
  return next.steps_done >= next.steps_total ? arrive(next) : next;
}

export function markUserAdded(gravity, trackIds) {
  const added = new Set(trackIds);
  if (!gravity.auto_track_ids.some((id) => added.has(id))) return gravity;
  return {
    ...gravity,
    auto_track_ids: gravity.auto_track_ids.filter((id) => !added.has(id)),
  };
}

// Removing a suggestion keeps it excluded on purpose, so this is a no-op hook.
export function noteRemoved(gravity) {
  return gravity;
}

export function setDestination(gravity, reference, stepsTotal) {
  return {
    ...gravity,
    destination: reference
      ? {
          entity_type: reference.entity_type,
          entity_id: reference.entity_id,
          label: reference.label ?? reference.entity_id,
        }
      : null,
    steps_done: 0,
    steps_total: Math.max(1, Math.floor(stepsTotal ?? gravity.steps_total)),
  };
}

export function setSource(gravity, source) {
  return {
    ...gravity,
    source:
      source?.kind === "references"
        ? {
            kind: "references",
            references: (source.references || []).map((reference) => ({
              ...reference,
              weight: reference.weight ?? 1,
            })),
          }
        : { kind: "queue" },
  };
}

export function setStepsTotal(gravity, stepsTotal) {
  const total = Math.max(1, Math.floor(stepsTotal));
  const next = { ...gravity, steps_total: total };
  return gravity.destination && (gravity.steps_done || 0) >= total
    ? arrive(next)
    : next;
}

export function setKnobs(gravity, partial) {
  return { ...gravity, knobs: { ...gravity.knobs, ...partial } };
}

export function applyDiagnostics(gravity, diagnostics, at = Date.now()) {
  return {
    ...gravity,
    last_diagnostics: diagnostics ? { ...diagnostics, at } : null,
  };
}

// Evenly strided sample that keeps order and always includes the first element,
// so a long playlist is represented end to end and the anchor is stable between calls.
export function sampleEvenly(list, max) {
  if (list.length <= max) return list;
  return Array.from(
    { length: max },
    (_, i) => list[Math.floor((i * list.length) / max)],
  );
}

export function buildContinuationRequest(
  gravity,
  tracksIds,
  currentIndex,
  count,
) {
  const g = normalize(gravity);
  const auto = new Set(g.auto_track_ids);
  const index = Number.isInteger(currentIndex)
    ? Math.min(currentIndex, tracksIds.length - 1)
    : tracksIds.length - 1;
  const request = {
    context_track_ids: tracksIds.slice(-LEGACY_CONTEXT_COUNT),
    recent_track_ids: tracksIds.slice(
      Math.max(0, index - (RECENT_COUNT - 1)),
      index + 1,
    ),
    exclude_track_ids: unique([...tracksIds, ...g.auto_track_ids]),
    count,
  };
  if (g.source.kind === "references") {
    request.source_references = g.source.references.map(
      ({ entity_type, entity_id, weight }) => ({
        entity_type,
        entity_id,
        ...(weight != null ? { weight } : {}),
      }),
    );
  } else {
    request.source_track_ids = sampleEvenly(
      unique(tracksIds.filter((id) => !auto.has(id))),
      SOURCE_SAMPLE_CAP,
    );
  }
  const { recency_weight, criteria, diversity, randomness, mode, away } =
    g.knobs;
  if (recency_weight != null) request.recency_weight = recency_weight;
  if (g.destination) {
    request.destination = {
      entity_type: g.destination.entity_type,
      entity_id: g.destination.entity_id,
    };
    request.progress = progress(g);
  }
  if (criteria?.length) request.criteria = criteria;
  if (diversity != null) request.diversity = diversity;
  if (randomness != null) request.randomness = randomness;
  if (mode != null) request.mode = mode;
  if (away?.length) {
    request.away = away.map(({ entity_type, entity_id, weight }) => ({
      entity_type,
      entity_id,
      ...(weight != null ? { weight } : {}),
    }));
  }
  return request;
}
