/// One top-level item under the configured download folder.
class StorageEntry {
  const StorageEntry({
    required this.name,
    required this.bytes,
    required this.path,
    this.collectionId,
    this.collectionName,
  });

  final String name;
  final int bytes;
  final String path;
  final String? collectionId;
  final String? collectionName;
}
