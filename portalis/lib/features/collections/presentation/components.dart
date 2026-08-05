/// Collection feature widgets shared by its desktop and compact layouts.
///
/// These exports provide the feature boundary now; their implementation files
/// remain under `ui/` temporarily so this refactor does not combine a path
/// migration with behavioural changes.
library;

export '../../../ui/collection_views.dart';
export '../../../ui/media.dart';
export '../../../ui/omnibar.dart';
export '../../../ui/welcome.dart';
export 'home_sections.dart';
