/// A network peer seen on one collection.
///
/// This is deliberately not a collaborator. A torrent address has no signed
/// identity, so it can only be remembered as an observation with a timestamp.
class PeerObservation {
  const PeerObservation({
    required this.collectionId,
    required this.collectionName,
    required this.address,
    required this.lastSeen,
  });

  final String collectionId;
  final String collectionName;
  final String address;
  final DateTime lastSeen;
}
