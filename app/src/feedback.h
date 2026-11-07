// "This doesn't feel right!": the shelf bar's button and the report window. The report opens as a GitHub issue page in the
// person's browser. The app makes no network requests, and nothing is public until they press Submit on that page.
#pragma once

class QWidget;

namespace az {

// The button for the far right of the shelf bar.
QWidget* make_feedback_button(QWidget* parent);
// Takes the screenshot and opens the report window, or raises the one already open.
void open_feedback(QWidget* main_window);

} // namespace az
