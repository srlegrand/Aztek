// The question asked at the first start when the reference's user folder is found: copy it, use it where it is, or
// start without it.
#pragma once

class QWidget;

namespace az {

// Asks once, before the user folder's start-up scripts run. Does nothing when a choice has been made or no such folder
// is there.
void ask_user_folder_at_first_start(QWidget* parent);

} // namespace az
