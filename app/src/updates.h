// New versions: the question on first start, the check at start-up and from Help, and installing the new release in
// whatever way this copy was installed (installer, zip, .deb, .rpm, tarball, app bundle). A build from source is told
// about it and pointed at git.
#pragma once
#include <functional>

class QWidget;

namespace az {

// At start-up: asks once whether to check, then checks in the background when the answer was yes.
// may_quit asks to save the scene and says whether Aztek may close for the install.
void updates_at_startup(QWidget* main, std::function<bool()> may_quit);
// Help > Check for Updates: checks now and says what it found, newer or not.
void check_for_updates(QWidget* main, std::function<bool()> may_quit);

} // namespace az
