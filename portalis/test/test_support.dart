import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/app/app_controllers.dart';
import 'package:portalis/features/collections/domain/collection.dart';
import 'package:portalis/features/media/domain/media_item.dart';
import 'package:portalis/features/settings/domain/engine_settings.dart';
import 'package:portalis/main.dart';
import 'package:portalis/services/navigation.dart';

export 'package:flutter/material.dart';
export 'package:flutter_test/flutter_test.dart';
export 'package:portalis/app/app_controllers.dart';
export 'package:portalis/design/design.dart';
export 'package:portalis/features/collections/domain/collection.dart';
export 'package:portalis/features/collections/domain/paste.dart';
export 'package:portalis/features/collections/presentation/collection_detail.dart';
export 'package:portalis/features/collections/presentation/collection_join.dart';
export 'package:portalis/features/collections/presentation/collection_share.dart';
export 'package:portalis/features/collections/presentation/command_bar.dart';
export 'package:portalis/features/media/domain/media_item.dart';
export 'package:portalis/features/media/presentation/media_viewer_screen.dart';
export 'package:portalis/features/settings/domain/engine_settings.dart';
export 'package:portalis/main.dart';
export 'package:portalis/screens/people.dart';
export 'package:portalis/screens/root_shell.dart';
export 'package:portalis/screens/settings.dart';
export 'package:portalis/services/navigation.dart';
export 'package:portalis/theme.dart';

const phoneSize = Size(390, 844);
const desktopSize = Size(1280, 800);

Collection buildCollection({
  String id = 'c1',
  String name = 'Iceland trip',
  CollectionKind kind = CollectionKind.shared,
  double downloadMbps = 0,
  double uploadMbps = 0,
  String state = 'seeding',
  int livePeers = 0,
  int pendingMedia = 0,
  List<Collaborator> collaborators = const [],
  List<MediaItem> media = const [],
  int totalBytes = 0,
  int downloadedBytes = 0,
  int? etaSecs,
}) =>
    Collection(
      id: id,
      name: name,
      kind: kind,
      collaborators: collaborators,
      media: media,
      progress: totalBytes == 0 ? 0 : downloadedBytes / totalBytes,
      totalBytes: totalBytes,
      downloadedBytes: downloadedBytes,
      downloadMbps: downloadMbps,
      uploadMbps: uploadMbps,
      livePeers: livePeers,
      pendingMedia: pendingMedia,
      etaSecs: etaSecs,
      state: state,
    );

EngineSettings buildEngineSettings() => const EngineSettings(
      listenPortStart: 6881,
      listenPortEnd: 6999,
      enableUpnpPortForwarding: true,
      disableDht: false,
      disableDhtPersistence: false,
      persistSession: true,
      fastresume: true,
      trackers: [],
    );

Future<void> pumpApp(
  WidgetTester tester, {
  Size size = phoneSize,
  List<Collection> collections = const [],
  String? error,
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(const MyApp());
  await tester.pump();
  AppControllers.collections.debugSeed(collections, error: error);
  await tester.pump();
}

Future<void> pumpTransition(WidgetTester tester) async {
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 300));
}

String inviteCode(String name) {
  final plain = '${'a' * 64}:$name';
  return plain.codeUnits
      .map((c) => c.toRadixString(16).padLeft(2, '0'))
      .join();
}

void resetTestState() {
  AppControllers.collections.debugSeed([]);
  AppNavigation.tab.value = 0;
  AppNavigation.depth.value = 0;
}
