// This is a basic Flutter widget test.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/main.dart';

void main() {
  testWidgets('Renders basic UI', (tester) async {
    // Build the app without triggering Rust calls.
    await tester.pumpWidget(const MyApp(loadOnStart: false));

    // App bar title present
    expect(find.text('Portalis'), findsOneWidget);

    // Body has the label prefix
    expect(find.textContaining('Rust says:'), findsOneWidget);
  });
}
