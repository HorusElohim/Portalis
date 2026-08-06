# Canonical media storage

Portalis keeps exactly one physical copy of content it owns. A collection
does not have a separate “torrent copy”, “preview copy”, and “gallery copy”.
The torrent engine, media viewer, editor, and export flow all refer to one
canonical content location.

## Ownership rule

```text
collection item -> canonical location -> read / write / seed / preview
```

The canonical location must support stable random reads for as long as a
collection seeds. A completed torrent is never moved to another location as a
post-processing step: moving invalidates the engine's storage location and
encourages duplicate data.

## Current filesystem implementation

Desktop and the user-visible Portalis folder on iOS use filesystem paths.
When creating a share from one filesystem file, Rust seeds the original path
directly. For several independent files it builds the required common torrent
layout with hard links. A hard link gives the torrent a stable collection name
while retaining one physical file allocation. Portalis deliberately refuses
to copy when the filesystem cannot make that link (for example, across
volumes).

Changing a shared source changes the same canonical bytes. The next torrent
verification correctly detects changed pieces; users should not edit source
files while they are being seeded.

## Target locations

| Platform | Canonical location | Native-library visibility |
| --- | --- | --- |
| Desktop | Chosen filesystem folder | The operating-system filesystem |
| Android | Persisted MediaStore `content://` item | Gallery sees the same item |
| iOS | `On My iPhone/Portalis` Files location | Portalis media library/editor |

Apple Photos is an external export target, not canonical storage: Photos
imports its own managed asset. Exporting there is an explicit user action and
may create a second Apple-managed copy. It must never happen automatically on
completion while the collection is seeding.

## Android adapter

Android will add a native `ContentLocation` bridge with a persisted URI grant.
Its implementation owns `ContentResolver` and opens positioned reads/writes
through a `ParcelFileDescriptor`. Rust will expose the adapter to librqbit as
a `StorageFactory`/`TorrentStorage`, so piece traffic uses the MediaStore item
directly rather than staging it in Dart or the app cache.

Portalis persistence, not librqbit's filesystem-only JSON session store, will
save the collection id, torrent metadata handle, URI, display name, length,
and read/write/seed capabilities. On restart the native adapter reopens the
persisted URI before restoring the torrent.

Creating a torrent from a URI needs a provider-backed metadata builder because
librqbit's current `create_torrent` accepts filesystem paths only. That
builder hashes the provider reader in bounded chunks and emits standard
bencoded metadata; it never serialises file bytes across Flutter-Rust Bridge.

## iOS boundary

Portalis' Files location is the honest one-copy iOS implementation: it gives
Rust a stable path and remains accessible to users. Existing Photos assets
may be exported into Portalis only with clear copy semantics. A future
PHAsset-backed reader is possible, but cannot promise local availability,
stable random access, or a no-copy Photos import when iCloud manages the
asset, so it is not part of the canonical storage contract.
