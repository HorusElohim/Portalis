/// The Portalis UI kit.
///
/// One import (`import '../ui/ui.dart';`) for every shared building block, so
/// screens contain screen logic rather than re-declaring a card, a formatter,
/// or a confirmation dialog each time. This replaced `widgets/common.dart`,
/// which had become a single 850-line file, plus five private copies of
/// `_formatBytes`, two of `_Section`, and two of `_InfoRow` scattered across
/// screens — some of which had already drifted apart in precision and
/// spacing.
///
/// Layout of the kit:
///
/// - [formatters] — bytes, rates, limits, pluralisation
/// - [primitives] — page frame, card surface, section label, status badge
/// - [controls]   — primary action, pill button, filter chips, back button
/// - [rows]       — settings/detail rows, switches, banners
/// - [indicators] — live dot, pulse rings, perimeter progress
/// - [identity]   — avatars
/// - [media]      — thumbnails and placeholders
/// - [collection_views] — the shared collection row
///
/// Design tokens live in `theme.dart` rather than here, since Rust-facing
/// code and tests reference them too.
library;

export 'ambient_background.dart';
export 'collection_views.dart';
export 'controls.dart';
export 'dialogs.dart';
export 'formatters.dart';
export 'home_button.dart';
export 'identity.dart';
export 'indicators.dart';
export 'media.dart';
export 'primitives.dart';
export 'rows.dart';
export 'toast.dart';
