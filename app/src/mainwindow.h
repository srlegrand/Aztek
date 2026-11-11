// The main window: menus by menu set, status line, shelf, toolbox, model panels, editors, time slider, command line
// and the hotkeys.
#pragma once
#include <QHash>
#include <QJsonObject>
#include <QJsonArray>
#include <QMainWindow>
#include <QSet>
#include <functional>

class QActionGroup;
class QComboBox;
class QDockWidget;
class QKeyEvent;
class QLabel;
class QLineEdit;
class QMenu;
class QTabWidget;
class QToolBar;
class QToolButton;
class QSpinBox;
class QTimer;

namespace az {

class PanelLayout;
class ScriptEditor;
class CommandLine;
class TimeSlider;
class RangeSlider;
class Hotbox;
class ViewportWidget;
class ToolSettings;
class ShelfBar;
class InViewEditor;
struct MarkItem;

// A named action, callable from a hotkey, MEL or Python as the reference's runtime commands are.
struct RuntimeCommand {
    QString name, category, annotation;
    std::function<void()> fn;
};

struct HotkeyBinding {
    QString key;
    bool ctrl, shift, alt;
    QString press, release;
};

class MainWindow : public QMainWindow {
    Q_OBJECT
public:
    /// File > New Scene's options: the units a new scene starts in.
    void new_scene_options();
    /// Adds the reference's shelf files to the shelf bar, skipping the ones already there by name. Returns how many.
    int import_shelves(const QStringList& files);
    /// Windows > Settings/Preferences > Plug-in Manager.
    void show_plugin_manager();
    MainWindow();
    PanelLayout* panels() const { return panels_; }
    void run_startup(const QString& script, const QString& screenshot);
    void open_file(const QString& path);
    void show_reference_editor();
    void dragEnterEvent(QDragEnterEvent* e) override;
    void dropEvent(QDropEvent* e) override;
    void create_reference_dialog(bool options = false);
protected:
    bool eventFilter(QObject* o, QEvent* e) override;
    void closeEvent(QCloseEvent* e) override;
private:
    // Menus.
    void build_menus();
    void build_menu_set(const QString& set);
    QMenu* file_menu();
    QMenu* edit_menu();
    QMenu* create_menu();
    QMenu* select_menu();
    QMenu* modify_menu();
    QMenu* display_menu();
    QMenu* windows_menu();
    QMenu* mesh_menu();
    QMenu* edit_mesh_menu();
    QMenu* mesh_tools_menu();
    QMenu* mesh_display_menu();
    QMenu* curves_menu();
    QMenu* surfaces_menu();
    QMenu* uv_menu();
    QMenu* deform_menu();
    QMenu* skeleton_menu();
    QMenu* skin_menu();
    QMenu* constrain_menu();
    QList<QMenu*> dynamics_menus();
    QMenu* control_menu();
    QMenu* key_menu();
    QMenu* playback_menu();
    QAction* cmd(QMenu* m, const QString& label, const QString& command, const QJsonObject& call = QJsonObject(), const QString& shortcut = QString(), bool options = false);
    QAction* todo(QMenu* m, const QString& label);

    // Bars and panels.
    void build_status_line();
    void build_shelf();
    void build_toolbox();
    void build_docks();
    void build_bottom();
    void apply_prefs();

    // State.
    void refresh_state();
    void set_tool(const QString& tool);
    void set_mode(const QString& mode);
    ViewportWidget* active_view() const;
    void marking_menu(QPoint global, int mods);
    void repeat_last();
    void open_scene();
    void new_scene();
    /// True when the scene saved, or when it can be thrown away: asks if it has changes. False: stay where you are.
    bool confirm_unsaved();
    bool save_scene(bool as);
    bool no_prompts_ = false;
    bool hotkey(QKeyEvent* k, bool press);
    // runtime.cpp: named actions, the hotkey table and its editor.
    void build_runtime();
    void add_runtime(const QString& name, const QString& category, const QString& annotation, std::function<void()> fn);
    bool run_command(const QString& command);
    void view_key(const QString& key, const QString& json);
    void apply_snap();
    QList<HotkeyBinding> default_hotkeys() const;
    void load_hotkeys();
    void set_binding(const QJsonObject& o);
    void save_user_hotkey(const QJsonObject& o);
    void show_hotkey_editor();
    void handle_ui_request(const QString& r);
    // Model panels by the reference's names, and telling the core what they show.
    ViewportWidget* view_for_panel(const QString& panel) const;
    void sync_panels();
    QString panels_synced_;
    // statusline.cpp.
    void input_entered(const QString& text);
    void sync_status_line();
    // appmenus.cpp: helpers the menus and hotkeys share.
    void toggle_main_pane();
    void toggle_view_flag(const QString& key);
    void step_time(int by);
    void step_key(const QString& which);
    void increment_save();
    void nudge(int dx, int dy);
    void scale_manip(double by);
    void show_history_manips();
    void tool_marking_menu(QPoint global, const QString& hold);
    void tool_options_marking_menu(QPoint global);
    QList<MarkItem> hotbox_zone(int zone);
    void object_marking_items(const QString& hit, QPoint global, QList<MarkItem>& items);
    void apply_workspace(const QString& name);
    void show_named_window(const QString& name);
    void add_to_shelf(QAction* a);
    void select_by_type(const QString& type);
    void select_shell(bool border);
    void set_project();
    void add_recent_project(const QString& dir);
    void remember_command(const QString& mel_text);
    void view_scrub(QObject* o, QEvent* e);
    void run_menu_path(const QString& path);
    void show_script_editor();
    void show_attribute_editor();
    void show_graph_editor();
    void show_uv_editor();
    void show_preferences();
    void show_tool_settings();
    void show_dope_sheet();
    void show_node_editor();
    void show_hypershade();
    // Assigning materials, to the object under the marking menu or the selection when hit is empty.
    void assign_new_material(const QString& hit = QString());
    void assign_existing_material_menu(QMenu* menu, const QString& hit = QString());
    void material_attributes(const QString& hit = QString());
    QMenu* lighting_shading_menu();
    void show_set_driven_key();
    void time_slider_menu(QPoint global);
    void update_title();
    void toggle_play(int dir);
    void stop_play();
    void sync_time_range();

    PanelLayout* panels_ = nullptr;
    ScriptEditor* script_editor_ = nullptr;
    // Script history from the start, held until the Script Editor first opens.
    QJsonArray log_before_editor_;
    QWidget* script_window_ = nullptr;
    QWidget* graph_window_ = nullptr;
    QWidget* uv_window_ = nullptr;
    QWidget* dope_window_ = nullptr;
    QWidget* node_window_ = nullptr;
    QWidget* hypershade_window_ = nullptr;
    QWidget* sdk_window_ = nullptr;
    CommandLine* command_line_ = nullptr;
    TimeSlider* time_slider_ = nullptr;
    RangeSlider* range_slider_ = nullptr;
    QSpinBox *anim_start_ = nullptr, *play_start_ = nullptr, *play_end_ = nullptr, *anim_end_ = nullptr, *current_frame_ = nullptr;
    QTimer* play_timer_ = nullptr;
    int play_dir_ = 1;
    QAction* auto_key_ = nullptr;
    QLabel* help_line_ = nullptr;
    ToolSettings* tool_settings_ = nullptr;
    QDockWidget* tool_dock_ = nullptr;
    QComboBox* menu_set_ = nullptr;
    QList<QMenu*> set_menus_;
    QHash<QString, QAction*> mode_buttons_;
    QHash<QString, QAction*> tool_buttons_;
    QHash<QString, QAction*> scope_buttons_;
    QList<QAction*> object_masks_;
    QSet<QString> snap_on_;
    QString snap_held_;
    QComboBox* symmetry_ = nullptr;
    QComboBox* input_mode_ = nullptr;
    QLineEdit* input_field_ = nullptr;
    QAction* soft_button_ = nullptr;
    ShelfBar* shelf_bar_ = nullptr;
    QHash<QString, RuntimeCommand> runtime_;
    QList<HotkeyBinding> hotkeys_;
    QHash<int, QString> held_;
    // A key held for a drag in the view: move, rotate, scale, select (marking menu), scrub, nudgeKeys.
    QString view_hold_;
    double scrub_from_ = 0, scrub_time_ = 0;
    bool scrubbing_ = false;
    QString last_tool_ = "select";
    QStringList recent_commands_;
    QStringList clipboard_;
    QHash<QString, QWidget*> named_windows_;
    QByteArray main_pane_state_;
    bool main_pane_ = false;
    QTimer* autosave_ = nullptr;
    InViewEditor* in_view_ = nullptr;
    bool play_every_frame_ = false;
    double play_speed_ = 1.0;
    QString loop_mode_ = "Continuous";
    qint64 play_started_ms_ = 0;
    double play_started_frame_ = 1;
    QComboBox* fps_box_ = nullptr;
    QComboBox* speed_box_ = nullptr;
    double last_full_min_ = 0, last_full_max_ = 0;
    QAction* history_button_ = nullptr;
    QDockWidget* outliner_dock_ = nullptr;
    QDockWidget* channel_dock_ = nullptr;
    QDockWidget* attr_dock_ = nullptr;
    Hotbox* hotbox_ = nullptr;
    QString tool_ = "select";
    /// What G does again, and what it's called. Only what changed the scene counts: opening a window or selecting doesn't.
    std::function<void()> last_repeat_;
    QString last_repeat_label_;
    bool repeating_ = false;
    double menu_mark_ = 0;
    void watch_menus();
    void run_repeatable(const QString& label, const std::function<void()>& fn);
    void note_repeat(const QString& label, const std::function<void()>& fn, double undo_before);
    qint64 space_down_ms_ = 0;
    bool space_held_ = false;
    bool pivot_mode_ = false;
    // When D went down: a tap toggles Edit Pivot, a hold edits the pivot only while held.
    qint64 pivot_hold_ms_ = 0;
    void set_pivot_mode(bool on);
};

} // namespace az
