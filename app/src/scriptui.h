// Windows, layouts and controls scripts build. The model lives in the core: this file mirrors it in widgets and reports
// what the user does. Commands that must answer at once (dialogs, playblast) arrive through the hook.
#pragma once
#include <QElapsedTimer>
#include <QHash>
#include <QJsonArray>
#include <QJsonObject>
#include <QList>
#include <QObject>
#include <QPoint>
#include <QPointer>
#include <QString>
#include <functional>

class QAction;
class QActionGroup;
class QDockWidget;
class QLabel;
class QMainWindow;
class QMenu;
class QMenuBar;
class QPushButton;
class QWidget;

namespace az {

class ViewportWidget;

class ScriptUi : public QObject {
    Q_OBJECT
public:
    static ScriptUi* get();
    // The main window scripts hang menus on, and how a playblast finds a view by panel name.
    void set_main(QMainWindow* w, std::function<ViewportWidget*(const QString&)> view_for_panel, std::function<void()> take_requests);
    // Applies what the core has queued. Cheap when nothing is.
    void sync();
    // A view shown in a script's layout, by panel name.
    ViewportWidget* embedded_view(const QString& panel) const;
    void show_in_view_message(const QJsonObject& o);
    // One of the application's own editors, for scripts to set up by the reference's name for it.
    void add_builtin_editor(const QString& name, const QString& kind, QWidget* w);
    void hook(const QString& kind, const QJsonObject& payload);
protected:
    bool eventFilter(QObject* o, QEvent* e) override;
private:
    struct Item;
    ScriptUi();
    void apply_op(const QJsonObject& op);
    void create(const QJsonObject& op);
    void edit(Item* it, const QJsonObject& props);
    void remove(Item* it);
    void show_window(Item* it);
    void build_control(Item* it, const QJsonObject& props);
    void build_layout(Item* it, const QJsonObject& props);
    void build_window(Item* it, const QJsonObject& props);
    void place_dock(Item* it, const QJsonObject& props, bool made);
    QDockWidget* dock_named(const QString& name) const;
    void build_group(Item* it, const QJsonObject& props);
    void build_menu(Item* it, Item* parent, const QJsonObject& props);
    void build_menu_item(Item* it, Item* parent, const QJsonObject& props);
    void place(Item* parent, Item* child);
    void apply_common(Item* it, const QJsonObject& props);
    void apply_control(Item* it, const QJsonObject& props);
    void apply_layout(Item* it, const QJsonObject& props);
    void apply_group_columns(Item* it);
    void apply_menu_item(Item* it, const QJsonObject& props);
    void relayout_form(Item* it);
    void rebuild_option_menu(Item* it);
    QMenuBar* menu_bar_of(Item* it);
    void fire(const QString& path, const QJsonObject& set, const QJsonArray& flags, const QJsonArray& args);
    QAction* menu_action_for(Item* it) const;
    // Dialogs.
    void confirm_dialog(const QJsonObject& p);
    void prompt_dialog(const QJsonObject& p);
    void file_dialog2(const QJsonObject& p);
    void file_dialog(const QJsonObject& p);
    void playblast(const QJsonObject& p);
    void pump();

    QHash<QString, Item*> items_;
    QMainWindow* main_ = nullptr;
    std::function<ViewportWidget*(const QString&)> view_for_panel_;
    std::function<void()> take_requests_;
    QHash<QString, QPointer<ViewportWidget>> embedded_;
    QElapsedTimer pump_timer_;
    bool wait_cursor_ = false;
    QHash<QString, QActionGroup*> radio_groups_;
    QPointer<QLabel> in_view_;
    QPoint drag_from_;
};

// Called by the Host poller.
void scriptui_sync();

} // namespace az
