import 'test_support.dart';

void main() {
  tearDown(resetTestState);

group('people and settings', () {
    testWidgets('the User profile counts distinct collaborators',
        (tester) async {
      const ana = Collaborator(deviceId: 'dev-ana', name: 'Ana');
      const jonas = Collaborator(deviceId: 'dev-jonas', name: 'Jonas');
      const rosa = Collaborator(deviceId: 'dev-rosa', name: 'Rosa');
      await pumpApp(tester, collections: [
        // Ana is in both collections; she is one person, not two. Four
        // memberships across two collections, three actual people â€” the
        // counts differ deliberately so this can only pass by deduplicating.
        buildCollection(id: 'a', collaborators: [ana, jonas]),
        buildCollection(id: 'b', collaborators: [ana, rosa]),
      ]);
      await tester.tap(find.byKey(const Key('navTab1')));
      await tester.pump();

      expect(find.text('PEOPLE'), findsOneWidget);
      expect(find.text('3'), findsOneWidget);
      expect(find.text('COLLECTIONS'), findsOneWidget);
      expect(find.text('2'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('shows anonymous torrent peers from shared collections',
        (tester) async {
      await pumpApp(tester, collections: [
        buildCollection(
          id: 'shared-with-peer',
          torrentPeers: const ['198.51.100.7:6881'],
        ),
      ]);

      await tester.tap(find.byKey(const Key('navTab2')));
      await tester.pump();

      expect(find.text('198.51.100.7:6881'), findsOneWidget);
      expect(find.textContaining('Torrent peer'), findsOneWidget);
      expect(find.byTooltip('Forget peer'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    // People has gone missing on a platform twice: first it existed only as
    // a desktop sidebar pane, then as a row so far down the old You screen
    // that it was past the address, the identity notice and File formats.
    // Both times it was reachable in principle and unfindable in practice.
    // It is now its own bottom tab on both layouts — direct rather than
    // reachable — and the count in User's profile is a shortcut onto that
    // same tab, not a second route to a second copy of it.
    testWidgets(
        'People is its own tab, and the User profile count is a shortcut to it',
        (tester) async {
      const ana = Collaborator(deviceId: 'dev-ana', name: 'Ana');
      await pumpApp(tester, collections: [
        buildCollection(id: 'a', collaborators: [ana]),
      ]);

      // Direct: no intermediate screen to lose it behind.
      await tester.tap(find.byKey(const Key('navTab2')));
      await tester.pump();
      expect(find.byType(PeopleScreen), findsOneWidget);
      expect(find.text('Ana'), findsOneWidget);

      // The User profile's count still works, but as a shortcut onto the
      // People tab rather than a push â€” tapping it selects the shared tab.
      await tester.tap(find.byKey(const Key('navTab1')));
      await tester.pump();
      await tester.tap(find.text('PEOPLE'));
      await pumpTransition(tester);

      expect(find.byType(PeopleScreen), findsOneWidget);
      expect(AppNavigation.tab.value, AppNavigation.peopleTab);
      expect(tester.takeException(), isNull);
    });
  });



  group('settings', () {
    testWidgets('hides engine internals behind Network & engine',
        (tester) async {
      await tester.binding.setSurfaceSize(phoneSize);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      AppControllers.settings.debugSeed(buildEngineSettings());
      await tester.pumpWidget(const MaterialApp(home: SettingsScreen()));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));

      // Advanced rows are not on the first screen.
      expect(find.text('Network & engine'), findsOneWidget);
      expect(find.text('ENGINE EFFICIENCY'), findsOneWidget);
      expect(find.text('Listen ports'), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('the advanced screen exposes the construction-time rows',
        (tester) async {
      await tester.binding.setSurfaceSize(phoneSize);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      AppControllers.settings.debugSeed(buildEngineSettings());
      await tester.pumpWidget(
        const MaterialApp(home: SettingsScreen(advanced: true)),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));

      expect(find.text('Listen ports'), findsOneWidget);
      expect(find.text('Disable DHT'), findsOneWidget);
      // Profile â€” the old You screen's content â€” is scoped to Basic; the
      // engine internals drill-down has nothing to do with identity.
      expect(find.text('Change name'), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });
}
