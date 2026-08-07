# Gallery-linked media

Portalis must not import a phone-gallery selection into its sandbox merely to
turn it into a path. That creates a second full-size copy and makes the app's
storage scale with a user's library.

## Source contract

Gallery media is represented as a durable native reference, never an `XFile`
cache path:

- Android: a persisted `content://` URI from the system Photo Picker or Storage
  Access Framework, retained with `takePersistableUriPermission`.
- iOS: a `PHAsset.localIdentifier` selected through `PHPickerViewController`.
- Desktop and iOS Files: the existing path/security-scoped bookmark source.

Each descriptor carries its display name, byte length, platform kind, and
stable reference. It is persisted with an active import and with the torrent
that it seeds.

## Torrent integration

`librqbit`'s built-in torrent creator only accepts filesystem paths. The
gallery implementation therefore needs two native pieces:

1. hash descriptors through a random-access reader to construct metainfo;
2. attach a custom `librqbit::TorrentStorage` that serves `pread_exact` from
   Android MediaStore or iOS PhotoKit instead of an output directory.

This keeps the original gallery item as the only source copy. Receiving peers
still download their own copies, as required by BitTorrent.

## Availability checks

Before the native gallery adapter lands, filesystem/security-scoped import
sources are validated when an import starts, immediately on app restart, and
every ten seconds while an import is active. If the original source disappears
or becomes unreadable, the import is cancelled and recorded as failed instead
of silently using a cache copy.

The completed gallery adapter uses the same cadence for persisted MediaStore
URI and PhotoKit identifier checks. A lost reference pauses seeding and is
shown as a source-unavailable state; it never triggers a fallback import.
