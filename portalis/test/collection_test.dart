import 'test_support.dart';

import 'package:portalis/features/collections/domain/peer_observation.dart';
import 'package:portalis/features/collections/presentation/collection_peers.dart';
import 'package:portalis/features/collections/presentation/collection_presentation.dart';

void main() {
  tearDown(resetTestState);

  group('collection', () {
    test('peer age keeps only the compact time unit', () {
      final now = DateTime(2026, 8, 11, 12);

      expect(
        formatLastSeen(now.subtract(const Duration(seconds: 4)), now: now),
        '4s',
      );
      expect(
        formatLastSeen(now.subtract(const Duration(minutes: 2)), now: now),
        '2m',
      );
      expect(
        formatLastSeen(now.subtract(const Duration(hours: 3)), now: now),
        '3h',
      );
    });

    testWidgets('active and disconnected peers have honest distinct colours',
        (tester) async {
      final observedAt = DateTime.now().subtract(const Duration(seconds: 12));
      final collection = buildCollection(
        kind: CollectionKind.torrent,
        torrentPeers: const ['203.0.113.5:6881'],
      );

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: CollectionPeers(
              collection: collection,
              peerHistory: [
                PeerObservation(
                  collectionId: collection.id,
                  collectionName: collection.name,
                  address: '203.0.113.5:6881',
                  lastSeen: observedAt,
                ),
                PeerObservation(
                  collectionId: collection.id,
                  collectionName: collection.name,
                  address: '198.51.100.9:6881',
                  lastSeen: observedAt,
                ),
              ],
            ),
          ),
        ),
      );

      final active = tester.widget<Text>(find.text('203.0.113.5:6881'));
      final disconnected = tester.widget<Text>(find.text('198.51.100.9:6881'));
      expect(active.style?.color, AppColors.ember);
      expect(
        disconnected.style?.color,
        rememberedPeerColor('198.51.100.9:6881'),
      );
      expect(disconnected.style?.color, isNot(AppColors.textFaint));
      expect(find.textContaining(RegExp(r'^1[2-3]s$')), findsNWidgets(2));
      expect(find.textContaining('seen'), findsNothing);
      expect(find.textContaining('ago'), findsNothing);
      expect(tester.takeException(), isNull);
    });

    test('only active native imports keep real-time polling enabled', () {
      final active = buildCollection(
        ingestion: const CollectionImport(
          stage: 'copying',
          progress: 0.4,
          processedBytes: 400,
          totalBytes: 1000,
        ),
      );
      final failed = buildCollection(
        ingestion: const CollectionImport(
          stage: 'failed',
          progress: 0,
          processedBytes: 400,
          totalBytes: 1000,
          error: 'source disappeared',
        ),
      );

      expect(active.isMoving, isTrue);
      expect(failed.isMoving, isFalse);
    });

    testWidgets('a shared collection shows its own invite code',
        (tester) async {
      // The invite code travels *with* the collection, so showing it needs no
      // round trip â€” this used to mint a throwaway collection on every tap.
      const collection = Collection(
        id: 'e7b1f0aa-0000-4000-8000-000000000001',
        name: 'Test Collection',
        kind: CollectionKind.shared,
        inviteCode: 'abcdef0123456789',
        collaborators: [],
        media: [],
        state: 'empty',
      );
      await tester.binding.setSurfaceSize(phoneSize);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        const MaterialApp(home: CollectionScreen(collection: collection)),
      );
      await tester.pump();

      await tester.tap(find.text('Invite'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));

      expect(find.text('Invite a collaborator'), findsOneWidget);
      expect(find.text('abcdef0123456789'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a plain torrent offers no invite or add-media',
        (tester) async {
      // A torrent's contents are fixed by its info-hash and it has no invite
      // secret, so those actions must not appear â€” they would be dead
      // buttons.
      const collection = Collection(
        id: '0123456789abcdef0123456789abcdef01234567',
        name: 'Some Torrent',
        kind: CollectionKind.torrent,
        collaborators: [],
        media: [],
        state: 'downloading',
      );
      await tester.binding.setSurfaceSize(phoneSize);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        const MaterialApp(home: CollectionScreen(collection: collection)),
      );
      await tester.pump();

      expect(find.text('Some Torrent'), findsOneWidget);
      expect(find.text('Invite'), findsNothing);
      expect(find.text('ï¼‹ Add media'), findsNothing);
      expect(find.text('Sync'), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('deletion is one command with collection-or-files choices',
        (tester) async {
      final collection = buildCollection(name: 'Trip archive');
      await tester.binding.setSurfaceSize(const Size(390, 1000));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        MaterialApp(home: CollectionScreen(collection: collection)),
      );
      await tester.pump();

      expect(find.text('Forget'), findsNothing);
      expect(find.text('Delete files'), findsNothing);
      await tester.tap(find.byKey(const Key('collectionCommanddelete')));
      await pumpTransition(tester);

      expect(find.text('Delete "Trip archive"?'), findsOneWidget);
      expect(find.text('Cancel'), findsOneWidget);
      expect(find.byKey(const Key('deleteCollectionOnly')), findsOneWidget);
      expect(
        find.byKey(const Key('deleteCollectionWithFiles')),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    });

    test('media regroups into the manifest entries it was flattened from', () {
      // The grid renders a flat file list, but the unit a collection *grows*
      // by is the manifest entry â€” the details screen shows that structure.
      const collection = Collection(
        id: 'c1',
        name: 'Trip',
        kind: CollectionKind.shared,
        collaborators: [],
        media: [
          MediaItem(
              label: 'a.mp4',
              entryLabel: 'Beach day',
              infoHash: 'aa',
              sizeBytes: 100,
              downloadedBytes: 100,
              addedBy: 'dev1'),
          MediaItem(
              label: 'b.mp4',
              entryLabel: 'Beach day',
              infoHash: 'aa',
              sizeBytes: 100,
              downloadedBytes: 50,
              addedBy: 'dev1'),
          MediaItem(
              label: 'later',
              entryLabel: 'later',
              infoHash: 'bb',
              fetched: false,
              addedBy: 'dev2'),
        ],
        state: 'downloading',
      );

      final entries = collection.entries;

      expect(entries.length, 2);
      // The entry's own signed label, not its first file's name.
      expect(entries.first.label, 'Beach day');
      expect(entries.first.infoHash, 'aa');
      expect(entries.first.media.length, 2);
      expect(entries.first.addedBy, 'dev1');
      expect(entries.first.fetched, isTrue);
      expect(entries.first.totalBytes, 200);
      expect(entries.first.downloadedBytes, 150);
      expect(entries.first.progress, 0.75);
      // A not-yet-fetched entry has no byte counts to report â€” its size isn't
      // knowable until the torrent's metadata arrives.
      expect(entries.last.fetched, isFalse);
      expect(entries.last.totalBytes, 0);
      expect(entries.last.progress, 0.0);
    });

    testWidgets('the viewer carries its own details and stays live',
        (tester) async {
      // Reading a file's size used to cost two taps and a screen transition,
      // and the screen it landed on held a snapshot taken when the tile was
      // tapped â€” so the numbers stopped moving exactly when they mattered.
      const media = MediaItem(
        label: 'clip.mp4',
        entryLabel: 'Beach day',
        infoHash: 'aa',
        localPath: 'C:/Media/clip.mp4',
        sizeBytes: 1000,
        downloadedBytes: 400,
        progress: 0.4,
      );
      final collection = buildCollection(
        state: 'downloading',
        media: const [media],
        totalBytes: 1000,
        downloadedBytes: 400,
        downloadMbps: 2,
        livePeers: 3,
      );
      AppControllers.collections.debugSeed([collection]);
      await tester.binding.setSurfaceSize(const Size(390, 1000));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(MaterialApp(
        home: MediaViewerScreen(collection: collection, media: media),
      ));
      await tester.pump();

      // On screen without asking: how much of it is here.
      expect(find.textContaining('400 B of 1 KB'), findsOneWidget);
      expect(find.textContaining('3 peers'), findsOneWidget);

      // Details are part of the viewer itself; no second tap or route is
      // needed to inspect the media metadata.
      expect(find.text('Info hash'), findsOneWidget);
      expect(find.text('File path'), findsOneWidget);
      expect(find.byType(MediaViewerScreen), findsOneWidget);

      // And it follows the cache rather than the arguments it was built with.
      AppControllers.collections.debugSeed([
        buildCollection(
          state: 'downloading',
          media: const [
            MediaItem(
              label: 'clip.mp4',
              entryLabel: 'Beach day',
              infoHash: 'aa',
              sizeBytes: 1000,
              downloadedBytes: 900,
              progress: 0.9,
            ),
          ],
          totalBytes: 1000,
          downloadedBytes: 900,
          downloadMbps: 2,
          livePeers: 3,
        ),
      ]);
      await tester.pump();

      expect(find.textContaining('900 B of 1 KB'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('identifiers are a disclosure, not a destination',
        (tester) async {
      // They were a pushed screen whose only remaining content was a type, a
      // state and an id â€” everything on it that moved is now on the screen
      // itself.
      final collection = buildCollection(state: 'seeding');
      AppControllers.collections.debugSeed([collection]);
      await tester.binding.setSurfaceSize(const Size(390, 1000));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(MaterialApp(
        home: CollectionScreen(collection: collection),
      ));
      await tester.pump();

      expect(find.text('Collection id'), findsOneWidget);
      expect(find.byTooltip('Details'), findsNothing);
      expect(find.byType(CollectionScreen), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('shows anonymous torrent peer addresses in the collection',
        (tester) async {
      final collection = buildCollection(
        kind: CollectionKind.torrent,
        torrentPeers: const ['203.0.113.5:6881'],
        livePeers: 1,
      );
      await tester.binding.setSurfaceSize(phoneSize);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        MaterialApp(home: CollectionScreen(collection: collection)),
      );
      await tester.pump();

      expect(find.text('203.0.113.5:6881'), findsOneWidget);
      expect(find.text('PEERS - 1'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('contents are grouped by the batch they arrived in',
        (tester) async {
      // A collection grows one signed manifest entry at a time; the grid used
      // to flatten that away, so what arrived together â€” and from whom â€” was
      // invisible.
      final collection = buildCollection(
        state: 'downloading',
        collaborators: const [Collaborator(deviceId: 'dev1', name: 'Mark')],
        media: const [
          MediaItem(
              label: 'a.jpg',
              entryLabel: 'Beach day',
              infoHash: 'aa',
              sizeBytes: 2000,
              downloadedBytes: 2000,
              progress: 1,
              addedBy: 'dev1'),
          MediaItem(
              label: 'b.jpg',
              entryLabel: 'Beach day',
              infoHash: 'aa',
              sizeBytes: 2000,
              downloadedBytes: 2000,
              progress: 1,
              addedBy: 'dev1'),
          MediaItem(
              label: 'later',
              entryLabel: 'Sunday',
              infoHash: 'bb',
              fetched: false,
              addedBy: 'dev1'),
        ],
      );
      AppControllers.collections.debugSeed([collection]);
      await tester.binding.setSurfaceSize(const Size(390, 1400));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(MaterialApp(
        home: CollectionScreen(collection: collection),
      ));
      await tester.pump();

      // The batch label, its size, and the collaborator who signed it.
      expect(find.text('Beach day'), findsOneWidget);
      expect(find.textContaining('2 files'), findsOneWidget);
      expect(find.textContaining('from Mark'), findsWidgets);
      // And each file says what it is without being opened.
      expect(find.text('a.jpg'), findsOneWidget);
      expect(find.text('Sunday'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });
}
