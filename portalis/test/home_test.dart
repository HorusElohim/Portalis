import 'test_support.dart';

import 'package:portalis/features/collections/application/collections_controller.dart';
import 'package:portalis/features/collections/data/collections_repository.dart';
import 'package:portalis/features/collections/data/peer_history_store.dart';
import 'package:portalis/features/collections/data/transfer_history_store.dart';
import 'package:portalis/features/collections/domain/peer_observation.dart';
import 'package:portalis/features/collections/domain/transfer_history.dart';

void main() {
  tearDown(resetTestState);

group('how long is left', () {
    test('reads coarser the further out it is', () {
      // Seconds matter when there are seconds left and are noise when there
      // are hours.
      expect(formatEta(45), '45s');
      expect(formatEta(200), '3m 20s');
      expect(formatEta(8040), '2h 14m');
      // Past a day, a precise figure extrapolated from five seconds of
      // throughput would be false precision.
      expect(formatEta(90000), 'over a day');
    });

    test('does not round an incomplete transfer up to 100%', () {
      expect(formatProgressPercent(0.999), '99%');
      expect(formatProgressPercent(1), '100%');
    });

    testWidgets('a downloading collection says when it lands', (tester) async {
      await pumpApp(
        tester,
        collections: [
          buildCollection(
            state: 'downloading',
            totalBytes: 1000,
            downloadedBytes: 400,
            downloadMbps: 1,
            etaSecs: 8040,
          ),
        ],
      );

      // Twice: the live hero card states it, and so does the collection's
      // own row in the list right below.
      expect(find.textContaining('2h 14m left'), findsWidgets);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a stalled or finished collection claims nothing',
        (tester) async {
      // No rate means no basis for an estimate, and a number meaning "never"
      // is worse than no number.
      await pumpApp(
        tester,
        collections: [
          buildCollection(state: 'downloading', totalBytes: 1000, downloadedBytes: 400),
        ],
      );

      expect(find.textContaining('left'), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });



  group('home holds both the welcome and the list', () {
    testWidgets('shows the welcome when there is nothing yet', (tester) async {
      await pumpApp(tester);
      expect(find.textContaining('SEND ANYTHING'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('shows the list once you own something, not a second welcome',
        (tester) async {
      // Home doubles as the list now â€” the merge that gave desktop's pane a
      // search bar and filter chips gave mobile the list it used to keep on
      // a separate Collections tab. One destination for "what can I do" and
      // "what do I have", on both layouts, the same way desktop's pane
      // already worked before this merge.
      await pumpApp(tester,
          collections: [buildCollection(name: 'Iceland trip')],
          size: const Size(390, 1300));

      expect(find.text('Iceland trip'), findsOneWidget);
      expect(find.textContaining('SEND ANYTHING'), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });



  group('home', () {
    testWidgets('shows an active collection once in the library',
        (tester) async {
      await pumpApp(tester, collections: [
        buildCollection(
          name: 'Iceland trip',
          state: 'downloading',
          downloadMbps: 42.6,
          totalBytes: 1900000000,
          downloadedBytes: 1400000000,
          livePeers: 3,
        ),
      ]);

      expect(find.text('Iceland trip'), findsOneWidget);
      expect(find.text('73%'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('does not show an empty state when an idle collection exists',
        (tester) async {
      await pumpApp(tester, collections: [
        buildCollection(state: 'seeding', downloadMbps: 0, uploadMbps: 0),
      ]);

      expect(find.text('Iceland trip'), findsOneWidget);
      expect(find.textContaining('SEND ANYTHING'), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('filter sheet narrows the list by state', (tester) async {
      await pumpApp(tester, collections: [
        buildCollection(id: 'a', name: 'Band demos', state: 'seeding'),
        buildCollection(id: 'b', name: 'Iceland trip', state: 'downloading'),
      ]);

      expect(find.text('Band demos'), findsOneWidget);
      expect(find.text('Iceland trip'), findsOneWidget);

      await tester.tap(find.byKey(const Key('collectionFilterButton')));
      await pumpTransition(tester);
      await tester.tap(find.text('Sharing'));
      await pumpTransition(tester);
      expect(find.text('Band demos'), findsOneWidget);
      expect(find.text('Iceland trip'), findsNothing);

      await tester.tap(find.byKey(const Key('collectionFilterButton')));
      await pumpTransition(tester);
      await tester.tap(find.text('Receiving'));
      await pumpTransition(tester);
      expect(find.text('Band demos'), findsNothing);
      expect(find.text('Iceland trip'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a failed backend does not look like an empty one',
        (tester) async {
      // These used to render identically, which is what made real failures so
      // hard to spot on device. Home shows a dedicated failure state now,
      // full-page â€” the same one desktop's pane already used â€” rather than
      // falling back to a welcome that has nothing to do with what happened.
      await pumpApp(tester, collections: [], error: 'PanicException(boom)');
      expect(find.textContaining('Couldn\'t load your collections'),
          findsOneWidget);
      expect(find.textContaining('SEND ANYTHING'), findsNothing);

      AppControllers.collections.debugSeed([]);
      await tester.pump();
      expect(find.textContaining('SEND ANYTHING'), findsOneWidget);
      expect(find.textContaining('Couldn\'t load'), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('empty command submission does not open an add flow',
        (tester) async {
      await pumpApp(tester);

      expect(find.byKey(const Key('commandBarField')), findsOneWidget);

      await tester.tap(find.byKey(const Key('commandBarField')));
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await pumpTransition(tester);
      expect(find.byKey(const Key('addShareAction')), findsNothing);
      expect(find.byKey(const Key('addJoinAction')), findsNothing);
      expect(find.byKey(const Key('addTorrentAction')), findsNothing);
      expect(find.text('PRESS ENTER'), findsNothing);
      await tester.tap(find.byKey(const Key('shareCollectionAction')));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byType(ShareScreen), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });



  group('colour carries meaning', () {
    testWidgets('a torrent row is ember and a shared row is mint',
        (tester) async {
      // Tall enough that both rows are built â€” a SliverList doesn't
      // instantiate what it can't show, and the assertion is about both.
      await pumpApp(tester, collections: [
        buildCollection(
            id: 'a', name: 'Shared thing', state: 'downloading',
            downloadMbps: 1),
        buildCollection(
          id: 'b',
          name: 'ubuntu.iso',
          kind: CollectionKind.torrent,
          state: 'downloading',
          downloadMbps: 2,
        ),
      ], size: const Size(390, 1400));

      final bars = tester
          .widgetList<LinearProgressIndicator>(
              find.byType(LinearProgressIndicator))
          .map((w) => w.valueColor?.value)
          .toList();
      expect(bars, contains(AppColors.signal));
      expect(bars, contains(AppColors.ember));
      expect(tester.takeException(), isNull);
    });
  });



  group('sharing is stated, never implied', () {
    testWidgets('says so plainly while the engine is still starting',
        (tester) async {
      // Collections exist on disk but nothing is live yet. Showing them as
      // though they were simply empty is what made a fresh launch look
      // broken.
      await pumpApp(tester, collections: [buildCollection(state: 'pending')]);

      expect(find.textContaining('Starting the transfer engine'),
          findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a seeding collection with content reads as SHARING',
        (tester) async {
      await pumpApp(tester, collections: [
        buildCollection(
          state: 'seeding',
          media: const [MediaItem(label: 'a.jpg', infoHash: 'aa')],
        ),
      ]);

      expect(find.text('SHARING'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('an empty collection does not claim to be sharing',
        (tester) async {
      await pumpApp(tester, collections: [
        buildCollection(state: 'seeding', media: const []),
      ]);

      expect(find.text('SHARING'), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });



  group('polling cadence', () {
    test('an unchanged poll notifies nobody', () async {
      // A settled app polls for minutes without anything moving. Every one of
      // those polls used to rebuild every widget listening to this.
      // Whatever earlier tests left behind, one poll records it â€” the
      // singleton is process-wide, so this cannot assume a starting point.
      final controller = CollectionsController(
        repository: _EmptyCollectionsRepository(),
        peerHistoryStore: _EmptyPeerHistoryStore(),
        transferHistoryStore: _EmptyTransferHistoryStore(),
      );
      await controller.refresh();

      var notifications = 0;
      void count() => notifications++;
      controller.addListener(count);
      addTearDown(controller.dispose);
      addTearDown(() => controller.removeListener(count));

      await controller.refresh();

      expect(notifications, 0);
    });

    testWidgets('slows down when nothing is moving, speeds up when it does',
        (tester) async {
      // The single biggest power cost in an idle app was a one-second FFI
      // round trip plus a full rebuild, forever.
      await pumpApp(tester, collections: [
        buildCollection(state: 'seeding', downloadMbps: 0, uploadMbps: 0),
      ]);
      expect(AppControllers.collections.liveRate, 0);

      await pumpApp(tester, collections: [
        buildCollection(state: 'downloading', downloadMbps: 12.5),
      ]);
      expect(AppControllers.collections.liveRate, 12.5);
    });
  });



  group('what is in flight', () {
    testWidgets('is a filter on Home, not a destination of its own',
        (tester) async {
      // The list already knows how to show a subset, so the subset stays on
      // Home rather than becoming a second destination.
      await pumpApp(tester, collections: [
        buildCollection(id: 'a', name: 'Settled', state: 'seeding'),
        buildCollection(
            id: 'b', name: 'In flight', state: 'downloading', downloadMbps: 5),
      ]);

      expect(find.text('Settled'), findsOneWidget);
      expect(find.text('In flight'), findsOneWidget);

      await tester.tap(find.byKey(const Key('collectionFilterButton')));
      await pumpTransition(tester);
      await tester.tap(find.text('Receiving'));
      await pumpTransition(tester);

      expect(find.text('In flight'), findsOneWidget);
      expect(find.text('Settled'), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });
}

class _EmptyCollectionsRepository implements CollectionsRepository {
  @override
  Future<bool> isEngineReady() async => true;

  @override
  Future<List<Collection>> list() async => const [];

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      Future<dynamic>.error(UnimplementedError());
}

class _EmptyPeerHistoryStore implements PeerHistoryStore {
  @override
  Future<List<PeerObservation>> load() async => const [];

  @override
  Future<void> save(List<PeerObservation> peers) async {}
}

class _EmptyTransferHistoryStore implements TransferHistoryStore {
  @override
  Future<Map<String, TransferHistory>> load() async => const {};

  @override
  Future<void> save(Map<String, TransferHistory> histories) async {}
}
