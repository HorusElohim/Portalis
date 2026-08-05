/// What the desktop shell's centre column is currently showing.
///
/// Its own file rather than living in `desktop_shell.dart`: both that file
/// and `desktop_sidebar.dart` need the type — the sidebar to know which of
/// its own buttons is lit — and a type two files share belongs to neither of
/// them, or the pair would have to import each other.
///
/// [you] no longer exists as its own pane — that content folded into
/// [settings] (see `settings.dart`'s absorption of the old
/// `user_screen.dart`), so tapping the identity chip now opens Settings
/// directly.
///
/// [home] carries what [collections] used to plus Home's own welcome and
/// omnibar, unlike mobile — both used to be answers to questions this layout
/// left open on its own terms: Transfers showed the same collections a
/// second time (every row already carries its own bar, rate and countdown,
/// and the list is permanently on screen), and Home was the welcome — what
/// Portalis is, and the ways to start something — which the sidebar already
/// said outright with its own actions beside a list that is always visible,
/// and which the list itself said the same way ([Welcome], from
/// `ui/welcome.dart`) when there was nothing in it yet. Folding [home] and
/// [collections] into one pane matches what `home.dart` now is on mobile too
/// — one screen for "what do I have and how do I add something" rather than
/// two.
///
/// [share] and [join] exist for the same reason as the rest: whatever the
/// centre pane shows, the sidebar and list stay put. They differ from
/// [people]/[settings] only in how they're reached — a one-shot action in the
/// sidebar (or the omnibar, or a drop) rather than a persistent header
/// button — nothing that selects a pane otherwise distinguishes them.
enum DesktopPane { home, people, settings, share, join }
