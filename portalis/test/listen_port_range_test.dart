import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/features/settings/domain/listen_port_range.dart';

void main() {
  test('parses both plain and typographic port range separators', () {
    final plain = parseListenPortRange('6881-6999');
    final typographic = parseListenPortRange('6881 – 6999');

    expect((plain?.start, plain?.end), (6881, 6999));
    expect((typographic?.start, typographic?.end), (6881, 6999));
  });

  test('rejects malformed, reversed, and out-of-range ports', () {
    for (final value in ['', '0-1', '6999-6881', '6881-', '70000-70001']) {
      expect(parseListenPortRange(value), isNull, reason: value);
    }
  });
}
