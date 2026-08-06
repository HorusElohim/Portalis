import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

import '../domain/transfer_history.dart';

abstract interface class TransferHistoryStore {
  Future<Map<String, TransferHistory>> load();
  Future<void> save(Map<String, TransferHistory> histories);
}

class SharedPreferencesTransferHistoryStore implements TransferHistoryStore {
  const SharedPreferencesTransferHistoryStore();

  static const _key = 'collections.transfer_history.v1';

  @override
  Future<Map<String, TransferHistory>> load() async {
    final preferences = await SharedPreferences.getInstance();
    final raw = preferences.getString(_key);
    if (raw == null) return const {};

    final decoded = jsonDecode(raw);
    if (decoded is! Map<String, dynamic>) return const {};
    final histories = <String, TransferHistory>{};
    for (final entry in decoded.entries) {
      final value = entry.value;
      if (value is! Map<String, dynamic>) continue;
      final samples = value['samples'];
      if (samples is! List) continue;
      histories[entry.key] = TransferHistory.restore(
        startedAt: DateTime.parse(value['startedAt'] as String),
        completedAt: (value['completedAt'] as String?) == null
            ? null
            : DateTime.parse(value['completedAt'] as String),
        samples: [
          for (final sample in samples)
            if (sample is Map<String, dynamic>)
              TransferSample(
                at: DateTime.parse(sample['at'] as String),
                downloadMbps: (sample['downloadMbps'] as num).toDouble(),
                uploadMbps: (sample['uploadMbps'] as num).toDouble(),
                progress: (sample['progress'] as num).toDouble(),
              ),
        ],
      );
    }
    return histories;
  }

  @override
  Future<void> save(Map<String, TransferHistory> histories) async {
    final preferences = await SharedPreferences.getInstance();
    await preferences.setString(
      _key,
      jsonEncode({
        for (final entry in histories.entries)
          entry.key: {
            'startedAt': entry.value.startedAt.toUtc().toIso8601String(),
            'completedAt': entry.value.completedAt?.toUtc().toIso8601String(),
            'samples': [
              for (final sample in entry.value.samples)
                {
                  'at': sample.at.toUtc().toIso8601String(),
                  'downloadMbps': sample.downloadMbps,
                  'uploadMbps': sample.uploadMbps,
                  'progress': sample.progress,
                },
            ],
          },
      }),
    );
  }
}
