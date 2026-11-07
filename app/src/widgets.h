// Smaller pieces of the main window: option boxes, the time and range sliders, the command line, the hotbox and
// the preferences window.
#pragma once
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


// A command's settings, built from its flags and remembered between sessions.
class OptionsDialog : public QDialog {
    Q_OBJECT
public:
    static void open(const QString& command, const QString& title, const QJsonObject& fixed = QJsonObject());
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
signals:
    void timeChanged(double t);
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
};

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

// Space held: every menu at the cursor, with marking menus in the zones around them. `common` of the menus go on top.
class Hotbox : public QWidget {
    Q_OBJECT
public:
    Hotbox(const QList<QMenu*>& menus, int common, QWidget* parent);
    void popup(QPoint global);
    // Zone: 0 north, 1 east, 2 south, 3 west.
    void set_zones(std::function<QList<MarkItem>(int)> f) { zones_ = std::move(f); }
signals:
    void spaceReleased();
protected:
    void paintEvent(QPaintEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void keyReleaseEvent(QKeyEvent*) override;
private:
    int zone_at(QPoint p) const;
    QWidget* box_;
    QPoint center_;
    QPixmap under_;
    std::function<QList<MarkItem>(int)> zones_;
};

class Preferences : public QDialog {
    Q_OBJECT
public:
    explicit Preferences(QWidget* parent);
signals:
    void changed();
};

} // namespace az
