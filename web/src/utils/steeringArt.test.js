import test from "node:test";
import assert from "node:assert/strict";
import {
  TILE_PALETTE,
  mixTitle,
  referenceKey,
  tileColor,
  withAlpha,
} from "./steeringArt.js";

test("tile colours are deterministic and from the palette", () => {
  assert.equal(tileColor("audioset:Jazz"), tileColor("audioset:Jazz"));
  assert.ok(TILE_PALETTE.includes(tileColor("genre:shoegaze")));
  const colours = new Set(
    ["a", "b", "c", "d", "e", "f", "g", "h"].map((id) => tileColor(id)),
  );
  assert.ok(colours.size > 1);
});

test("withAlpha converts hex colours", () => {
  assert.equal(withAlpha("#1e3264", 0.5), "rgba(30, 50, 100, 0.5)");
});

test("mix titles summarise several references", () => {
  assert.equal(mixTitle([]), "");
  assert.equal(mixTitle([{ entity_id: "x", label: "Jazz" }]), "Jazz");
  assert.equal(
    mixTitle([
      { entity_id: "a", label: "Kind of Blue" },
      { entity_id: "b", label: "Coltrane" },
      { entity_id: "c" },
    ]),
    "Kind of Blue + 2 more",
  );
  assert.equal(
    referenceKey({ entity_type: "album", entity_id: "1" }),
    "album:1",
  );
});
