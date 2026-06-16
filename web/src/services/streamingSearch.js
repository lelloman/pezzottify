/**
 * Streaming Search Service
 *
 * Consumes the SSE streaming search endpoint and provides progressive results.
 */

/**
 * Section types returned by the streaming search API.
 * Each section is tagged with a "section" field in snake_case.
 */
export const SectionType = {
  PRIMARY_ARTIST: "primary_artist",
  PRIMARY_ALBUM: "primary_album",
  PRIMARY_TRACK: "primary_track",
  POPULAR_BY: "popular_by",
  ALBUMS_BY: "albums_by",
  TRACKS_FROM: "tracks_from",
  RELATED_ARTISTS: "related_artists",
  MORE_RESULTS: "more_results",
  RESULTS: "results",
  DONE: "done",
};

function buildStreamingSearchUrl(query, options = {}) {
  const encodedQuery = encodeURIComponent(query);
  const searchMode = options.searchMode || "expanded";
  let url = `/v1/content/search/stream?q=${encodedQuery}&search_mode=${encodeURIComponent(searchMode)}`;
  if (options.excludeUnavailable) {
    url += "&exclude_unavailable=true";
  }
  return url;
}

/**
 * Executes a streaming search and calls the callback for each section received.
 *
 * @param {string} query - The search query
 * @param {function} onSection - Callback called with each section object
 * @param {function} onError - Callback called on error
 * @param {function} onComplete - Callback called when stream completes
 * @param {Object} options - Optional search options
 * @param {boolean} options.excludeUnavailable - If true, exclude unavailable content
 * @returns {function} Abort function to cancel the stream
 */
export function streamingSearch(
  query,
  onSection,
  onError,
  onComplete,
  options = {},
) {
  const controller = new AbortController();

  fetch(buildStreamingSearchUrl(query, options), {
    method: "GET",
    headers: {
      Accept: "text/event-stream",
    },
    signal: controller.signal,
  })
    .then(async (response) => {
      if (!response.ok) {
        throw new Error(`Search failed: ${response.status}`);
      }

      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";

      while (true) {
        const { done, value } = await reader.read();

        if (done) {
          break;
        }

        buffer += decoder.decode(value, { stream: true });

        // Process complete SSE events from buffer
        const lines = buffer.split("\n");
        buffer = lines.pop(); // Keep incomplete line in buffer

        for (const line of lines) {
          if (line.startsWith("data:")) {
            const jsonStr = line.slice(5).trim();
            if (jsonStr) {
              try {
                const section = JSON.parse(jsonStr);
                onSection(section);

                // Check if this is the final section
                if (section.section === SectionType.DONE) {
                  if (onComplete) onComplete(section);
                }
              } catch (e) {
                console.error("Failed to parse SSE data:", e, jsonStr);
              }
            }
          }
        }
      }
    })
    .catch((error) => {
      if (error.name !== "AbortError") {
        console.error("Streaming search error:", error);
        if (onError) onError(error);
      }
    });

  // Return abort function
  return () => controller.abort();
}

/**
 * Executes a streaming search and resolves with all emitted sections.
 *
 * @param {string} query - The search query
 * @param {Object} options - Optional search options
 * @param {AbortSignal} signal - Optional abort signal
 * @returns {Promise<Array>} Streamed sections
 */
export async function fetchStreamingSearchSections(
  query,
  options = {},
  signal,
) {
  const response = await fetch(buildStreamingSearchUrl(query, options), {
    method: "GET",
    headers: {
      Accept: "text/event-stream",
    },
    signal,
  });

  if (!response.ok) {
    throw new Error(`Search failed: ${response.status}`);
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const sections = [];
  let buffer = "";

  const processLine = (line) => {
    if (!line.startsWith("data:")) {
      return;
    }

    const jsonStr = line.slice(5).trim();
    if (!jsonStr) {
      return;
    }

    sections.push(JSON.parse(jsonStr));
  };

  while (true) {
    const { done, value } = await reader.read();

    if (done) {
      break;
    }

    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split("\n");
    buffer = lines.pop();

    for (const line of lines) {
      processLine(line);
    }
  }

  if (buffer) {
    processLine(buffer);
  }

  return sections;
}

/**
 * Converts streaming search sections to a flat array of results
 * compatible with the organic search format.
 *
 * @param {Array} sections - Array of section objects
 * @returns {Array} Flat array of search results
 */
export function sectionsToResults(sections) {
  const results = [];
  const seenIds = new Set();

  for (const section of sections) {
    let items = [];

    switch (section.section) {
      case SectionType.PRIMARY_ARTIST:
      case SectionType.PRIMARY_ALBUM:
      case SectionType.PRIMARY_TRACK:
        if (section.item) {
          items = [section.item];
        }
        break;

      case SectionType.MORE_RESULTS:
      case SectionType.RESULTS:
        items = section.items || [];
        break;

      // Enrichment sections contain summary objects, not full results
      // We skip them for the flat results view
      case SectionType.POPULAR_BY:
      case SectionType.ALBUMS_BY:
      case SectionType.TRACKS_FROM:
      case SectionType.RELATED_ARTISTS:
        // These are enrichment sections with different data structure
        // Skip for now - they'll be handled by the streaming UI
        break;

      case SectionType.DONE:
        // Terminal section, nothing to add
        break;
    }

    // Add items, avoiding duplicates
    for (const item of items) {
      const id = item.id;
      if (id && !seenIds.has(id)) {
        seenIds.add(id);
        results.push(item);
      }
    }
  }

  return results;
}
