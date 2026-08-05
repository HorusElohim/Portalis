// Cross-feature transient feedback primitive.

import 'dart:async';

import 'package:flutter/material.dart';

import '../theme.dart';

/// How much the message matters, which is the only thing that decides its
/// colour.
///
/// Severity colour is allowed to reuse [AppColors.signal] and
/// [AppColors.ember] here, even though those are otherwise reserved for
/// "data is moving" and "torrent". A toast is transient and carries its own
/// text: it appears, says what happened, and leaves. The reservation exists
/// so a *persistent* indicator — a row, a badge, a background — can be read
/// at a glance without labels, and a toast is never read that way.
enum ToastSeverity {
  /// Something happened that the user asked for. Neutral.
  info,

  /// It worked.
  success,

  /// It worked, but not entirely, or it needs a follow-up.
  warning,

  /// It failed.
  error,
}

extension _SeverityStyle on ToastSeverity {
  Color get color => switch (this) {
        ToastSeverity.info => AppColors.textDim,
        ToastSeverity.success => AppColors.signal,
        ToastSeverity.warning => AppColors.ember,
        ToastSeverity.error => AppColors.danger,
      };

  IconData get icon => switch (this) {
        ToastSeverity.info => Icons.info_outline,
        ToastSeverity.success => Icons.check_circle_outline,
        ToastSeverity.warning => Icons.error_outline,
        ToastSeverity.error => Icons.cancel_outlined,
      };
}

/// Shows a toast that rises and settles like a balloon.
///
/// Replaces `ScaffoldMessenger.showSnackBar`, which docks a hard-edged bar to
/// the bottom of the screen — under the nav bar on mobile, and with no way to
/// carry severity. These float above everything, stack when several arrive,
/// and dismiss on tap.
///
/// Requires a [ToastScope] above it, which `MaterialApp.builder` installs once
/// for the whole app. Deliberately *not* an `OverlayEntry`: entries inserted
/// into the root overlay painted correctly but never received pointer events —
/// taps fell straight through to the page beneath, so the toast looked
/// interactive and wasn't. A widget in the normal tree has no such problem.
void showToast(
  BuildContext context,
  String message, {
  ToastSeverity severity = ToastSeverity.info,
  Duration? duration,
}) {
  final scope = ToastScope.maybeOf(context);
  // No scope means no app around us — a bare widget test, typically. Drop the
  // message rather than throwing inside what is usually an error path.
  if (scope == null) return;

  scope.add(
    _ToastMessage(
      text: message,
      severity: severity,
      // Long messages need longer to read. Backend errors are the long ones,
      // and they're exactly what shouldn't vanish mid-sentence.
      duration: duration ??
          Duration(
              milliseconds: (2200 + message.length * 45).clamp(2200, 7000)),
    ),
  );
}

@immutable
class _ToastMessage {
  const _ToastMessage({
    required this.text,
    required this.severity,
    required this.duration,
  });

  final String text;
  final ToastSeverity severity;
  final Duration duration;
}

/// Hosts the live stack of toasts. Installed once, above the navigator, via
/// `MaterialApp.builder` — so a toast survives the navigation that triggered
/// it (creating a collection pops the screen, then reports success).
class ToastScope extends StatefulWidget {
  const ToastScope({super.key, required this.child});

  final Widget child;

  static ToastScopeState? maybeOf(BuildContext context) =>
      context.findAncestorStateOfType<ToastScopeState>();

  @override
  State<ToastScope> createState() => ToastScopeState();
}

class ToastScopeState extends State<ToastScope> {
  final List<_ToastMessage> _messages = [];

  /// Library-private on purpose: [showToast] is the only entry point, so the
  /// message type never has to become part of the public surface.
  // ignore: library_private_types_in_public_api
  void add(_ToastMessage message) {
    if (!mounted) return;
    setState(() {
      _messages.add(message);
      // Three is enough to show a burst without burying the screen; the
      // oldest drops off, like the earliest balloon drifting away.
      if (_messages.length > 3) _messages.removeAt(0);
    });
  }

  void _remove(_ToastMessage message) {
    if (!mounted) return;
    setState(() => _messages.remove(message));
  }

  @override
  Widget build(BuildContext context) {
    final media = MediaQuery.of(context);
    return Stack(
      children: [
        widget.child,
        if (_messages.isNotEmpty)
          Positioned(
            left: 0,
            right: 0,
            // Clear of the bottom nav, which the old docked SnackBar sat under.
            bottom: media.padding.bottom + 88,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final m in _messages)
                  _Balloon(
                    key: ObjectKey(m),
                    message: m,
                    onDone: () => _remove(m),
                  ),
              ],
            ),
          ),
      ],
    );
  }
}

/// A single toast. Rises with a little overshoot, drifts while it waits, then
/// lifts away.
class _Balloon extends StatefulWidget {
  const _Balloon({super.key, required this.message, required this.onDone});

  final _ToastMessage message;
  final VoidCallback onDone;

  @override
  State<_Balloon> createState() => _BalloonState();
}

class _BalloonState extends State<_Balloon> with TickerProviderStateMixin {
  late final AnimationController _entry = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 460),
    reverseDuration: const Duration(milliseconds: 260),
  );

  /// The idle drift. Only ever runs while a toast is on screen — seconds at a
  /// time, never in the background.
  AnimationController? _drift;
  Timer? _timer;
  bool _leaving = false;

  bool get _reduceMotion => MediaQuery.disableAnimationsOf(context);

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_entry.status == AnimationStatus.dismissed) {
      _entry.forward();
      if (!_reduceMotion) {
        _drift = AnimationController(
          vsync: this,
          duration: const Duration(milliseconds: 2600),
        )..repeat(reverse: true);
      }
      _timer = Timer(widget.message.duration, _leave);
    }
  }

  Future<void> _leave() async {
    if (_leaving || !mounted) return;
    _leaving = true;
    _timer?.cancel();
    _drift?.stop();
    await _entry.reverse();
    widget.onDone();
  }

  @override
  void dispose() {
    _timer?.cancel();
    _drift?.dispose();
    _entry.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final severity = widget.message.severity;
    final color = severity.color;

    // easeOutBack gives the slight overshoot that reads as buoyancy — the
    // balloon rises past its resting point and settles back.
    final rise = CurvedAnimation(
      parent: _entry,
      curve: Curves.easeOutBack,
      reverseCurve: Curves.easeInCubic,
    );
    final fade = CurvedAnimation(parent: _entry, curve: Curves.easeOut);

    // The gesture sits *outside* the animated wrappers on purpose. Nested
    // inside them, the opacity/transform layers swallowed the hit test and
    // taps never reached the button at all — the toast looked interactive and
    // wasn't.
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: _leave,
      child: AnimatedBuilder(
        animation: Listenable.merge([_entry, _drift]),
        builder: (context, child) {
          final bob = _drift == null
              ? 0.0
              // A shallow, slow rise and fall. 3px is enough to feel alive and
              // small enough never to look like a glitch.
              : (Curves.easeInOut.transform(_drift!.value) - 0.5) * 6;
          // Leaving lifts away rather than dropping back down — balloons go up.
          final dy =
              _leaving ? -28 * (1 - _entry.value) : 34 * (1 - rise.value) + bob;
          return Opacity(
            opacity: fade.value.clamp(0.0, 1.0),
            child: Transform.translate(offset: Offset(0, dy), child: child),
          );
        },
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 5, 20, 5),
          child: Center(
            child: Container(
              constraints: const BoxConstraints(maxWidth: 460),
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
              decoration: BoxDecoration(
                color: AppColors.surface,
                borderRadius: BorderRadius.circular(AppRadius.pill),
                border: Border.all(color: color.withValues(alpha: 0.45)),
                boxShadow: [
                  // Tinted, not black: the glow reads as the balloon's own
                  // colour lifting off the page.
                  BoxShadow(
                    color: color.withValues(alpha: 0.16),
                    blurRadius: 22,
                    offset: const Offset(0, 8),
                  ),
                ],
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(severity.icon, size: 16, color: color),
                  const SizedBox(width: 10),
                  Flexible(
                    child: Text(
                      widget.message.text,
                      style: AppText.body(height: 1.35),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
