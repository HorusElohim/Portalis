import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/app/app_controllers.dart';
import 'package:portalis/design/design.dart';
import 'package:portalis/features/collections/domain/collection.dart';
import 'package:portalis/features/collections/domain/paste.dart';
import 'package:portalis/features/settings/domain/engine_settings.dart';
import 'package:portalis/main.dart';
import 'package:portalis/screens/home.dart';
import 'package:portalis/screens/home/collection/collection.dart';
import 'package:portalis/screens/home/collection/join.dart';
import 'package:portalis/screens/home/collection/media_viewer.dart';
import 'package:portalis/screens/home/collection/share.dart';
import 'package:portalis/screens/home/torrent/add.dart';
import 'package:portalis/screens/people.dart';
import 'package:portalis/screens/root_shell.dart';
import 'package:portalis/screens/settings.dart';
import 'package:portalis/services/navigation.dart';
import 'package:portalis/theme.dart';

/// A phone-sized window, below the desktop breakpoint, so the mobile layout.
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
  AppControllers.collections.debugSeed(collections, error: error);
  await tester.pump();
}

/// A real invite code: hex-wrapped `<64-hex secret>:<name>`, the shape
/// `collections.rs::invite_code_for` mints and `decodeInviteCode` reads. Built
/// rather than pasted as a literal so it stays valid if the format moves.
String _inviteCode(String name) {
  final plain = '${'a' * 64}:$name';
  return plain.codeUnits
      .map((c) => c.toRadixString(16).padLeft(2, '0'))
      .join();
}

void main() {
  // Every test drives the same singletons, so each one states the world it
  // expects rather than inheriting whatever the previous test left behind.
  // The selected tab and navigator depth are app-global now (the Home button
  // lives above the navigator and has to reach them), which means a test that
  // switches tabs would otherwise leave every later test starting there.
  tearDown(() {
    AppControllers.collections.debugSeed([]);
    AppNavigation.tab.value = 0;
    AppNavigation.depth.value = 0;
  });

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

  group('what a paste turns out to be', () {
    test('a magnet link and a bare info hash are both magnets', () {
      expect(PasteKind.of('magnet:?xt=urn:btih:${'a' * 40}'), PasteKind.magnet);
      expect(PasteKind.of('a' * 40), PasteKind.magnet);
      // Whitespace round a pasted link is the norm, not the exception.
      expect(PasteKind.of('  ${'a' * 40}  '), PasteKind.magnet);
    });

    test('an invite code is anything that decodes to secret:name', () {
      expect(PasteKind.of(_inviteCode('Iceland trip')), PasteKind.invite);
    });

    test('hex that decodes to nothing shaped like an invite is a search', () {
      // Without the colon check, any even-length hex string decodes to
      // *something* and would be offered as a joinable collection.
      expect(PasteKind.of('abcdef'), PasteKind.search);
    });

    test('a 40-char hash wins over the invite reading', () {
      // It is valid hex of even length, so ordering is what keeps a torrent
      // hash from being mistaken for a collection to join.
      expect(PasteKind.of('a' * 40), PasteKind.magnet);
    });

    test('ordinary words are a search, and empty is empty', () {
      expect(PasteKind.of('iceland'), PasteKind.search);
      expect(PasteKind.of(''), PasteKind.empty);
      expect(PasteKind.of('   '), PasteKind.empty);
    });
  });

  group('shell', () {
    testWidgets('has three destinations and switches between them',
        (tester) async {
      await _pumpApp(tester, collections: []);

      expect(find.text('Home'), findsOneWidget);
      expect(find.text('People'), findsOneWidget);
      expect(find.text('Settings'), findsOneWidget);
      // Transfers and Collections both showed the same collections a second
      // place — every row now carries its own bar, rate and countdown, and
      // the list lives on Home itself.
      expect(find.text('Transfers'), findsNothing);
      expect(find.text('Collections'), findsNothing);
      expect(AppBottomNav.items.length, 3);

      await tester.tap(find.byKey(const Key('navTab1')));
      await tester.pump();
      expect(find.byType(PeopleScreen), findsOneWidget);

      await tester.tap(find.byKey(const Key('navTab2')));
      await tester.pump();
      expect(find.byType(SettingsScreen), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('Home states the aggregate Transfers used to carry',
        (tester) async {
      // The one fact that destination had which the plain list didn't.
      await _pumpApp(tester, collections: [
        _collection(state: 'downloading', downloadMbps: 1.5, uploadMbps: 0.5),
      ]);

      expect(find.textContaining('1 transfer'), findsOneWidget);
      expect(find.textContaining('1.5 MB/s'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('uses the phone layout narrow and three panes wide',
        (tester) async {
      await _pumpApp(tester, collections: [_collection()]);

      await _pumpApp(tester, size: _phone);
      expect(find.byType(AppBottomNav), findsOneWidget);
      expect(find.byKey(const Key('headerPeopleButton')), findsNothing);

      await tester.binding.setSurfaceSize(_desktop);
      await tester.pumpWidget(const MyApp());
      await tester.pump();
      AppControllers.collections.debugSeed([_collection()]);
      await tester.pump();
      // The desktop sidebar carries a People pane that mobile has no room
      // for, and the bottom bar is gone entirely.
      expect(find.byType(AppBottomNav), findsNothing);
      expect(find.byKey(const Key('headerPeopleButton')), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a collection opens inside its own card', (tester) async {
      // It used to take a button in a side panel that pushed a full-screen
      // route over the sidebar, the list and all. Then it was a second panel
      // beside the list — a thinner account of the same collection, plus a
      // button to get from one to the other. The card is the view.
      await _pumpApp(tester, size: _desktop, collections: [
        _collection(id: 'a', name: 'Iceland'),
        _collection(id: 'b', name: 'Studio'),
      ]);

      expect(find.text('Open collection'), findsNothing);
      expect(find.byType(CollectionDetail), findsNothing);

      await tester.tap(find.text('Studio').first);
      await tester.pump();

      // Open in place: the whole list is still there around it.
      expect(find.byType(CollectionDetail), findsOneWidget);
      expect(find.text('Invite'), findsOneWidget);
      expect(find.text('Iceland'), findsWidgets);

      // And clicking it again closes it — the card is the only view, so a
      // second click has nothing else to mean.
      await tester.tap(find.text('Studio').first);
      await tester.pump();
      expect(find.byType(CollectionDetail), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a destination survives crossing the breakpoint',
        (tester) async {
      // The window can now be dragged between the two layouts freely, so the
      // shells have to agree about where you are rather than each keeping its
      // own idea of it.
      await _pumpApp(tester, size: _desktop, collections: [_collection()]);
      await tester.tap(find.byKey(const Key('identityChip')));
      await tester.pump();

      await tester.binding.setSurfaceSize(_phone);
      await tester.pumpWidget(const MyApp());
      await tester.pump();

      expect(find.byType(SettingsScreen), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('the desktop list is destinations only, controls sit above it',
        (tester) async {
      // Transfers said nothing the always-visible list didn't; People and
      // Settings are controls rather than things to look at, so they sit in
      // the header rather than the list. What is left in the list is what
      // there is to look at.
      await _pumpApp(tester, size: _desktop, collections: [_collection()]);

      for (final gone in ['Transfers', 'Settings', 'People']) {
        expect(find.text(gone), findsNothing, reason: '$gone is not a row');
      }
      expect(find.byKey(const Key('headerPeopleButton')), findsOneWidget);
      expect(find.byKey(const Key('headerSettingsButton')), findsOneWidget);

      // Renaming this device was impossible on desktop before it had any way
      // in at all; the identity chip is that way in, and now opens Settings
      // directly rather than a separate You pane.
      await tester.tap(find.byKey(const Key('identityChip')));
      await tester.pump();
      expect(find.byType(SettingsScreen), findsOneWidget);
      expect(find.text('Change name'), findsOneWidget);

      // Tapping the header button for the pane that is already open closes
      // it back to Home — the same toggle every pane button gets.
      await tester.tap(find.byKey(const Key('headerSettingsButton')));
      await tester.pump();
      expect(find.byType(SettingsScreen), findsNothing);

      // One primary action beside one field, plus the one thing a paste
      // cannot express. The sidebar's own New share, Join, magnet field,
      // Paste, Add and .torrent picker are all gone — the omnibar takes any
      // of them as a paste.
      expect(find.text('New share'), findsOneWidget);
      expect(find.text('Join with a key'), findsNothing);
      expect(find.byKey(const Key('sidebarMagnetField')), findsNothing);
      expect(find.byKey(const Key('omnibarField')), findsOneWidget);
      expect(find.byKey(const Key('addTorrentButton')), findsOneWidget);
      // A .torrent is a file, so it is the one thing the bar cannot absorb
      // as a paste — it keeps an affordance, or desktop loses the capability.
      expect(find.byKey(const Key('omnibarTorrentFile')), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('the omnibar dispatches on what was pasted', (tester) async {
      await _pumpApp(tester, size: _desktop, collections: [
        _collection(name: 'Iceland trip'),
        _collection(id: 'c2', name: 'Band demos'),
      ]);

      final field = find.byKey(const Key('omnibarField'));

      // A magnet is unmistakable, so the bar offers to act on it.
      await tester.enterText(field, 'magnet:?xt=urn:btih:${'a' * 40}');
      await tester.pump();
      expect(find.text('ADD TORRENT'), findsOneWidget);
      // ...and does not treat it as a search: both collections stay.
      expect(find.text('Iceland trip'), findsOneWidget);
      expect(find.text('Band demos'), findsOneWidget);

      // Anything that is neither a magnet nor a code was meant as a filter,
      // applied as you type with nothing to press.
      await tester.enterText(field, 'iceland');
      await tester.pump();
      expect(find.text('FILTERING'), findsOneWidget);
      expect(find.text('Iceland trip'), findsOneWidget);
      expect(find.text('Band demos'), findsNothing);

      // Emptying it restores the list whatever the text used to be.
      await tester.enterText(field, '');
      await tester.pump();
      expect(find.text('Band demos'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a pasted invite key still has to be confirmed',
        (tester) async {
      await _pumpApp(tester, size: _desktop, collections: [_collection()]);

      // Joining announces you to strangers — recognising a code opens the
      // join screen with it filled in, it never joins on the paste alone.
      await tester.enterText(
          find.byKey(const Key('omnibarField')), _inviteCode('Iceland trip'));
      await tester.pump();
      expect(find.text('JOIN'), findsOneWidget);

      await tester.tap(find.byKey(const Key('omnibarSubmit')));
      await tester.pumpAndSettle();

      expect(find.byType(JoinCollectionScreen), findsOneWidget);
      expect(find.text('Code recognised'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

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

    testWidgets('a downloading collection says when it lands', (tester) async {
      await _pumpApp(
        tester,
        collections: [
          _collection(
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
      await _pumpApp(
        tester,
        collections: [
          _collection(state: 'downloading', totalBytes: 1000, downloadedBytes: 400),
        ],
      );

      expect(find.textContaining('left'), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });

  group('home holds both the welcome and the list', () {
    testWidgets('shows the welcome when there is nothing yet', (tester) async {
      await _pumpApp(tester);
      expect(find.textContaining('SEND ANYTHING'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('shows the list once you own something, not a second welcome',
        (tester) async {
      // Home doubles as the list now — the merge that gave desktop's pane a
      // search bar and filter chips gave mobile the list it used to keep on
      // a separate Collections tab. One destination for "what can I do" and
      // "what do I have", on both layouts, the same way desktop's pane
      // already worked before this merge.
      await _pumpApp(tester,
          collections: [_collection(name: 'Iceland trip')],
          size: const Size(390, 1300));

      expect(find.text('Iceland trip'), findsOneWidget);
      expect(find.textContaining('SEND ANYTHING'), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });

  group('home tab', () {
    testWidgets('is the leftmost destination and returns from another tab',
        (tester) async {
      await _pumpApp(tester, collections: []);
      expect(find.text('Home'), findsOneWidget);

      await tester.tap(find.byKey(const Key('navTab2')));
      await tester.pump();
      expect(find.byType(SettingsScreen), findsOneWidget);

      await tester.tap(find.byKey(const Key('navTab0')));
      await tester.pump();

      expect(AppNavigation.tab.value, 0);
      expect(tester.takeException(), isNull);
    });

    testWidgets('unwinds a pushed screen rather than only switching tab',
        (tester) async {
      // The distinction that makes it a Home button and not just a tab: one
      // tap lands you at the start, not one screen shallower.
      await _pumpApp(tester);
      await tester.tap(find.byKey(const Key('shareSomethingButton')));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byType(ShareScreen), findsOneWidget);
      expect(AppNavigation.depth.value, greaterThan(0));

      AppNavigation.goHome();
      // Stepped, so the pop transition actually finishes: depth drops
      // immediately but the outgoing route stays mounted until it does.
      for (var i = 0; i < 12; i++) {
        await tester.pump(const Duration(milliseconds: 50));
      }

      expect(find.byType(ShareScreen), findsNothing);
      expect(AppNavigation.depth.value, 0);
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
      // The welcome is unconditional once there is genuinely nothing owned;
      // only the live card is conditional on something actually moving.
      expect(find.textContaining('SEND ANYTHING'), findsNothing);
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
      // hard to spot on device. Home shows a dedicated failure state now,
      // full-page — the same one desktop's pane already used — rather than
      // falling back to a welcome that has nothing to do with what happened.
      await _pumpApp(tester, collections: [], error: 'PanicException(boom)');
      expect(find.textContaining('Couldn\'t load your collections'),
          findsOneWidget);
      expect(find.textContaining('SEND ANYTHING'), findsNothing);

      AppControllers.collections.debugSeed([]);
      await tester.pump();
      expect(find.textContaining('SEND ANYTHING'), findsOneWidget);
      expect(find.textContaining('Couldn\'t load'), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets(
        'offers New share, Add torrent, and the omnibar for the rest',
        (tester) async {
      await _pumpApp(tester);

      expect(find.byKey(const Key('addTorrentButton')), findsOneWidget);
      expect(find.byKey(const Key('omnibarField')), findsOneWidget);

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
      // Tall enough that both rows are built — a SliverList doesn't
      // instantiate what it can't show, and the assertion is about both.
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
    test('an unchanged poll notifies nobody', () async {
      // A settled app polls for minutes without anything moving. Every one of
      // those polls used to rebuild every widget listening to this.
      // Whatever earlier tests left behind, one poll records it — the
      // singleton is process-wide, so this cannot assume a starting point.
      await AppControllers.collections.refresh();

      var notifications = 0;
      void count() => notifications++;
      AppControllers.collections.addListener(count);
      addTearDown(() => AppControllers.collections.removeListener(count));

      await AppControllers.collections.refresh();

      expect(notifications, 0);
    });

    testWidgets('slows down when nothing is moving, speeds up when it does',
        (tester) async {
      // The single biggest power cost in an idle app was a one-second FFI
      // round trip plus a full rebuild, forever.
      await _pumpApp(tester, collections: [
        _collection(state: 'seeding', downloadMbps: 0, uploadMbps: 0),
      ]);
      expect(AppControllers.collections.liveRate, 0);

      await _pumpApp(tester, collections: [
        _collection(state: 'downloading', downloadMbps: 12.5),
      ]);
      expect(AppControllers.collections.liveRate, 12.5);
    });
  });

  group('what is in flight', () {
    testWidgets('is a filter on Home, not a destination of its own',
        (tester) async {
      // This was the Transfers screen's whole job. The list already knew how
      // to show a subset, so the subset moved here rather than keeping a
      // second screen to hold it.
      await _pumpApp(tester, collections: [
        _collection(id: 'a', name: 'Settled', state: 'seeding'),
        _collection(
            id: 'b', name: 'In flight', state: 'downloading', downloadMbps: 5),
      ]);

      expect(find.text('Settled'), findsOneWidget);
      // Twice: the live hero card names whatever is moving fastest, on top
      // of its own row in the list below.
      expect(find.text('In flight'), findsWidgets);

      await tester.tap(find.text('Receiving'));
      await tester.pump();

      expect(find.text('In flight'), findsWidgets);
      expect(find.text('Settled'), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });

  group('people and settings', () {
    testWidgets('the Settings profile counts distinct collaborators',
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

    // People has gone missing on a platform twice: first it existed only as
    // a desktop sidebar pane, then as a row so far down the old You screen
    // that it was past the address, the identity notice and File formats.
    // Both times it was reachable in principle and unfindable in practice.
    // It is now its own bottom tab on both layouts — direct rather than
    // reachable — and the count in Settings' profile section is a shortcut
    // onto that same tab, not a second route to a second copy of it.
    testWidgets(
        'People is its own tab, and the Settings profile count is a shortcut to it',
        (tester) async {
      const ana = Collaborator(deviceId: 'dev-ana', name: 'Ana');
      await _pumpApp(tester, collections: [
        _collection(id: 'a', collaborators: [ana]),
      ]);

      // Direct: no intermediate screen to lose it behind.
      await tester.tap(find.byKey(const Key('navTab1')));
      await tester.pump();
      expect(find.byType(PeopleScreen), findsOneWidget);
      expect(find.text('Ana'), findsOneWidget);

      // The Settings profile's count still works, but as a shortcut onto the
      // People tab rather than a push — tapping it selects the shared tab.
      await tester.tap(find.byKey(const Key('navTab2')));
      await tester.pump();
      await tester.tap(find.text('PEOPLE'));
      await tester.pumpAndSettle();

      expect(find.byType(PeopleScreen), findsOneWidget);
      expect(AppNavigation.tab.value, 1);
      expect(tester.takeException(), isNull);
    });
  });

  group('settings', () {
    testWidgets('hides engine internals behind Network & engine',
        (tester) async {
      await tester.binding.setSurfaceSize(_phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      AppControllers.settings.debugSeed(_engineSettings());
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
      AppControllers.settings.debugSeed(_engineSettings());
      await tester.pumpWidget(
        const MaterialApp(home: SettingsScreen(advanced: true)),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));

      expect(find.text('Listen ports'), findsOneWidget);
      expect(find.text('Disable DHT'), findsOneWidget);
      // Profile — the old You screen's content — is scoped to Basic; the
      // engine internals drill-down has nothing to do with identity.
      expect(find.text('Change name'), findsNothing);
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

    testWidgets('the viewer carries its own details and stays live',
        (tester) async {
      // Reading a file's size used to cost two taps and a screen transition,
      // and the screen it landed on held a snapshot taken when the tile was
      // tapped — so the numbers stopped moving exactly when they mattered.
      const media = MediaItem(
        label: 'clip.mp4',
        entryLabel: 'Beach day',
        infoHash: 'aa',
        sizeBytes: 1000,
        downloadedBytes: 400,
        progress: 0.4,
      );
      final collection = _collection(
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

      // The rest is a disclosure, not a destination — no route is pushed.
      expect(find.text('Info hash'), findsNothing);
      await tester.tap(find.text('Details'));
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.text('Info hash'), findsOneWidget);
      expect(find.byType(MediaViewerScreen), findsOneWidget);

      // And it follows the cache rather than the arguments it was built with.
      AppControllers.collections.debugSeed([
        _collection(
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
      // state and an id — everything on it that moved is now on the screen
      // itself.
      final collection = _collection(state: 'seeding');
      AppControllers.collections.debugSeed([collection]);
      await tester.binding.setSurfaceSize(const Size(390, 1000));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(MaterialApp(
        home: CollectionScreen(collection: collection),
      ));
      await tester.pump();

      expect(find.text('Collection id'), findsNothing);
      await tester.tap(find.byTooltip('Details'));
      await tester.pump(const Duration(milliseconds: 300));

      expect(find.text('Collection id'), findsOneWidget);
      expect(find.byType(CollectionScreen), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('contents are grouped by the batch they arrived in',
        (tester) async {
      // A collection grows one signed manifest entry at a time; the grid used
      // to flatten that away, so what arrived together — and from whom — was
      // invisible.
      final collection = _collection(
        state: 'downloading',
        collaborators: const [Collaborator(deviceId: 'dev1', name: 'Mark')],
        media: const [
          MediaItem(
              label: 'a.jpg', entryLabel: 'Beach day', infoHash: 'aa',
              sizeBytes: 2000, downloadedBytes: 2000, progress: 1, addedBy: 'dev1'),
          MediaItem(
              label: 'b.jpg', entryLabel: 'Beach day', infoHash: 'aa',
              sizeBytes: 2000, downloadedBytes: 2000, progress: 1, addedBy: 'dev1'),
          MediaItem(
              label: 'later', entryLabel: 'Sunday', infoHash: 'bb',
              fetched: false, addedBy: 'dev1'),
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
