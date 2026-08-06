import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../features/settings/presentation/device_profile_section.dart';
import '../services/navigation.dart';
import 'settings/formats.dart';

/// The local device identity and the people who receive from it.
///
/// User is deliberately separate from Settings: identity answers "who am I?"
/// while Settings answers "how does the engine behave?".
class UserScreen extends StatefulWidget {
  const UserScreen({super.key, this.embedded = false});

  final bool embedded;

  @override
  State<UserScreen> createState() => _UserScreenState();
}

class _UserScreenState extends State<UserScreen> {
  bool _showFormats = false;
  String? _syncAddress;

  @override
  void initState() {
    super.initState();
    AppControllers.identity.load();
    _loadSyncAddress();
  }

  Future<void> _loadSyncAddress() async {
    try {
      final address = await AppControllers.collections.syncAddress();
      if (mounted) setState(() => _syncAddress = address);
    } catch (_) {
      // The identity remains useful even when the sync listener is unavailable.
    }
  }

  Future<void> _rename() async {
    final profile = AppControllers.identity.info;
    final result = await promptForText(
      context,
      title: 'Your name',
      initialValue: profile?.nickname,
      helper: 'This is how you appear to collaborators.',
    );
    if (result == null || result.isEmpty || !mounted) return;
    try {
      await AppControllers.identity.rename(result);
    } catch (error) {
      if (mounted) showToast(context, 'Couldn\'t rename: $error');
    }
  }

  void _openFormats() => openNestedScreen(
        context,
        embedded: widget.embedded,
        showInPlace: () => setState(() => _showFormats = true),
        push: (_) => const FormatsScreen(),
      );

  @override
  Widget build(BuildContext context) {
    if (_showFormats) {
      return FormatsScreen(
        embedded: widget.embedded,
        onBack: () => setState(() => _showFormats = false),
      );
    }

    return AppScreen(
      title: 'User',
      embedded: widget.embedded,
      width: ScreenWidth.full,
      body: ListenableBuilder(
        listenable: Listenable.merge([
          AppControllers.identity,
          AppControllers.collections,
        ]),
        builder: (context, _) {
          final identity = AppControllers.identity;
          return SingleChildScrollView(
            child: DeviceProfileSection(
              profile: identity.info,
              identityError: identity.lastError,
              collections: AppControllers.collections.collections,
              syncAddress: _syncAddress,
              onRename: identity.info == null ? null : _rename,
              onOpenPeople: () =>
                  AppNavigation.tab.value = AppNavigation.peopleTab,
              onOpenFormats: _openFormats,
            ),
          );
        },
      ),
    );
  }
}
