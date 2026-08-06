import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

import '../domain/peer_observation.dart';

abstract interface class PeerHistoryStore {
  Future<List<PeerObservation>> load();
  Future<void> save(List<PeerObservation> peers);
}

class SharedPreferencesPeerHistoryStore implements PeerHistoryStore {
  const SharedPreferencesPeerHistoryStore();

  static const _key = 'collections.peer_history.v1';

  @override
  Future<List<PeerObservation>> load() async {
    final preferences = await SharedPreferences.getInstance();
    final raw = preferences.getString(_key);
    if (raw == null) return const [];

    final decoded = jsonDecode(raw);
    if (decoded is! List) return const [];
    return [
      for (final item in decoded)
        if (item is Map<String, dynamic>)
          PeerObservation(
            collectionId: item['collectionId'] as String,
            collectionName: item['collectionName'] as String,
            address: item['address'] as String,
            lastSeen: DateTime.parse(item['lastSeen'] as String),
          ),
    ];
  }

  @override
  Future<void> save(List<PeerObservation> peers) async {
    final preferences = await SharedPreferences.getInstance();
    await preferences.setString(
      _key,
      jsonEncode([
        for (final peer in peers)
          {
            'collectionId': peer.collectionId,
            'collectionName': peer.collectionName,
            'address': peer.address,
            'lastSeen': peer.lastSeen.toUtc().toIso8601String(),
          },
      ]),
    );
  }
}
