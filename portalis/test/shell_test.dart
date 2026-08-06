import 'test_support.dart';

void main() {
  tearDown(resetTestState);

group('shell', () {
    testWidgets('has four destinations and switches between them',
        (tester) async {
      await pumpApp(tester, collections: []);

      expect(find.text('Home'), findsOneWidget);
      expect(find.text('People'), findsOneWidget);
      expect(find.text('Settings'), findsOneWidget);
      // Transfers and Collections both showed the same collections a second
      // place â€” every row now carries its own bar, rate and countdown, and
      // the list lives on Home itself.
      expect(find.text('Transfers'), findsNothing);
      expect(find.text('Collections'), findsNothing);
      expect(AppBottomNav.items.length, 4);

      await tester.tap(find.byKey(const Key('navTab1')));
      await tester.pump();
      expect(find.byType(UserScreen), findsOneWidget);

      await tester.tap(find.byKey(const Key('navTab2')));
      await tester.pump();
      expect(find.byType(PeopleScreen), findsOneWidget);

      await tester.tap(find.byKey(const Key('navTab3')));
      await tester.pump();
      expect(find.byType(SettingsScreen), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('Home states the aggregate active transfer summary',
        (tester) async {
      // The header provides a compact summary while the library owns the
      // single source of truth for each collection.
      await pumpApp(tester, collections: [
        buildCollection(state: 'downloading', downloadMbps: 1.5, uploadMbps: 0.5),
      ]);

      expect(find.textContaining('1 active transfer'), findsOneWidget);
      expect(find.textContaining('1.5 MB/s'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('uses the phone layout narrow and three panes wide',
        (tester) async {
      await pumpApp(tester, collections: [buildCollection()]);

      await pumpApp(tester, size: phoneSize);
      expect(find.byType(AppBottomNav), findsOneWidget);
      expect(find.byKey(const Key('headerPeopleButton')), findsNothing);

      await tester.binding.setSurfaceSize(desktopSize);
      await tester.pumpWidget(const MyApp());
      await tester.pump();
      AppControllers.collections.debugSeed([buildCollection()]);
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
      // beside the list â€” a thinner account of the same collection, plus a
      // button to get from one to the other. The card is the view.
      //
      // It now grows in three taps rather than snapping fully open: mid
      // (graph and actions) for a routine glance, full (peers and files too)
      // for whoever wants everything, and a third tap all the way back
      // closed â€” three light touches instead of one all-or-nothing click.
      await pumpApp(tester, size: desktopSize, collections: [
        buildCollection(id: 'a', name: 'Iceland'),
        buildCollection(id: 'b', name: 'Studio'),
      ]);

      expect(find.text('Open collection'), findsNothing);
      expect(find.byType(CollectionDetail), findsNothing);

      await tester.tap(find.text('Studio').first);
      await tester.pump();

      // Mid: open in place, whole list still around it, but not everything
      // is showing yet â€” peers and files are one tap deeper.
      expect(find.byType(CollectionDetail), findsOneWidget);
      expect(find.text('Invite'), findsOneWidget);
      expect(find.text('Iceland'), findsWidgets);

      await tester.tap(find.text('Studio').first);
      await tester.pump();

      // Full: still the same card, now with everything.
      expect(find.byType(CollectionDetail), findsOneWidget);

      // A third click has nothing left to mean but closing it â€” the card is
      // the only view.
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
      await pumpApp(tester, size: desktopSize, collections: [buildCollection()]);
      await tester.tap(find.byKey(const Key('identityChip')));
      await tester.pump();

      await tester.binding.setSurfaceSize(phoneSize);
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
      await pumpApp(tester, size: desktopSize, collections: [buildCollection()]);

      for (final gone in ['Transfers', 'Settings', 'People']) {
        expect(find.text(gone), findsNothing, reason: '$gone is not a row');
      }
      expect(find.byKey(const Key('headerPeopleButton')), findsOneWidget);
      expect(find.byKey(const Key('headerSettingsButton')), findsOneWidget);

      // The identity chip is the direct way into User; engine settings keep
      // their own header action.
      await tester.tap(find.byKey(const Key('identityChip')));
      await tester.pump();
      expect(find.byType(UserScreen), findsOneWidget);
      expect(find.text('Change name'), findsOneWidget);

      // Tapping the header button for the pane that is already open closes
      // The settings action still toggles back to Home — the same behaviour
      // every pane button gets.
      await tester.tap(find.byKey(const Key('headerSettingsButton')));
      await tester.pump();
      expect(find.byType(SettingsScreen), findsNothing);

      // One primary action beside one field, plus the one thing a paste
      // cannot express. The sidebar's own New share, Join, magnet field,
      // Paste, Add and .torrent picker are all gone â€” the command bar takes any
      // of them as a paste.
      expect(find.text('New share'), findsNothing);
      expect(find.text('Join with a key'), findsNothing);
      expect(find.byKey(const Key('sidebarMagnetField')), findsNothing);
      expect(find.byKey(const Key('commandBarField')), findsOneWidget);
      expect(find.byKey(const Key('addTorrentButton')), findsNothing);
      // A .torrent is a file, so it is the one thing the bar cannot absorb
      // as a paste â€” it keeps an affordance, or desktop loses the capability.
      expect(find.byKey(const Key('commandBarTorrentFile')), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('the command bar dispatches on what was pasted', (tester) async {
      await pumpApp(tester, size: desktopSize, collections: [
        buildCollection(name: 'Iceland trip'),
        buildCollection(id: 'c2', name: 'Band demos'),
      ]);

      final field = find.byKey(const Key('commandBarField'));

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
      await pumpApp(tester, size: desktopSize, collections: [buildCollection()]);

      // Joining announces you to strangers â€” recognising a code opens the
      // join screen with it filled in, it never joins on the paste alone.
      await tester.enterText(
          find.byKey(const Key('commandBarField')), inviteCode('Iceland trip'));
      await tester.pump();
      expect(find.text('JOIN'), findsOneWidget);

      await tester.tap(find.byKey(const Key('commandBarSubmit')));
      await pumpTransition(tester);

      expect(find.byType(JoinCollectionScreen), findsOneWidget);
      expect(find.text('Code recognised'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });



  group('home tab', () {
    testWidgets('is the leftmost destination and returns from another tab',
        (tester) async {
      await pumpApp(tester, collections: []);
      expect(find.text('Home'), findsOneWidget);

      await tester.tap(find.byKey(const Key('navTab3')));
      await tester.pump();
      expect(find.byType(SettingsScreen), findsOneWidget);

      await tester.tap(find.byKey(const Key('navTab0')));
      await tester.pump();

      expect(AppNavigation.tab.value, AppNavigation.homeTab);
      expect(tester.takeException(), isNull);
    });

    testWidgets('unwinds a pushed screen rather than only switching tab',
        (tester) async {
      // The distinction that makes it a Home button and not just a tab: one
      // tap lands you at the start, not one screen shallower.
      await pumpApp(tester);
      await tester.tap(find.byKey(const Key('commandBarField')));
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await pumpTransition(tester);
      await tester.tap(find.byKey(const Key('addShareAction')));
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
}
