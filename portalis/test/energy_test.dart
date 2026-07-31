import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/models.dart';
import 'package:portalis/theme.dart';

Collection _c({
  String state = 'seeding',
  double down = 0,
  double up = 0,
  List<MediaItem> media = const [
    MediaItem(label: 'a.jpg', infoHash: 'aa'),
  ],
}) =>
    Collection(
      id: 'c1',
      name: 'Trip',
      kind: CollectionKind.shared,
      collaborators: const [],
      media: media,
      state: state,
      downloadMbps: down,
      uploadMbps: up,
    );

void main() {
  group('is it actually being shared', () {
    test('seeding with content is sharing', () {
      expect(_c(state: 'seeding').isSharing, isTrue);
    });

    test('an empty collection is not sharing, whatever its state says', () {
      // Nothing to serve means nothing is being served — telling someone
      // their photos are available when no torrent is live is the exact
      // ambiguity this replaces.
      expect(_c(state: 'seeding', media: const []).isSharing, isFalse);
    });

    test('downloading or pending is not sharing', () {
      expect(_c(state: 'downloading').isSharing, isFalse);
      expect(_c(state: 'pending').isSharing, isFalse);
      expect(_c(state: 'empty').isSharing, isFalse);
    });
  });

  group('glow is earned, never decorative', () {
    test('an idle, unshared collection has none', () {
      expect(_c(state: 'pending').glow, GlowLevel.none);
      expect(_c(state: 'empty', media: const []).glow, GlowLevel.none);
    });

    test('shared and standing by glows calmly', () {
      expect(_c(state: 'seeding').glow, GlowLevel.calm);
    });

    test('transferring glows brighter the faster it goes', () {
      expect(_c(state: 'downloading', down: 1).glow, GlowLevel.active);
      expect(_c(state: 'downloading', down: 12).glow, GlowLevel.vivid);
      // Upload counts as much as download — sending is just as alive.
      expect(_c(state: 'seeding', up: 9).glow, GlowLevel.vivid);
    });
  });

  group('glow tokens', () {
    test('none is genuinely nothing — no shadow, structural border', () {
      final glow = Glow.of(GlowLevel.none);
      expect(glow.isVisible, isFalse);
      expect(glow.shadows, isEmpty);
      expect(glow.border.top.color, AppColors.border);
    });

    test('intensity increases monotonically across the levels', () {
      // The table is the single place the app's energy is tuned, so its
      // ordering has to hold or "brighter" stops meaning "more active".
      final levels = [GlowLevel.calm, GlowLevel.active, GlowLevel.vivid]
          .map(Glow.of)
          .toList();
      for (var i = 1; i < levels.length; i++) {
        expect(levels[i].borderOpacity, greaterThan(levels[i - 1].borderOpacity));
        expect(levels[i].shadowOpacity, greaterThan(levels[i - 1].shadowOpacity));
        expect(levels[i].blur, greaterThan(levels[i - 1].blur));
      }
    });

    test('takes whichever colour it is given, so torrents glow ember', () {
      final glow = Glow.of(GlowLevel.active, color: AppColors.ember);
      // Compare the RGB channels; the alpha is the level's own opacity.
      expect(glow.border.top.color.toARGB32() & 0x00FFFFFF,
          AppColors.ember.toARGB32() & 0x00FFFFFF);
      expect(glow.shadows.single.color.toARGB32() & 0x00FFFFFF,
          AppColors.ember.toARGB32() & 0x00FFFFFF);
    });
  });
}
