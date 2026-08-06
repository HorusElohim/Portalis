import 'package:flutter/foundation.dart';

/// Whether the current picker contract supplies a stable filesystem path that
/// Rust can seed without staging another copy. Mobile picker paths may point
/// at plugin cache files, so they are intentionally rejected until the native
/// content-location adapter is available.
bool get supportsDirectPathSources =>
    defaultTargetPlatform != TargetPlatform.android &&
    defaultTargetPlatform != TargetPlatform.iOS;

const directPathSourcesUnavailableMessage =
    'Direct mobile media sharing is not ready yet. Portalis will not copy a '
    'gallery asset; use a computer or wait for native Gallery storage.';
