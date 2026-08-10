import 'dart:math' as math;

import 'test_support.dart';

import 'package:portalis/features/collections/presentation/collection_commands.dart';
import 'package:portalis/features/collections/presentation/collection_overview.dart';
import 'package:portalis/features/collections/presentation/collection_views.dart';

void main() {
  testWidgets('opening a transfer replaces row facts instead of repeating them',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(1600, 1200));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final collection = buildCollection(
      name: 'Tears of Steel',
      kind: CollectionKind.shared,
      state: 'downloading',
      collaborators: const [
        Collaborator(deviceId: 'admin', name: 'Admin', isAdmin: true),
      ],
      media: [
        for (var index = 0; index < 10; index++)
          MediaItem(label: 'part-$index.mkv', infoHash: 'aa'),
      ],
      totalBytes: 571000000,
      downloadedBytes: 570000000,
      downloadMbps: 27.7,
      livePeers: 8,
      etaSecs: 1,
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 1500,
            child: CollectionRow(
              collection: collection,
              selected: true,
              onTap: () {},
              detail: (level, inlineHeader, inlineStatus) => CollectionOverview(
                collection: collection,
                busy: false,
                showTitle: false,
                showCommands: true,
                inlineHeader: inlineHeader,
                inlineStatus: inlineStatus,
                level: level,
                onCommand: (_) {},
                onInvite: () {},
                onAddMedia: () {},
                onFetch: () {},
              ),
            ),
          ),
        ),
      ),
    );

    expect(find.text('10 items · 1s left'), findsOneWidget);
    expect(find.text('99%'), findsOneWidget);

    await tester.tap(find.text('Tears of Steel'));
    await tester.pump();

    expect(find.text('10 items · 1s left'), findsNothing);
    expect(find.text('10 items · 1 admin'), findsOneWidget);
    expect(find.text('DOWNLOADING'), findsOneWidget);
    expect(find.text('99%'), findsOneWidget);
    expect(find.text('PEERS'), findsOneWidget);
    expect(find.text('8'), findsOneWidget);
    expect(find.text('REMAINING'), findsOneWidget);
    expect(find.text('1s'), findsOneWidget);
    expect(find.textContaining('Shared collection - 10 items'), findsNothing);
    final timelineTop = tester.getTopLeft(find.text('START')).dy;
    final progressBarTop =
        tester.getTopLeft(find.byKey(const Key('transferProgressBar'))).dy;
    for (final finder in [
      find.text('Tears of Steel'),
      find.text('99%'),
      find.text('TRANSFER SPEED'),
      find.byKey(const Key('collectionCommandrestart')),
    ]) {
      expect(tester.getTopLeft(finder).dy, lessThan(timelineTop));
    }
    expect(progressBarTop, greaterThan(timelineTop));
    final dockActions = [
      for (final command in CollectionCommand.values)
        find.byKey(Key('collectionCommand${command.name}')),
      find.byKey(const Key('collectionInvite')),
      find.byKey(const Key('collectionAddMedia')),
    ];
    final dockCenters = [
      for (final finder in dockActions) tester.getCenter(finder).dy,
    ];
    expect(find.text('Restart'), findsNothing);
    expect(
      dockCenters.reduce(math.max) - dockCenters.reduce(math.min),
      lessThan(1),
    );
    expect(tester.takeException(), isNull);
  });
}
