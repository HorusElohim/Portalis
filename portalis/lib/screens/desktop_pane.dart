/// What the desktop shell's centre column is currently showing.
///
/// Its own file rather than living in `desktop_shell.dart`: both that file
/// and `desktop_sidebar.dart` need the type — the sidebar to know which of
/// its own buttons is lit — and a type two files share belongs to neither of
/// them, or the pair would have to import each other.
///
/// Neither [DesktopPane.you] nor Home appear here as a pane distinct from
/// [collections], unlike mobile — both are answers to questions this layout
/// doesn't leave open. Transfers showed the same collections a second time:
/// every row already carries its own bar, rate and countdown, and the list
/// is permanently on screen. Home was the welcome — what Portalis is, and
/// the ways to start something — which the sidebar now says outright with
/// its own actions beside a list that is always visible, and which the list
/// itself says the same way ([Welcome], from `ui/welcome.dart`) when there
/// is nothing in it yet. On a phone both earn their keep, because there the
/// list is one destination among four and a row is small.
///
/// [share] and [join] exist for the same reason as the rest: whatever the
/// centre pane shows, the sidebar and list stay put. They differ from
/// [people]/[you]/[settings] only in how they're reached — a one-shot action
/// in the sidebar rather than a persistent header button — nothing that
/// selects a pane otherwise distinguishes them. [settings] alone has no
/// mobile tab of its own; [people] and [you] both do, so crossing the
/// breakpoint into either keeps you on the same destination rather than
/// dropping you on Collections.
enum DesktopPane { collections, people, you, settings, share, join }
