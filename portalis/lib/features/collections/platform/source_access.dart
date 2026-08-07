import 'package:flutter/foundation.dart';

/// Whether generic Flutter pickers supply a stable filesystem path that Rust
/// can seed without staging another copy. Mobile plugin paths may point at
/// cache files, so they are deliberately not used there.
bool get supportsDirectPathSources =>
    defaultTargetPlatform != TargetPlatform.android &&
    defaultTargetPlatform != TargetPlatform.iOS;

/// Gallery selection stays disabled until the native readers can seed the
/// persistent platform reference directly. Enabling an `image_picker` cache
/// path here would silently put a second copy in Portalis' sandbox.
bool get supportsMobileGallerySources =>
    defaultTargetPlatform == TargetPlatform.iOS;

/// iOS uses its native Files picker, which returns a security-scoped location.
bool get supportsNativeFilesSources =>
    defaultTargetPlatform == TargetPlatform.iOS;

bool get supportsNoCopySources =>
    supportsDirectPathSources || supportsNativeFilesSources;

bool get supportsMediaSources =>
    supportsNoCopySources || supportsMobileGallerySources;

String get noCopySourceUnavailableMessage =>
    defaultTargetPlatform == TargetPlatform.android
        ? 'Android gallery linking is being added without copying media into '
            'Portalis. Choose a supported source for now.'
        : 'Choose files from Files. Photos assets stay in Apple Photos and '
            'cannot be seeded without making a second copy.';
