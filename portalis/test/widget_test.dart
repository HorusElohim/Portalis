import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/main.dart';
import 'package:portalis/models.dart';
import 'package:portalis/screens/add_torrent_screen.dart';
import 'package:portalis/screens/collection_details_screen.dart';
import 'package:portalis/screens/home_screen.dart';
import 'package:portalis/screens/collection_screen.dart';
import 'package:portalis/screens/join_collection_screen.dart';
import 'package:portalis/screens/root_shell.dart';
import 'package:portalis/screens/settings_screen.dart';
import 'package:portalis/screens/share_screen.dart';
import 'package:portalis/screens/transfers_screen.dart';
import 'package:portalis/screens/user_screen.dart';
import 'package:portalis/services/collections.dart';
import 'package:portalis/services/settings_service.dart';
import 'package:portalis/theme.dart';

/// A phone-sized window — below [kDesktopBreakpoint], so the mobile layout.
const _phone = Size(390, 844);

/// Comfortably above the breakpoint, for the three-pane layout.
const _desktop = Size(1280, 800);

Collection _collection({
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
      state: state,
    );

EngineSettings _engineSettings() => const EngineSettings(
      listenPortStart: 6881,
      listenPortEnd: 6999,
      enableUpnpPortForwarding: true,
      disableDht: false,
      disableDhtPersistence: false,
      persistSession: true,
      fastresume: true,
      trackers: [],
    );

/// Pumps the app and then seeds the cache.
///
/// Order matters: `RootShell.initState` starts polling, and that first
/// (failing) refresh would overwrite anything seeded beforehand.
Future<void> _pumpApp(
  WidgetTester tester, {
  Size size = _phone,
  List<Collection> collections = const [],
  String? error,
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(const MyApp());
  await tester.pump();
  Collections.instance.debugSeed(collections, error: error);
  await tester.pump();
}

void main() {
  // Every test drives the same singleton, so each one states the world it
  // expects rather than inheriting whatever the previous test left behind.
  tearDown(() => Collections.instance.debugSeed([]));

  group('design system', () {
    test('the signal accent is the mint the design specifies', () {
      expect(AppColors.signal, const Color(0xFF5CE7A3));
      expect(AppColors.ember, const Color(0xFFF0B357));
    });

    test('per-collection hues never collide with signal or ember', () {
      // A collection's identity colour must not be mistakable for
      // "transferring" or "torrent" — that is the whole point of reserving
      // those two.
      expect(AppColors.hues, isNot(contains(AppColors.signal)));
      expect(AppColors.hues, isNot(contains(AppColors.ember)));
    });
  });

  group('shell', () {
    testWidgets('has three destinations and switches between them',
        (tester) async {
      await _pumpApp(tester, collections: []);

      expect(find.text('Collections'), findsOneWidget);
      expect(find.text('Transfers'), findsWidgets);
      expect(find.text('You'), findsOneWidget);

      await tester.tap(find.byKey(const Key('navTab1')));
      await tester.pump();
      expect(find.byType(TransfersScreen), findsOneWidget);

      await tester.tap(find.byKey(const Key('navTab2')));
      await tester.pump();
      expect(find.byType(UserScreen), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('uses the phone layout narrow and three panes wide',
        (tester) async {
      await _pumpApp(tester, collections: [_collection()]);

      await _pumpApp(tester, size: _phone);
      expect(find.byType(AppBottomNav), findsOneWidget);
      expect(find.text('People'), findsNothing);

      await tester.binding.setSurfaceSize(_desktop);
      await tester.pumpWidget(const MyApp());
      await tester.pump();
      Collections.instance.debugSeed([_collection()]);
      await tester.pump();
      // The desktop sidebar carries a People pane that mobile has no room
      // for, and the bottom bar is gone entirely.
      expect(find.byType(AppBottomNav), findsNothing);
      expect(find.text('People'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

  group('home', () {
    testWidgets('shows a live hero card when something is moving',
        (tester) async {
      await _pumpApp(tester, collections: [
        _collection(
          name: 'Iceland trip',
          state: 'downloading',
          downloadMbps: 42.6,
          totalBytes: 1900000000,
          downloadedBytes: 1400000000,
          livePeers: 3,
        ),
      ]);

      expect(find.byType(LiveTransferCard), findsOneWidget);
      expect(find.text('Moving now'), findsOneWidget);
      expect(find.text('RECEIVING'), findsOneWidget);
      expect(find.text('42.6'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('shows no hero card when nothing is moving', (tester) async {
      // The rule the whole palette rests on: mint means data is moving, so an
      // idle collection must not produce a live card at all.
      await _pumpApp(tester, collections: [
        _collection(state: 'seeding', downloadMbps: 0, uploadMbps: 0),
      ]);

      expect(find.byType(LiveTransferCard), findsNothing);
      expect(find.text('Moving now'), findsNothing);
      expect(find.text('Your collections'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('filter chips narrow the list by state', (tester) async {
      await _pumpApp(tester, collections: [
        _collection(id: 'a', name: 'Band demos', state: 'seeding'),
        _collection(id: 'b', name: 'Iceland trip', state: 'downloading'),
      ]);

      expect(find.text('Band demos'), findsOneWidget);
      expect(find.text('Iceland trip'), findsOneWidget);

      await tester.tap(find.text('Sharing'));
      await tester.pump();
      expect(find.text('Band demos'), findsOneWidget);
      expect(find.text('Iceland trip'), findsNothing);

      await tester.tap(find.text('Receiving'));
      await tester.pump();
      expect(find.text('Band demos'), findsNothing);
      expect(find.text('Iceland trip'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a failed backend does not look like an empty one',
        (tester) async {
      // These used to render identically, which is what made real failures so
      // hard to spot on device.
      await _pumpApp(tester, collections: [], error: 'PanicException(boom)');

      expect(find.textContaining('Couldn\'t load your collections'),
          findsOneWidget);
      expect(find.textContaining('Send anything'), findsNothing);

      await _pumpApp(tester, collections: []);
      await tester.pump();
      expect(find.textContaining('Send anything'), findsOneWidget);
      expect(find.textContaining('Couldn\'t load'), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('first run offers the three ways in', (tester) async {
      await _pumpApp(tester);

      await tester.tap(find.byKey(const Key('shareSomethingButton')));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byType(ShareScreen), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

  group('colour carries meaning', () {
    testWidgets('a torrent row is ember and a shared row is mint',
        (tester) async {
      await _pumpApp(tester, collections: [
        _collection(
            id: 'a', name: 'Shared thing', state: 'downloading',
            downloadMbps: 1),
        _collection(
          id: 'b',
          name: 'ubuntu.iso',
          kind: CollectionKind.torrent,
          state: 'downloading',
          downloadMbps: 2,
        ),
      ]);

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
      await _pumpApp(tester, collections: [_collection(state: 'pending')]);

      expect(find.textContaining('Starting the transfer engine'),
          findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a seeding collection with content reads as SHARING',
        (tester) async {
      await _pumpApp(tester, collections: [
        _collection(
          state: 'seeding',
          media: const [MediaItem(label: 'a.jpg', infoHash: 'aa')],
        ),
      ]);

      expect(find.text('SHARING'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('an empty collection does not claim to be sharing',
        (tester) async {
      await _pumpApp(tester, collections: [
        _collection(state: 'seeding', media: const []),
      ]);

      expect(find.text('SHARING'), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });

  group('polling cadence', () {
    testWidgets('slows down when nothing is moving, speeds up when it does',
        (tester) async {
      // The single biggest power cost in an idle app was a one-second FFI
      // round trip plus a full rebuild, forever.
      await _pumpApp(tester, collections: [
        _collection(state: 'seeding', downloadMbps: 0, uploadMbps: 0),
      ]);
      expect(Collections.instance.liveRate, 0);

      await _pumpApp(tester, collections: [
        _collection(state: 'downloading', downloadMbps: 12.5),
      ]);
      expect(Collections.instance.liveRate, 12.5);
    });
  });

  group('transfers', () {
    testWidgets('lists only what is in flight, else an empty state',
        (tester) async {
      await _pumpApp(tester, collections: [
        _collection(id: 'a', name: 'Settled', state: 'seeding'),
        _collection(
            id: 'b', name: 'In flight', state: 'downloading', downloadMbps: 5),
      ]);
      await tester.tap(find.byKey(const Key('navTab1')));
      await tester.pump();

      expect(find.text('In flight'), findsOneWidget);
      expect(find.text('Settled'), findsNothing);

      await _pumpApp(tester, collections: [
        _collection(id: 'a', name: 'Settled', state: 'seeding'),
      ]);
      await tester.pump();
      expect(find.textContaining('No transfers in flight'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

  group('you', () {
    testWidgets('counts distinct collaborators across collections',
        (tester) async {
      const ana = Collaborator(deviceId: 'dev-ana', name: 'Ana');
      const jonas = Collaborator(deviceId: 'dev-jonas', name: 'Jonas');
      const rosa = Collaborator(deviceId: 'dev-rosa', name: 'Rosa');
      await _pumpApp(tester, collections: [
        // Ana is in both collections; she is one person, not two. Four
        // memberships across two collections, three actual people — the
        // counts differ deliberately so this can only pass by deduplicating.
        _collection(id: 'a', collaborators: [ana, jonas]),
        _collection(id: 'b', collaborators: [ana, rosa]),
      ]);
      await tester.tap(find.byKey(const Key('navTab2')));
      await tester.pump();

      expect(find.text('PEOPLE'), findsOneWidget);
      expect(find.text('3'), findsOneWidget);
      expect(find.text('COLLECTIONS'), findsOneWidget);
      expect(find.text('2'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

  group('settings', () {
    testWidgets('hides engine internals behind Network & engine',
        (tester) async {
      await tester.binding.setSurfaceSize(_phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      SettingsService.instance.debugSeed(_engineSettings());
      await tester.pumpWidget(const MaterialApp(home: SettingsScreen()));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));

      // Advanced rows are not on the first screen.
      expect(find.text('Network & engine'), findsOneWidget);
      expect(find.text('Listen ports'), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('the advanced screen exposes the construction-time rows',
        (tester) async {
      await tester.binding.setSurfaceSize(_phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      SettingsService.instance.debugSeed(_engineSettings());
      await tester.pumpWidget(
        const MaterialApp(home: SettingsScreen(advanced: true)),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));

      expect(find.text('Listen ports'), findsOneWidget);
      expect(find.text('Disable DHT'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

  group('collection', () {
    testWidgets('a shared collection shows its own invite code', (tester) async {
      // The invite code travels *with* the collection, so showing it needs no
      // round trip — this used to mint a throwaway collection on every tap.
      const collection = Collection(
        id: 'e7b1f0aa-0000-4000-8000-000000000001',
        name: 'Test Collection',
        kind: CollectionKind.shared,
        inviteCode: 'abcdef0123456789',
        collaborators: [],
        media: [],
        state: 'empty',
      );
      await tester.binding.setSurfaceSize(_phone);
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
      // secret, so those actions must not appear — they would be dead
      // buttons.
      const collection = Collection(
        id: '0123456789abcdef0123456789abcdef01234567',
        name: 'Some Torrent',
        kind: CollectionKind.torrent,
        collaborators: [],
        media: [],
        state: 'downloading',
      );
      await tester.binding.setSurfaceSize(_phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        const MaterialApp(home: CollectionScreen(collection: collection)),
      );
      await tester.pump();

      expect(find.text('Some Torrent'), findsOneWidget);
      expect(find.text('Invite'), findsNothing);
      expect(find.text('＋ Add media'), findsNothing);
      expect(find.text('Sync'), findsNothing);
      expect(tester.takeException(), isNull);
    });

    test('media regroups into the manifest entries it was flattened from', () {
      // The grid renders a flat file list, but the unit a collection *grows*
      // by is the manifest entry — the details screen shows that structure.
      const collection = Collection(
        id: 'c1',
        name: 'Trip',
        kind: CollectionKind.shared,
        collaborators: [],
        media: [
          MediaItem(
              label: 'a.mp4', entryLabel: 'Beach day', infoHash: 'aa',
              sizeBytes: 100, downloadedBytes: 100, addedBy: 'dev1'),
          MediaItem(
              label: 'b.mp4', entryLabel: 'Beach day', infoHash: 'aa',
              sizeBytes: 100, downloadedBytes: 50, addedBy: 'dev1'),
          MediaItem(
              label: 'later', entryLabel: 'later', infoHash: 'bb',
              fetched: false, addedBy: 'dev2'),
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
      // A not-yet-fetched entry has no byte counts to report — its size isn't
      // knowable until the torrent's metadata arrives.
      expect(entries.last.fetched, isFalse);
      expect(entries.last.totalBytes, 0);
      expect(entries.last.progress, 0.0);
    });

    testWidgets('details degrades when the collection is unavailable',
        (tester) async {
      Collections.instance.debugSeed([]);
      await tester.binding.setSurfaceSize(_phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(const MaterialApp(
        home: CollectionDetailsScreen(collectionId: 'does-not-exist'),
      ));
      await tester.pump();

      expect(find.textContaining('no longer available'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

  group('flows still surface backend errors', () {
    testWidgets('torrent screen validates input', (tester) async {
      await tester.binding.setSurfaceSize(_phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(const MaterialApp(home: AddTorrentScreen()));
      await tester.pump();

      await tester.enterText(
        find.byKey(const Key('magnetField')),
        'magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567',
      );
      await tester.pump();
      expect(find.text('READY TO ADD'), findsOneWidget);

      await tester.tap(find.byKey(const Key('addMagnetButton')));
      await tester.pump();
      // RustLib isn't initialized, so the add is expected to fail — inline
      // error, screen stays open, no uncaught exception.
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byType(AddTorrentScreen), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('join screen recognises a valid code', (tester) async {
      await tester.binding.setSurfaceSize(_phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(const MaterialApp(home: JoinCollectionScreen()));
      await tester.pump();

      // Invite codes are hex-wrapped (collections.rs::invite_code_for) so the
      // name and address aren't visible in plain text — encode the same way.
      final plain = '${'ab' * 32}:Test Collection@192.168.1.9:5321';
      final code = plain.codeUnits
          .map((c) => c.toRadixString(16).padLeft(2, '0'))
          .join();
      await tester.enterText(find.byKey(const Key('inviteCodeField')), code);
      await tester.pump();

      expect(find.text('Test Collection'), findsOneWidget);
      expect(find.textContaining('1 address'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('share screen requires a name and files', (tester) async {
      await tester.binding.setSurfaceSize(_phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(const MaterialApp(home: ShareScreen()));
      await tester.pump();

      expect(find.text('Nothing added yet'), findsOneWidget);
      await tester.enterText(
          find.byKey(const Key('collectionNameField')), 'Trip to Lisbon');
      await tester.pump();

      final button = tester
          .widget<FilledButton>(find.byKey(const Key('createShareButton')));
      expect(button.onPressed, isNull);
      expect(tester.takeException(), isNull);
    });
  });
}
