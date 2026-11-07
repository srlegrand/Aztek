// The display colour palette as swatches: the chooser layers and drawing overrides use, and Display > Wireframe Color.
#pragma once

#include <QColor>
#include <QString>

class QWidget;

namespace az {

// A palette entry as the core holds it, so colorIndex edits show.
QColor palette_color(int index);

struct ColorChoice {
    bool accepted = false;
    bool use_rgb = false;
    int index = 0;
    QColor rgb;
};

// Modal: the 32 entries, where 0 is no colour, or an RGB colour.
ColorChoice choose_display_color(QWidget* parent, const QString& title, int index, bool use_rgb, const QColor& rgb);

// Display > Wireframe Color: acts on the selection when Apply is pressed.
void show_wireframe_color_window(QWidget* parent);

} // namespace az
