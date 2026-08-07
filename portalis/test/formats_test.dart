import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/features/media/application/media_formats.dart';
import 'package:portalis/screens/settings/formats.dart';

void main() {
  setUp(MediaFormats.resetToDefaults);

  group('registry', () {
    test('classifies by extension, case-insensitively', () {
      expect(kindOf('holiday.JPG'), MediaKind.image);
      expect(kindOf('clip.mp4'), MediaKind.video);
      expect(kindOf('take_4.flac'), MediaKind.audio);
      expect(kindOf('film.srt'), MediaKind.subtitle);
      expect(kindOf('notes.md'), MediaKind.document);
      expect(kindOf('backup.zip'), MediaKind.archive);
    });

    test('an unknown extension resolves rather than throwing', () {
      // Callers must never have to special-case an unrecognised file.
      final format = MediaFormats.resolve('mystery.qqq');
      expect(format, MediaFormats.unknown);
      expect(format.kind, MediaKind.other);
      expect(format.preview, PreviewSupport.externalOnly);
      expect(format.previewNote, isNotNull);
    });

    test('a file with no extension at all still resolves', () {
      expect(MediaFormats.resolve('LICENSE'), MediaFormats.unknown);
      expect(extensionOf('LICENSE'), '');
    });

    test('registering a format makes it discoverable everywhere at once', () {
      const raw = MediaFormat(
        extensions: ['dng', 'cr2'],
        label: 'Camera raw',
        kind: MediaKind.image,
        preview: PreviewSupport.externalOnly,
        previewNote: 'Raw files need a dedicated decoder.',
      );

      MediaFormats.register(raw);

      // One call, and every lookup path knows about it — that is the whole
      // point of the registry being open.
      expect(MediaFormats.resolve('IMG_0001.dng'), same(raw));
      expect(MediaFormats.resolve('IMG_0001.CR2'), same(raw));
      expect(kindOf('IMG_0001.dng'), MediaKind.image);
      expect(MediaFormats.all, contains(raw));
      expect(MediaFormats.ofKind(MediaKind.image), contains(raw));
      expect(MediaFormats.knownExtensions, containsAll({'dng', 'cr2'}));
    });

    test('a later registration overrides a built-in extension', () {
      const override = MediaFormat(
        extensions: ['png'],
        label: 'PNG (overridden)',
        kind: MediaKind.document,
        preview: PreviewSupport.externalOnly,
        previewNote: 'test override',
      );

      MediaFormats.register(override);

      expect(MediaFormats.resolve('a.png').label, 'PNG (overridden)');
      // ...and resetting puts the built-in back, so one test can't leak into
      // the next.
      MediaFormats.resetToDefaults();
      expect(MediaFormats.resolve('a.png').label, 'PNG image');
    });

    test('every format that cannot be previewed explains why', () {
      // A dead end without a reason is exactly the kind of silent gap this
      // registry exists to prevent.
      for (final f in MediaFormats.all) {
        if (f.preview == PreviewSupport.externalOnly) {
          expect(f.previewNote, isNotNull,
              reason: '${f.label} has no preview and no explanation');
          expect(f.previewNote, isNotEmpty);
        }
      }
    });

    test('no extension is claimed by two formats', () {
      final seen = <String, String>{};
      for (final f in MediaFormats.all) {
        for (final ext in f.extensions) {
          expect(seen.containsKey(ext), isFalse,
              reason: '.$ext claimed by both ${seen[ext]} and ${f.label}');
          seen[ext] = f.label;
        }
      }
    });

    test('format accents never collide with the reserved signal colours', () {
      // Mint means "moving" and ember means "torrent". A file type is
      // neither, so it must not borrow either colour.
      for (final f in MediaFormats.all) {
        expect(f.accent, isNot(const Color(0xFF5CE7A3)));
        expect(f.accent, isNot(const Color(0xFFF0B357)));
      }
    });
  });

  group('sharing', () {
    test('HEIC originals use the native image decoder without copying', () {
      final heic = MediaFormats.resolve('IMG_1234.HEIC');

      expect(heic.kind, MediaKind.image);
      expect(heic.preview, PreviewSupport.nativeImage);
      expect(hasInAppPreview('IMG_1234.HEIC'), isTrue);
    });
  });

  group('the viewer obeys the registry', () {
    test('common video containers request inline playback', () {
      expect(MediaFormats.resolve('a.mp4').preview, PreviewSupport.player);
      expect(kindOf('a.mkv'), MediaKind.video);
      expect(MediaFormats.resolve('a.mkv').preview, PreviewSupport.player);
      expect(MediaFormats.resolve('a.avi').preview, PreviewSupport.player);
    });

    test('inline image decoding is a capability, not a kind', () {
      expect(MediaFormats.resolve('a.png').preview, PreviewSupport.image);
      // Audio is a kind with no inline renderer at all.
      expect(kindOf('a.mp3'), MediaKind.audio);
      expect(hasInAppPreview('a.mp3'), isFalse);
      expect(hasInAppPreview('a.png'), isTrue);
    });
  });

  group('formats screen', () {
    testWidgets('lists the registry, and finds a newly registered type',
        (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 844));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      MediaFormats.register(const MediaFormat(
        extensions: ['xyz'],
        label: 'Unit test format',
        kind: MediaKind.document,
        preview: PreviewSupport.text,
      ));

      await tester.pumpWidget(const MaterialApp(home: FormatsScreen()));
      await tester.pump();

      expect(find.text('FILE FORMATS'), findsOneWidget);
      expect(find.text('JPEG image'), findsWidgets);

      // The screen is generated from the registry, so a type registered a
      // moment ago is already on it.
      await tester.enterText(find.byKey(const Key('formatSearchField')), 'xyz');
      await tester.pump();
      expect(find.text('Unit test format'), findsOneWidget);
      expect(find.text('JPEG image'), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a type with no in-app viewer is labelled, not hidden',
        (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 844));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      await tester.pumpWidget(const MaterialApp(home: FormatsScreen()));
      await tester.pump();

      await tester.enterText(find.byKey(const Key('formatSearchField')), 'pdf');
      await tester.pump();

      expect(find.text('PDF document'), findsOneWidget);
      expect(find.text('OPENS OUT'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });
}
