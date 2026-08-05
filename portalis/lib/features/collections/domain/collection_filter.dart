import 'collection.dart';

/// The small set of library views users can choose without changing the
/// collection source of truth.
enum CollectionFilter { all, sharing, receiving }

extension CollectionFilterMatch on CollectionFilter {
  String get label => switch (this) {
        CollectionFilter.all => 'All collections',
        CollectionFilter.sharing => 'Sharing',
        CollectionFilter.receiving => 'Receiving',
      };

  bool includes(Collection collection) => switch (this) {
        CollectionFilter.all => true,
        CollectionFilter.sharing => collection.state == 'seeding',
        CollectionFilter.receiving => collection.state == 'downloading',
      };
}
