import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/bridge_generated/collections.dart' as bridge;
import 'package:portalis/bridge_generated/torrent.dart' as torrent_bridge;
import 'package:portalis/design/design.dart';
import 'package:portalis/features/collections/data/collection_mapper.dart';
import 'package:portalis/features/collections/domain/collection.dart';
import 'package:portalis/features/collections/presentation/collection_contents.dart';
import 'package:portalis/features/media/domain/media_item.dart';
import 'package:portalis/features/media/presentation/media_piece_frame.dart';

void main() {
  test('bridge piece runs retain their byte positions and real peers', () {
    final mapped = CollectionMapper.fromInfo(
      bridge.CollectionInfo(
        id: 'torrent',
        name: 'Atlantis',
        kind: bridge.CollectionKind.torrent,
        collaborators: const [],
        media: [
          bridge.MediaInfo(
            name: 'part-2.mkv',
            entryName: 'Atlantis',
            infoHash: 'aa',
            lengthBytes: BigInt.from(100),
            downloadedBytes: BigInt.from(20),
            progress: 0.2,
            pieceRuns: [
              torrent_bridge.PieceRun(
                offsetBytes: BigInt.from(60),
                lengthBytes: BigInt.from(10),
                verified: false,
                peers: const ['203.0.113.5:6881'],
              ),
            ],
            fetched: true,
          ),
        ],
        progress: 0.2,
        totalBytes: BigInt.from(100),
        downloadedBytes: BigInt.from(20),
        uploadedBytes: BigInt.zero,
        downloadMbps: 1,
        uploadMbps: 0,
        livePeers: 1,
        torrentPeers: const ['203.0.113.5:6881'],
        pendingMedia: 0,
        state: 'downloading',
      ),
    );

    final run = mapped.media.single.pieceRuns.single;
    expect(run.offsetBytes, 60);
    expect(run.lengthBytes, 10);
    expect(run.isDownloading, isTrue);
    expect(run.peers, ['203.0.113.5:6881']);
  });

  testWidgets('media frame projects byte ranges onto the perimeter',
      (tester) async {
    const media = MediaItem(
      label: 'part-2.mkv',
      infoHash: 'aa',
      sizeBytes: 100,
      downloadedBytes: 20,
      progress: 0.2,
      pieceRuns: [
        MediaPieceRun(
          offsetBytes: 0,
          lengthBytes: 20,
          verified: true,
        ),
        MediaPieceRun(
          offsetBytes: 60,
          lengthBytes: 10,
          verified: false,
          peers: ['203.0.113.5:6881', '203.0.113.9:51413'],
        ),
      ],
    );

    await tester.pumpWidget(
      const MaterialApp(
        home: SizedBox(
          width: 200,
          height: 120,
          child: MediaPieceFrame(
            media: media,
            color: Colors.amber,
            borderRadius: BorderRadius.all(Radius.circular(8)),
            child: ColoredBox(color: Colors.black),
          ),
        ),
      ),
    );

    final perimeter = tester.widget<PerimeterProgress>(
      find.byType(PerimeterProgress),
    );
    expect(perimeter.segments, const [
      PerimeterSegment(start: 0, extent: 0.2),
      PerimeterSegment(
        start: 0.6,
        extent: 0.1,
        active: true,
        workerCount: 2,
      ),
    ]);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('worker emphasis respects reduced motion', (tester) async {
    const media = MediaItem(
      label: 'part-2.mkv',
      infoHash: 'aa',
      sizeBytes: 100,
      progress: 0.2,
      pieceRuns: [
        MediaPieceRun(
          offsetBytes: 60,
          lengthBytes: 10,
          verified: false,
          peers: ['203.0.113.5:6881'],
        ),
      ],
    );

    await tester.pumpWidget(
      const MaterialApp(
        home: MediaQuery(
          data: MediaQueryData(disableAnimations: true),
          child: MediaPieceFrame(
            media: media,
            color: Colors.amber,
            borderRadius: BorderRadius.all(Radius.circular(8)),
            child: SizedBox(width: 200, height: 120),
          ),
        ),
      ),
    );
    await tester.pump();

    expect(tester.binding.transientCallbackCount, 0);
  });

  testWidgets('collection media previews stay tiny on wide surfaces',
      (tester) async {
    const media = MediaItem(
      label: 'part-2.mkv',
      infoHash: 'aa',
      sizeBytes: 100,
      downloadedBytes: 20,
      progress: 0.2,
    );
    const collection = Collection(
      id: 'atlantis',
      name: 'Atlantis',
      kind: CollectionKind.torrent,
      collaborators: [],
      media: [media],
      state: 'downloading',
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Align(
            alignment: Alignment.topLeft,
            child: SizedBox(
              width: 1000,
              child: CollectionContents(
                collection: collection,
                onOpenMedia: (_) {},
              ),
            ),
          ),
        ),
      ),
    );

    final previewSize = tester.getSize(find.byType(MediaPieceFrame));
    expect(previewSize.width, lessThanOrEqualTo(84));
    expect(previewSize.height, lessThan(84));
    expect(tester.takeException(), isNull);
  });
}
