import 'test_support.dart';

void main() {
  tearDown(resetTestState);

group('flows still surface backend errors', () {
    testWidgets('torrent screen validates input', (tester) async {
      await tester.binding.setSurfaceSize(phoneSize);
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
      // RustLib isn't initialized, so the add is expected to fail â€” inline
      // error, screen stays open, no uncaught exception.
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byType(AddTorrentScreen), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('join screen recognises a valid code', (tester) async {
      await tester.binding.setSurfaceSize(phoneSize);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(const MaterialApp(home: JoinCollectionScreen()));
      await tester.pump();

      // Invite codes are hex-wrapped (collections.rs::invite_code_for) so the
      // name and address aren't visible in plain text â€” encode the same way.
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
      await tester.binding.setSurfaceSize(phoneSize);
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

