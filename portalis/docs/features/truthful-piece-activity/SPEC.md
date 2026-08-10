# Truthful piece activity

Status: **implemented for Portalis 1.0.9 / backend 0.1.7**

## Intent

Make a multi-file torrent look like the work the swarm is actually doing.
Portalis may visualize verified byte ranges and peer assignments reported by
the engine. It must never spread progress across media, animate synthetic
workers, or infer that a partial file is playable from a percentage.

## User experience

Each incomplete media tile keeps its existing verified percentage and gains a
piece-activity perimeter:

- verified ranges use the collection's progress colour;
- a piece currently assigned to a connected peer pulses at its real relative
  position in the file;
- missing ranges remain the ordinary border;
- connected peers with no current piece assignment remain collection-level
  peers and are not attached to a media item.

The enlarged media preview uses the same perimeter. It continues to show a
placeholder until the backend exposes a complete path. Early playback is a
separate feature: it requires a streaming endpoint and real buffered ranges,
not a completion threshold.

Several media may light up at once when the engine really has pieces in flight
across those files. If the engine works through one file at a time, the UI
shows one file at a time.

## Source-of-truth contract

The vendored librqbit adapter adds one snapshot to `TorrentStats`:

```text
piece length
piece count
packed verified-piece bitmap
in-flight assignments: piece index + peer address
```

The Rust backend intersects torrent-global pieces with each file's byte range
and projects compact, file-relative runs:

```text
MediaPieceRun
  offsetBytes
  lengthBytes
  state: verified | downloading
  peers: [] | [ip:port, ...]
```

Missing bytes are implicit. Adjacent verified intersections are merged, so a
normally sequential download crosses the bridge as one run rather than one DTO
per piece. Downloading runs remain exact because they carry their real peer
assignments. A piece that crosses a file boundary is intersected with both
files; neither file receives bytes outside its own range.

During startup verification the activity list is empty, matching the existing
rule that scan progress is not download progress. Paused torrents retain
verified runs but have no in-flight assignments.

## Boundaries

- librqbit owns piece truth and peer-to-piece assignment.
- The Rust backend owns torrent-to-file byte-range projection and compaction.
- Flutter maps the bridge DTO into immutable media-domain values.
- Presentation paints only those values. It does not simulate scheduling.
- Peer addresses remain anonymous network observations. They are not matched
  to collaborators or treated as durable identity.

## Performance constraints

- Verified adjacent pieces are merged before crossing FFI.
- Missing ranges do not cross FFI.
- Collection polling cadence does not increase.
- Repaints are scoped to media whose activity fingerprint changed.
- The UI uses reduced-motion-safe emphasis; motion is decorative, while
  position and colour continue to carry the state.

## Acceptance criteria

1. A verified out-of-order piece appears at its actual relative file position.
2. Two peers assigned to pieces in different files activate both media.
3. Sequential engine activity does not activate unrelated media.
4. A cross-file piece contributes only its intersecting bytes to each file.
5. A failed or stolen assignment disappears on the next real snapshot.
6. Startup checking never appears as downloaded or in-flight activity.
7. Completed, pending, and zero-length media retain their current behavior.
8. Rust projection tests cover sequential, sparse, cross-file, and in-flight
   mappings; Flutter tests cover mapper and painter/domain behavior.

## Out of scope

- Changing librqbit's piece scheduler.
- Fabricating a balanced worker distribution.
- Naming torrent peers or correlating them with collection collaborators.
- Opening incomplete files or claiming that they are playable.
- Historical peer trails after an assignment is no longer active.
