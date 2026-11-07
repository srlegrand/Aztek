// Harold from the interface: the Render and Harold menus of the Rendering menu set, the Render View, the Render Settings
// window, Render Sequence, and the Harold items of the Create menu.
#pragma once
#include <QString>

class QMenu;
class QWidget;

namespace az {

// renderview.cpp
void show_render_view(QWidget* parent);
// Render Current Frame: the Render View opens and the frame starts in the background.
void render_current_frame(QWidget* parent);
void render_ipr(QWidget* parent);
void redo_previous_render(QWidget* parent);
void render_sequence_dialog(QWidget* parent);
QMenu* render_menu(QWidget* parent);
QMenu* harold_menu(QWidget* parent);
// Create > Harold: lights, the atmosphere and the standard surface.
void harold_create_menu(QMenu* create, QWidget* parent);
void create_harold_light(const QString& type);
void create_mesh_light();
void create_atmosphere();

// rendersettings.cpp
void show_render_settings(QWidget* parent);

} // namespace az
