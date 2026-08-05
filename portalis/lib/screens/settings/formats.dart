import 'package:flutter/material.dart';

import '../../design/design.dart';
import '../../media/formats.dart';
import '../../theme.dart';

/// Every file type Portalis knows, and exactly what it does with each.
///
/// Generated from [MediaFormats] rather than written out, so it cannot claim
/// support the app doesn't have: if a format isn't registered it doesn't
/// appear here, and if its preview is [PreviewSupport.externalOnly] this says
/// so, with the reason. Registering a new type makes it show up here with no
/// edit to this file.
///
/// The point is transparency about the user's own data — what gets converted
/// on the way in, what can be viewed in-app, and what is handed to the
/// system untouched.
class FormatsScreen extends StatefulWidget {
  const FormatsScreen({super.key, this.embedded = false, this.onBack});

  /// Set when this replaces the You pane in place on desktop rather than
  /// being pushed over it — see [AppScreen].
  final bool embedded;

  /// Called instead of popping a route. Only meaningful when [embedded]:
  /// there is no route to pop there, so the caller supplies its own
  /// "collapse back to You" callback.
  final VoidCallback? onBack;

  @override
  State<FormatsScreen> createState() => _FormatsScreenState();
}

class _FormatsScreenState extends State<FormatsScreen> {
  String _query = '';

  @override
  Widget build(BuildContext context) {
    final all = MediaFormats.all;
    final q = _query.trim().toLowerCase();
    final matches = q.isEmpty
        ? all
        : all
            .where((f) =>
                f.label.toLowerCase().contains(q) ||
                f.extensions.any((e) => e.contains(q)))
            .toList();

    final converted = all.where((f) => f.isTransformedOnShare).length;
    final viewable =
        all.where((f) => f.preview != PreviewSupport.externalOnly).length;

    return AppScreen(
      title: 'File formats',
      subtitle: Text('${all.length} types · $viewable viewable in the app · '
          '$converted converted when shared'),
      embedded: widget.embedded,
      forceShowBack: true,
      onBack: widget.onBack,
      body: Column(
        children: [
          // Above the scroll rather than inside it: the field that filters
          // the list shouldn't scroll away from the list it filters.
          Padding(
            padding:
                const EdgeInsets.fromLTRB(kScreenGutter, 0, kScreenGutter, 4),
            child: _SearchField(onChanged: (v) => setState(() => _query = v)),
          ),
          Expanded(
            child: CustomScrollView(
              slivers: [
                if (matches.isEmpty)
                  SliverToBoxAdapter(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(
                          kScreenGutter, 40, kScreenGutter, 0),
                      child: Center(
                        child: Text(
                          'Nothing matches "$_query".\nUnlisted types are '
                          'still shared byte-for-byte — they just open in '
                          'your system app.',
                          textAlign: TextAlign.center,
                          style: AppText.body(
                              color: AppColors.textDim, height: 1.6),
                        ),
                      ),
                    ),
                  )
                else
                  for (final kind in MediaKind.values)
                    ..._kindSection(
                        kind, matches.where((f) => f.kind == kind).toList()),
                SliverToBoxAdapter(
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(
                        kScreenGutter, 26, kScreenGutter, 28),
                    child: Text(
                      'Anything not listed is still shareable. Portalis never '
                      'inspects or re-encodes a file unless a conversion is '
                      'shown above — everything else is seeded exactly as it '
                      'is on your disk.',
                      style: AppText.secondary(height: 1.6),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  List<Widget> _kindSection(MediaKind kind, List<MediaFormat> formats) {
    if (formats.isEmpty) return const [];
    return [
      SliverToBoxAdapter(
        child: Padding(
          padding:
              const EdgeInsets.fromLTRB(kScreenGutter, 22, kScreenGutter, 8),
          child: SectionLabel('${_kindLabel(kind).toUpperCase()} · '
              '${formats.length}'),
        ),
      ),
      SliverPadding(
        padding: const EdgeInsets.symmetric(horizontal: kScreenGutter),
        sliver: SliverList.separated(
          itemCount: formats.length,
          separatorBuilder: (_, __) => const SizedBox(height: 8),
          itemBuilder: (context, i) => _FormatCard(format: formats[i]),
        ),
      ),
    ];
  }

  static String _kindLabel(MediaKind kind) => switch (kind) {
        MediaKind.image => 'Images',
        MediaKind.video => 'Video',
        MediaKind.audio => 'Audio',
        MediaKind.subtitle => 'Subtitles',
        MediaKind.document => 'Documents',
        MediaKind.archive => 'Archives',
        MediaKind.other => 'Other',
      };
}

class _SearchField extends StatelessWidget {
  const _SearchField({required this.onChanged});

  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14),
      decoration: BoxDecoration(
        color: AppColors.surfaceRaised,
        borderRadius: BorderRadius.circular(AppRadius.control),
        border: Border.all(color: AppColors.border),
      ),
      child: Row(
        children: [
          const Icon(Icons.search, size: 16, color: AppColors.textGhost),
          const SizedBox(width: 10),
          Expanded(
            child: TextField(
              key: const Key('formatSearchField'),
              onChanged: onChanged,
              style: AppText.body(),
              decoration: InputDecoration(
                isDense: true,
                border: InputBorder.none,
                hintText: 'Search formats',
                hintStyle: AppText.body(color: AppColors.textGhost),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// One format, with its real capabilities.
class _FormatCard extends StatelessWidget {
  const _FormatCard({required this.format});

  final MediaFormat format;

  @override
  Widget build(BuildContext context) {
    return SurfaceCard(
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                width: 38,
                height: 38,
                decoration: BoxDecoration(
                  color: format.accent.withValues(alpha: 0.13),
                  borderRadius: BorderRadius.circular(AppRadius.inner),
                ),
                child:
                    Icon(format.effectiveIcon, size: 18, color: format.accent),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(format.label, style: AppText.cardTitle()),
                    const SizedBox(height: 3),
                    Text(
                      format.extensions.map((e) => '.$e').join('  '),
                      style: monoLabel(size: 10.5, letterSpacing: 0.2),
                    ),
                  ],
                ),
              ),
              _PreviewBadge(support: format.preview),
            ],
          ),
          if (format.previewNote != null) ...[
            const SizedBox(height: 10),
            _Note(
              icon: Icons.open_in_new_rounded,
              text: format.previewNote!,
            ),
          ],
          if (format.shareNote != null) ...[
            const SizedBox(height: 8),
            _Note(
              icon: Icons.autorenew,
              // Conversion is the one thing that changes the user's bytes, so
              // it gets the emphatic treatment rather than a footnote.
              color: AppColors.ember,
              text: format.shareNote!,
            ),
          ],
        ],
      ),
    );
  }
}

class _PreviewBadge extends StatelessWidget {
  const _PreviewBadge({required this.support});

  final PreviewSupport support;

  @override
  Widget build(BuildContext context) {
    final (label, color) = switch (support) {
      PreviewSupport.image => ('VIEW', AppColors.hues[1]),
      PreviewSupport.player => ('PLAY', AppColors.hues[3]),
      PreviewSupport.text => ('READ', AppColors.hues[2]),
      // Not a failure state, so not danger-coloured — it's just where the
      // file opens.
      PreviewSupport.externalOnly => ('OPENS OUT', null),
    };
    return StatusBadge(label: label, color: color);
  }
}

class _Note extends StatelessWidget {
  const _Note({required this.icon, required this.text, this.color});

  final IconData icon;
  final String text;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final c = color ?? AppColors.textFaint;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(icon, size: 13, color: c),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            text,
            style: AppText.caption(color: c, height: 1.45),
          ),
        ),
      ],
    );
  }
}
