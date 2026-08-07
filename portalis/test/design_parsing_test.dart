import 'test_support.dart';

void main() {
  tearDown(resetTestState);

  group('design system', () {
    test('the signal accent is the mint the design specifies', () {
      expect(AppColors.signal, const Color(0xFF5CE7A3));
      expect(AppColors.ember, const Color(0xFFF0B357));
    });

    test('per-collection hues never collide with signal or ember', () {
      // A collection's identity colour must not be mistakable for
      // "transferring" or "torrent" â€” that is the whole point of reserving
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
      expect(PasteKind.of(inviteCode('Iceland trip')), PasteKind.invite);
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
}
