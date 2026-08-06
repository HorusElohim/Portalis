import '../../../bridge_generated/collections.dart' as bridge;
import '../../media/domain/media_item.dart';
import '../domain/collection.dart';
import '../domain/collection_import.dart';

/// Converts generated Flutter-Rust Bridge DTOs into the frontend's stable,
/// framework-free collection model. This is the only collection file allowed
/// to know the generated DTO shape.
abstract final class CollectionMapper {
  static Collection fromInfo(bridge.CollectionInfo info) => Collection(
        id: info.id,
        name: info.name,
        kind: switch (info.kind) {
          bridge.CollectionKind.shared => CollectionKind.shared,
          bridge.CollectionKind.torrent => CollectionKind.torrent,
        },
        inviteCode: info.inviteCode,
        collaborators: info.collaborators.map(_collaborator).toList(),
        media: info.media.map(_media).toList(),
        progress: info.progress,
        totalBytes: info.totalBytes.toInt(),
        downloadedBytes: info.downloadedBytes.toInt(),
        uploadedBytes: info.uploadedBytes.toInt(),
        downloadMbps: info.downloadMbps,
        uploadMbps: info.uploadMbps,
        livePeers: info.livePeers,
        torrentPeers: info.torrentPeers,
        pendingMedia: info.pendingMedia,
        etaSecs: info.etaSecs?.toInt(),
        state: info.state,
        ingestion: info.ingestion == null
            ? null
            : CollectionImport(
                stage: info.ingestion!.stage,
                progress: info.ingestion!.progress,
                processedBytes: info.ingestion!.processedBytes.toInt(),
                totalBytes: info.ingestion!.totalBytes.toInt(),
                error: info.ingestion!.error,
              ),
      );

  static Collaborator _collaborator(bridge.CollaboratorInfo info) =>
      Collaborator(
        deviceId: info.deviceId,
        name: info.displayName,
        isAdmin: info.isAdmin,
      );

  static MediaItem _media(bridge.MediaInfo info) => MediaItem(
        label: info.name,
        entryLabel: info.entryName,
        infoHash: info.infoHash,
        localPath: info.absolutePath,
        progress: info.progress,
        sizeBytes: info.lengthBytes.toInt(),
        downloadedBytes: info.downloadedBytes.toInt(),
        fetched: info.fetched,
        addedBy: info.addedBy,
      );
}
