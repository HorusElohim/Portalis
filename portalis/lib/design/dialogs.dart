// Cross-feature dialog design primitive.

import 'package:flutter/material.dart';

import '../theme.dart';

/// Prompts for a single text value.
///
/// Returns `null` if dismissed, and the trimmed text otherwise — an empty
/// string is a *deliberate* answer meaning "unset", which is why it isn't
/// collapsed into `null`. Callers that treat blank as "leave alone" would
/// otherwise have no way to clear an optional setting.
///
/// Five screens had grown near-identical copies of this dialog.
Future<String?> promptForText(
  BuildContext context, {
  required String title,
  String? initialValue,
  String? hint,
  String? helper,
  TextInputType? keyboardType,
  int maxLines = 1,
  String confirmLabel = 'Save',
}) {
  final controller = TextEditingController(text: initialValue ?? '');
  return showDialog<String>(
    context: context,
    builder: (dialogContext) => AlertDialog(
      backgroundColor: AppColors.surface,
      title: Text(title, style: displayText(size: 17)),
      // Explicit width: AlertDialog measures its content's intrinsic width,
      // and some children (QR views, multi-line fields) can't answer that.
      content: SizedBox(
        width: 320,
        child: TextField(
          controller: controller,
          autofocus: true,
          keyboardType: keyboardType,
          maxLines: maxLines,
          style: monoLabel(size: 13, color: AppColors.text, letterSpacing: 0),
          decoration: InputDecoration(
            hintText: hint,
            hintStyle: TextStyle(color: AppColors.textGhost),
            helperText: helper,
            helperMaxLines: 3,
            helperStyle: AppText.caption(color: AppColors.textDim),
            focusedBorder: UnderlineInputBorder(
              borderSide: BorderSide(color: AppColors.signal),
            ),
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(dialogContext).pop(),
          child: const Text('Cancel'),
        ),
        TextButton(
          onPressed: () =>
              Navigator.of(dialogContext).pop(controller.text.trim()),
          child: Text(confirmLabel),
        ),
      ],
    ),
  );
}

/// A yes/no confirmation. Returns `true` only on explicit confirmation —
/// dismissing counts as "no".
Future<bool> confirmAction(
  BuildContext context, {
  required String title,
  required String message,
  String confirmLabel = 'Confirm',
  bool destructive = false,
}) async {
  final result = await showDialog<bool>(
    context: context,
    builder: (dialogContext) => AlertDialog(
      backgroundColor: AppColors.surface,
      title: Text(title, style: displayText(size: 17)),
      content: Text(
        message,
        style: AppText.secondary(color: AppColors.textDim, height: 1.5),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(dialogContext).pop(false),
          child: const Text('Cancel'),
        ),
        TextButton(
          onPressed: () => Navigator.of(dialogContext).pop(true),
          child: Text(
            confirmLabel,
            style: TextStyle(
                color: destructive ? AppColors.danger : AppColors.signal),
          ),
        ),
      ],
    ),
  );
  return result ?? false;
}
