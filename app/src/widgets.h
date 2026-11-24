// Smaller pieces of the main window: option boxes, the time and range sliders, the command line, the hotbox and
// the preferences window.
#pragma once
#include <QColor>
#include <QDialog>
#include <QHash>
#include <QJsonObject>
#include <QLineEdit>
#include <QMenu>
#include <QPixmap>
#include <QWidget>
#include <QWidgetAction>
#include <functional>

class QToolButton;
class QLabel;
class QDoubleSpinBox;
class QSpinBox;

class QPixmap;

namespace az {

// The logo on a dark panel with the version: the splash screen and the About box.
QPixmap splash_pixmap();
void show_about(QWidget* parent);
// Help > Thanks: the GitHub names the welcome screen thanks, as links. In welcome.cpp.
void show_thanks(QWidget* parent);
// Windows > Relationship Editors, opened on one kind: displayLayers, sets, deformerSets, shadingGroups, characters,
// lightCentric or objectCentric.
void show_relationship_editor(QWidget* parent, const QString& kind);
QWidget* make_relationship_editor(QWidget* parent, const QString& kind);
QWidget* make_hypershade(QWidget* parent);


// A command's settings, built from its flags and remembered between sessions.
class OptionsDialog : public QDialog {
    Q_OBJECT
public:
    static void open(const QString& command, const QString& title, const QJsonObject& fixed = QJsonObject(), const QPoint& at = QPoint());
    // The remembered settings of a command, as a call.
    static QJsonObject saved(const QString& command);
private:
    OptionsDialog(const QString& command, const QString& title);
    QJsonObject collect() const;
    void store(const QJsonObject& call) const;
    QString command_;
    // What the menu item itself passes: the projection type, the deformer's kind.
    QJsonObject fixed_;
    QList<std::function<void(QJsonObject&)>> readers_;
    // Rows that grey out or switch off others, from the layout's disabledWhen and excludes (#144).
    void apply_rules();
    QHash<QString, std::function<QJsonValue()>> value_of_;
    QHash<QString, QWidget*> widget_of_;
    QList<std::function<void()>> rules_, exclusions_;
};

// What Ctrl+Shift+click on a menu row with an option box does.
std::function<void(QAction*)>& shelf_capture();

// A menu item with the reference's option box on its right.
class OptionRow : public QWidgetAction {
    Q_OBJECT
public:
    OptionRow(const QString& label, const QString& shortcut, std::function<void()> run, std::function<void()> options, QObject* parent);
protected:
    QWidget* createWidget(QWidget* parent) override;
private:
    QString label_, shortcut_;
    std::function<void()> run_, options_;
};

class TimeSlider : public QWidget {
    Q_OBJECT
public:
    explicit TimeSlider(QWidget* parent = nullptr);
    double time() const { return time_; }
    void set_time(double t);
    void set_range(double a, double b);
    double start() const { return start_; }
    double end() const { return end_; }
    void set_keys(const QVector<double>& k);
    void set_breakdowns(const QVector<double>& k);
    const QVector<double>& keys() const { return keys_; }
    bool has_range() const { return range_a_ != range_b_ || selecting_; }
    void set_tick_width(int w) { tick_width_ = w; update(); }
    void set_ticks_shown(bool on) { ticks_shown_ = on; update(); }
    QString range_text() const;
    // Time Slider bookmarks: node, name, first and last frame, colour.
    struct Bookmark {
        QString node, name;
        double start = 1, stop = 1;
        QColor color;
    };
    void set_bookmarks(const QVector<Bookmark>& b);
    const QVector<Bookmark>& bookmarks() const { return bookmarks_; }
    bool bookmarks_shown = true;
    // The range picked with Shift-drag, or the current frame when there is none.
    std::pair<double, double> picked_range() const;
    // What timeControl set on it: colours, waveform part, snapping, tick span, whether it drives the scene's time, and
    // whether it shows; read again when the scene changes.
    void refresh_state();
signals:
    void timeChanged(double t);
    // A bookmark Ctrl-dragged: its node, new first and last frame.
    void bookmarkMoved(const QString& node, double start, double stop);
    // A selected range was dragged by this many frames.
    void rangeMoved(double from, double to, double by);
    void contextMenuAt(QPoint global);
protected:
    void paintEvent(QPaintEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void mouseMoveEvent(QMouseEvent*) override;
    void mouseReleaseEvent(QMouseEvent*) override;
private:
    double frame_at(int x) const;
    int tick_width_ = 2;
    bool ticks_shown_ = true;
    double time_ = 1, start_ = 1, end_ = 120;
    QVector<double> keys_;
    QVector<double> breakdowns_;
    // Shift-drag selects a range (drawn red); a drag in it moves its keys.
    double range_a_ = 0, range_b_ = 0;
    bool selecting_ = false;
    bool moving_ = false;
    double move_from_ = 0, move_by_ = 0;
    QVector<Bookmark> bookmarks_;
    // Ctrl-drag of a bookmark: which, which part (0 middle, 1 start edge, 2 stop edge), by how much.
    int bm_drag_ = -1, bm_part_ = 0;
    double bm_from_ = 0, bm_by_ = 0;
    int bookmark_at(double x, int* part) const;
    // timeControl's settings (refresh_state).
    QColor cur_color_{210, 210, 210, 120}, fg_color_{150, 150, 150}, bg_color_{45, 45, 45};
    QString waveform_ = "top", path_ = "timeControl1";
    int tick_span_ = 1;
    bool global_time_ = true, snap_ = true, enabled_ = true;
    void fire(const char* flag);
protected:
    bool event(QEvent* e) override;
};

// The waveform of one sound over a range of frames, with the current time: soundControl's widget. A press or drag
// moves the time; press and release report through the signals.
class SoundStrip : public QWidget {
    Q_OBJECT
public:
    explicit SoundStrip(QWidget* parent = nullptr);
    QString sound, waveform = "top";
    bool shown = true;
    double min_time = 1, max_time = 120;
    bool range_set = false;
signals:
    void pressed(double frame);
    void dragged(double frame);
    void released(double frame);
protected:
    void paintEvent(QPaintEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void mouseMoveEvent(QMouseEvent*) override;
    void mouseReleaseEvent(QMouseEvent*) override;
private:
    double frame_at(int x) const;
    std::pair<double, double> span() const;
};

// Draws a waveform's columns, each bucket's lowest and highest sample, in a band: part is top (the samples above
// zero, up from the bottom), bottom (those below, down from the top) or both (round the middle).
void draw_waveform(QPainter& p, const QJsonArray& peaks, const QRectF& band, const QString& part, const QColor& c);

// The bar under the time slider: the playback range inside the animation range, dragged by its ends or middle.
class RangeSlider : public QWidget {
    Q_OBJECT
public:
    explicit RangeSlider(QWidget* parent = nullptr);
    void set(double ast, double min, double max, double aet);
signals:
    void rangeChanged(double min, double max);
    // Double-click: the playback range to the whole animation range and back.
    void toggleFullRange();
protected:
    void paintEvent(QPaintEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void mouseMoveEvent(QMouseEvent*) override;
    void mouseReleaseEvent(QMouseEvent*) override;
    void mouseDoubleClickEvent(QMouseEvent*) override { emit toggleFullRange(); }
private:
    double frame_at(double x) const;
    double x_of(double f) const;
    double ast_ = 1, min_ = 1, max_ = 120, aet_ = 200;
    int grab_ = -1;
    double grab_at_ = 0, grab_min_ = 0, grab_max_ = 0;
};

class CommandLine : public QWidget {
    Q_OBJECT
public:
    explicit CommandLine(QWidget* parent = nullptr);
    void show_result(const QString& text, bool error);
signals:
    void openScriptEditor();
protected:
    bool eventFilter(QObject* o, QEvent* e) override;
private:
    QToolButton* lang_;
    QLineEdit* input_;
    QLineEdit* result_;
    // History per language, as the two share a line.
    QHash<QString, QStringList> histories_;
    int pos_ = 0;
};

struct MarkItem;

// Space held, or hotBox from a script: common and panel menus above the cursor, task menu sets below, and marking menus
// in five zones. Which rows show and how it is drawn are hotBox's settings, given each time it pops.
class Hotbox : public QWidget {
    Q_OBJECT
public:
    Hotbox(const QList<QMenu*>& common, const QList<QPair<QString, QList<QMenu*>>>& sets, QMenu* recent, QWidget* parent);
    // hotBox's rows by name (common, pane, modeling, polygons, surfaces, rigging, animation, dynamics, cloth, rendering);
    // a row not named shows. A menu with the azHotboxGroup property "polygons" or "surfaces" shows under that row alone.
    void set_rows_shown(const QHash<QString, bool>& rows);
    // hotBox's settings: "dh" all of it, "dzo" the zones without the menus, "dco" nothing drawn and the middle's marking
    // menu wherever the press; transparency 0 to 100; with rmb, a right press in the middle lists `current`'s menus.
    void set_look(const QString& style, int transparency, bool rmb, const QList<QMenu*>& current);
    // Popped by something other than a key (hotBox -noKeyPress): the key's release doesn't put it away.
    void set_hold(bool hold) { hold_ = hold; }
    bool holding() const { return hold_; }
    void popup(QPoint global, const QList<QAction*>& panel_actions);
    void finish();
    // Zone: 0 north, 1 east, 2 south, 3 west, 4 the Aztek box in the middle.
    void set_zones(std::function<QList<MarkItem>(int, QPoint)> f) { zones_ = std::move(f); }
signals:
    // Space up; a tap, quick and with nothing done in the hotbox, toggles the view under the cursor.
    void spaceReleased(bool tap);
    // A menu or zone of it was used.
    void used();
    // A row turned on or off from Hotbox Controls.
    void rowToggled(const QString& row, bool on);
protected:
    void paintEvent(QPaintEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void keyReleaseEvent(QKeyEvent*) override;
private:
    int zone_at(QPoint p) const;
    void mark_used();
    void apply_rows();
    void place(QPoint target);
    QWidget* box_ = nullptr;
    QWidget* middle_ = nullptr;
    QList<QToolButton*> panel_buttons_;
    QHash<QString, QWidget*> strips_;
    QHash<QString, QAction*> toggles_;
    QList<QPair<QToolButton*, QString>> groups_;
    QHash<QString, bool> rows_;
    QPoint center_;
    QPoint origin_;
    qint64 shown_ms_ = 0;
    bool used_ = false;
    bool hold_ = false;
    QString style_ = "dh";
    int transparency_ = 25;
    bool rmb_ = true;
    QList<QMenu*> current_;
    std::function<QList<MarkItem>(int, QPoint)> zones_;
};

class Preferences : public QDialog {
    Q_OBJECT
public:
    explicit Preferences(QWidget* parent);
signals:
    void changed();
};

} // namespace az
