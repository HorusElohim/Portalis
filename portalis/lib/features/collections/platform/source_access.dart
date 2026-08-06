import 'package:flutter/foundation.dart';

/// Whether generic Flutter pickers supply a stable filesystem path that Rust
/// can seed without staging another copy. Mobile plugin paths may point at
/// cache files, so they are deliberately not used there.
bool get supportsDirectPathSources =>
    defaultTargetPlatform != TargetPlatform.android &&
    defaultTargetPlatform != TargetPlatform.iOS;

/// iOS uses its native Files picker, which returns a security-scoped location.
/// Android remains unavailable until the Rust MediaStore adapter exists.
bool get supportsNativeFilesSources =>
    defaultTargetPlatform == TargetPlatform.iOS;

bool get supportsNoCopySources =>
    supportsDirectPathSources || supportsNativeFilesSources;

String get noCopySourceUnavailableMessage =>
    defaultTargetPlatform == TargetPlatform.android
        ? 'Portalis will not copy a gallery or picker cache file. Native '
            'MediaStore sharing is not ready yet.'
        : 'Choose files from Files. Photos assets stay in Apple Photos and '
            'cannot be seeded without making a second copy.';
