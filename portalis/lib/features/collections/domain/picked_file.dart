import 'dart:io';

/// A native file selected by the UI. Only identity and display metadata live
/// in Dart; Rust owns opening, copying, hashing, and publishing its contents.
class PickedFile {
  const PickedFile({
    required this.name,
    required this.path,
    required this.lengthBytes,
  });

  final String name;
  final String path;
  final int lengthBytes;
}

Future<PickedFile> pickedFileFrom({
  required String name,
  required String? nativePath,
}) async {
  if (nativePath == null || nativePath.isEmpty) {
    throw UnsupportedError('The selected file has no native path');
  }
  return PickedFile(
    name: name,
    path: nativePath,
    lengthBytes: await File(nativePath).length(),
  );
}
