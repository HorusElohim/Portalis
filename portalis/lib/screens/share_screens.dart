import 'package:flutter/material.dart';
import '../theme.dart';
import '../widgets/common.dart';

class ShareStep1Screen extends StatefulWidget {
  const ShareStep1Screen({super.key});

  @override
  State<ShareStep1Screen> createState() => _ShareStep1ScreenState();
}

class _ShareStep1ScreenState extends State<ShareStep1Screen> {
  final _selected = <int>{};
  static const _tileCount = 9;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(14, 0, 14, 6),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  _CircleCloseButton(
                    onTap: () => Navigator.of(context).pop(),
                  ),
                  const Text(
                    'Share something',
                    style: TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
                  ),
                  const SizedBox(width: 34),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 8),
              child: TextField(
                style: const TextStyle(color: AppColors.text, fontSize: 13.5),
                decoration: InputDecoration(
                  hintText: 'Collection name',
                  hintStyle: const TextStyle(color: AppColors.neutral400),
                  filled: true,
                  fillColor: AppColors.surface,
                  contentPadding:
                      const EdgeInsets.symmetric(horizontal: 12, vertical: 11),
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(8),
                    borderSide: const BorderSide(color: AppColors.borderStrong),
                  ),
                  enabledBorder: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(8),
                    borderSide: const BorderSide(color: AppColors.borderStrong),
                  ),
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 14, 20, 8),
              child: Align(
                alignment: Alignment.centerLeft,
                child: SectionLabel('FROM YOUR CAMERA ROLL'),
              ),
            ),
            Expanded(
              child: GridView.builder(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: 3,
                  mainAxisSpacing: 8,
                  crossAxisSpacing: 8,
                  childAspectRatio: 1,
                ),
                itemCount: _tileCount,
                itemBuilder: (context, index) {
                  final selected = _selected.contains(index);
                  return Material(
                    color: Colors.transparent,
                    borderRadius: BorderRadius.circular(6),
                    child: InkWell(
                      borderRadius: BorderRadius.circular(6),
                      onTap: () => setState(() {
                        if (!_selected.add(index)) _selected.remove(index);
                      }),
                      child: Container(
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(6),
                          border: selected
                              ? Border.all(color: AppColors.accent, width: 2)
                              : null,
                        ),
                        clipBehavior: Clip.antiAlias,
                        child: Stack(
                          children: [
                            const PlaceholderTile(),
                            if (selected)
                              Positioned(
                                top: 4,
                                right: 4,
                                child: Container(
                                  width: 18,
                                  height: 18,
                                  decoration: const BoxDecoration(
                                    color: AppColors.accent,
                                    shape: BoxShape.circle,
                                  ),
                                  child: const Icon(Icons.check,
                                      size: 12, color: AppColors.bg),
                                ),
                              ),
                          ],
                        ),
                      ),
                    ),
                  );
                },
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
              child: PillButton(
                label: 'Continue · ${_selected.length} selected',
                onTap: () => Navigator.of(context).push(
                  MaterialPageRoute(builder: (_) => const ShareStep2Screen()),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class ShareStep2Screen extends StatelessWidget {
  const ShareStep2Screen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: Column(
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: NavBackButton(onTap: () => Navigator.of(context).pop()),
            ),
            Expanded(
              child: SingleChildScrollView(
                padding: const EdgeInsets.symmetric(horizontal: 20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      'Invite people',
                      style:
                          TextStyle(fontSize: 20, fontWeight: FontWeight.w500),
                    ),
                    const SizedBox(height: 14),
                    Container(
                      padding:
                          const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
                      decoration: BoxDecoration(
                        color: AppColors.surface,
                        border: Border.all(color: AppColors.border),
                        borderRadius: BorderRadius.circular(8),
                      ),
                      child: Row(
                        children: [
                          const Expanded(
                            child: Text(
                              'smartshare.link/x7Kq2',
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                fontSize: 12,
                                fontFamily: 'monospace',
                                color: AppColors.text,
                              ),
                            ),
                          ),
                          const SizedBox(width: 8),
                          PillButton(label: 'Copy', onTap: () {}),
                        ],
                      ),
                    ),
                    const SizedBox(height: 20),
                    Center(
                      child: SizedBox(
                        width: 150,
                        height: 150,
                        child: PlaceholderTile(
                          label: 'QR — scan to join',
                          borderRadius: 8,
                        ),
                      ),
                    ),
                    const SizedBox(height: 18),
                    const Text(
                      'Anyone with the link becomes a collaborator. Media never '
                      'touches a server — it flows directly between your devices.',
                      textAlign: TextAlign.center,
                      style: TextStyle(
                        fontSize: 11.5,
                        height: 1.5,
                        color: AppColors.neutral400,
                      ),
                    ),
                  ],
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
              child: PillButton(
                label: 'Start sharing',
                onTap: () =>
                    Navigator.of(context).popUntil((route) => route.isFirst),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _CircleCloseButton extends StatelessWidget {
  const _CircleCloseButton({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      shape: CircleBorder(side: BorderSide(color: AppColors.borderStrong)),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: const SizedBox(
          width: 34,
          height: 34,
          child: Icon(Icons.close, size: 18, color: AppColors.text),
        ),
      ),
    );
  }
}
