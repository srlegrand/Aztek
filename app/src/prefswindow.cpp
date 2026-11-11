// Preferences: categories saved between sessions, each with an effect. Values live in the settings and as optionVars, so
// scripts read the same numbers.
#include "mainwindow.h"
#include "aztek.h"
#include "host.h"
#include "panels.h"
#include "viewport.h"
#include "widgets.h"

#include <algorithm>

#include <QAbstractButton>
#include <QApplication>
#include <QPushButton>
#include <QRadioButton>
#include <QCheckBox>
#include <QComboBox>
#include <QDialogButtonBox>
#include <QDoubleSpinBox>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QListWidget>
#include <QSettings>
#include <QSpinBox>
#include <QStackedWidget>
#include <QStatusBar>
#include <QTimer>
#include <QToolBar>
#include <QVBoxLayout>

namespace az {

namespace {
// The working units the reference offers, as (name, label). Time is in its order: frame rates, then clock units.
const QList<QPair<QString, QString>> kLinearUnits = {{"mm", "millimeter"}, {"cm", "centimeter"}, {"m", "meter"}, {"km", "kilometer"},
                                                     {"in", "inch"}, {"ft", "foot"}, {"yd", "yard"}, {"mi", "mile"}};
const QList<QPair<QString, QString>> kAngularUnits = {{"deg", "degrees"}, {"rad", "radians"}};
const QList<QPair<QString, QString>> kTimeUnits = {
    {"2fps", "2 fps"}, {"3fps", "3 fps"}, {"4fps", "4 fps"}, {"5fps", "5 fps"}, {"6fps", "6 fps"}, {"8fps", "8 fps"},
    {"10fps", "10 fps"}, {"12fps", "12 fps"}, {"game", "15 fps (Game)"}, {"16fps", "16 fps"}, {"20fps", "20 fps"},
    {"23.976fps", "23.976 fps"}, {"film", "24 fps (Film)"}, {"pal", "25 fps (PAL)"}, {"29.97fps", "29.97 fps"},
    {"29.97df", "29.97 df (drop frame)"}, {"ntsc", "30 fps (NTSC)"}, {"40fps", "40 fps"}, {"47.952fps", "47.952 fps"},
    {"show", "48 fps (Show)"}, {"palf", "50 fps (PAL Field)"}, {"59.94fps", "59.94 fps"}, {"ntscf", "60 fps (NTSC Field)"},
    {"75fps", "75 fps"}, {"80fps", "80 fps"}, {"100fps", "100 fps"}, {"120fps", "120 fps"}, {"125fps", "125 fps"},
    {"150fps", "150 fps"}, {"200fps", "200 fps"}, {"240fps", "240 fps"}, {"250fps", "250 fps"}, {"300fps", "300 fps"},
    {"375fps", "375 fps"}, {"400fps", "400 fps"}, {"500fps", "500 fps"}, {"600fps", "600 fps"}, {"750fps", "750 fps"},
    {"1200fps", "1200 fps"}, {"1500fps", "1500 fps"}, {"2000fps", "2000 fps"}, {"3000fps", "3000 fps"},
    {"6000fps", "6000 fps"}, {"44100fps", "44100 fps"}, {"48000fps", "48000 fps"}, {"millisec", "milliseconds"},
    {"sec", "seconds"}, {"min", "minutes"}, {"hour", "hours"}};

// A combo box of units, its data the unit's name; the current one is `now`, added when it isn't listed (a custom rate).
QComboBox* unit_combo(QWidget* parent, const QList<QPair<QString, QString>>& units, const QString& now) {
    auto* c = new QComboBox(parent);
    for (const auto& u : units) c->addItem(u.second, u.first);
    int i = c->findData(now);
    if (i < 0 && !now.isEmpty()) {
        c->addItem(now, now);
        i = c->count() - 1;
    }
    c->setCurrentIndex(std::max(i, 0));
    return c;
}

QString text_of(const QJsonValue& v) { return v.isArray() ? v.toArray().first().toString() : v.toString(); }
} // namespace

// File > New Scene's options: the units a new scene starts in, kept as the reference's optionVars.
void MainWindow::new_scene_options() {
    QWidget* parent = this;
    auto* d = new QDialog(parent);
    d->setAttribute(Qt::WA_DeleteOnClose);
    d->setWindowTitle("New Scene Options");
    auto* lay = new QVBoxLayout(d);
    auto* f = new QFormLayout();
    lay->addLayout(f);
    auto saved = [](const char* key, const QString& def) {
        QString v = text_of(mel(QString("optionVar -q \"%1\"").arg(key)));
        return v.isEmpty() || v == "0" ? def : v;
    };
    auto* lin = unit_combo(d, kLinearUnits, saved("workingUnitLinearDefault", "cm"));
    auto* ang = unit_combo(d, kAngularUnits, saved("workingUnitAngularDefault", "deg"));
    auto* tim = unit_combo(d, kTimeUnits, saved("workingUnitTimeDefault", "film"));
    f->addRow(new QLabel("<b>Default Working Units</b>", d));
    f->addRow("Linear:", lin);
    f->addRow("Angular:", ang);
    f->addRow("Time:", tim);
    auto* bb = new QDialogButtonBox(d);
    auto* make = bb->addButton("New Scene", QDialogButtonBox::AcceptRole);
    auto* apply = bb->addButton("Apply", QDialogButtonBox::ApplyRole);
    bb->addButton("Close", QDialogButtonBox::RejectRole);
    lay->addWidget(bb);
    auto store = [lin, ang, tim] {
        mel(QString("optionVar -sv \"workingUnitLinearDefault\" \"%1\" -sv \"workingUnitAngularDefault\" \"%2\" -sv \"workingUnitTimeDefault\" \"%3\"")
                .arg(lin->currentData().toString(), ang->currentData().toString(), tim->currentData().toString()));
        exec_quiet("newSceneUnits", QJsonObject{{"linear", lin->currentData().toString()}, {"angle", ang->currentData().toString()}, {"time", tim->currentData().toString()}});
    };
    QObject::connect(apply, &QPushButton::clicked, d, store);
    QObject::connect(make, &QPushButton::clicked, d, [this, d, store] {
        store();
        new_scene();
        d->accept();
    });
    QObject::connect(bb, &QDialogButtonBox::rejected, d, &QDialog::reject);
    d->show();
}

namespace {
void option_var(const QString& key, const QJsonValue& v) {
    const char* flag = v.isString() ? "stringValue" : v.isDouble() && v.toDouble() != int(v.toDouble()) ? "floatValue" : "intValue";
    exec_quiet("optionVar", QJsonObject{{flag, QJsonArray{key, v.isBool() ? QJsonValue(v.toBool() ? 1 : 0) : v}}});
}
} // namespace

Preferences::Preferences(QWidget* parent) : QDialog(parent) {
    setWindowTitle("Preferences");
    auto* lay = new QHBoxLayout(this);
    auto* cats = new QListWidget(this);
    cats->setFixedWidth(150);
    auto* pages = new QStackedWidget(this);
    lay->addWidget(cats);
    auto* right = new QVBoxLayout();
    right->addWidget(pages, 1);
    lay->addLayout(right, 1);
    QSettings s;
    QList<std::function<void(QSettings&)>>* savers = new QList<std::function<void(QSettings&)>>();
    QObject::connect(this, &QObject::destroyed, this, [savers] { delete savers; });

    auto page = [cats, pages, this](const QString& name) {
        auto* w = new QWidget(pages);
        auto* f = new QFormLayout(w);
        pages->addWidget(w);
        cats->addItem(name);
        return f;
    };
    auto check = [savers, &s, this](QFormLayout* f, const QString& label, const QString& key, bool def) {
        auto* c = new QCheckBox(this);
        c->setChecked(s.value("prefs/" + key, def).toBool());
        f->addRow(label + ":", c);
        savers->append([c, key](QSettings& st) {
            st.setValue("prefs/" + key, c->isChecked());
            option_var(key, c->isChecked());
        });
        return c;
    };
    auto number = [savers, &s, this](QFormLayout* f, const QString& label, const QString& key, double def, double lo, double hi, double step, int dec) {
        auto* d = new QDoubleSpinBox(this);
        d->setRange(lo, hi);
        d->setSingleStep(step);
        d->setDecimals(dec);
        d->setValue(s.value("prefs/" + key, def).toDouble());
        f->addRow(label + ":", d);
        savers->append([d, key](QSettings& st) {
            st.setValue("prefs/" + key, d->value());
            option_var(key, d->value());
        });
        return d;
    };
    auto choice = [savers, &s, this](QFormLayout* f, const QString& label, const QString& key, const QStringList& items, const QString& def) {
        auto* c = new QComboBox(this);
        c->addItems(items);
        c->setCurrentText(s.value("prefs/" + key, def).toString());
        f->addRow(label + ":", c);
        savers->append([c, key](QSettings& st) {
            st.setValue("prefs/" + key, c->currentText());
            option_var(key, c->currentText());
        });
        return c;
    };

    QFormLayout* f = page("Interface");
    number(f, "Interface font size (pt)", "fontSize", 9, 7, 16, 1, 0);
    check(f, "Hide the In-View Editor", "inViewEditorOff", false);
    check(f, "Remember window positions", "rememberLayout", true);
    check(f, "Check for updates at start-up", "updateCheck", false);
    f = page("UI Elements");
    check(f, "Status line", "uiStatusLine", true);
    check(f, "Shelf", "uiShelf", true);
    check(f, "Tool box", "uiToolbox", true);
    check(f, "Time slider", "uiTimeSlider", true);
    check(f, "Command line", "uiCommandLine", true);
    check(f, "Help line", "helpLine", true);
    f = page("Display");
    number(f, "View background (grey)", "background", 0.36, 0, 1, 0.02, 3);
    number(f, "Joint size", "jointScale", 1, 0.01, 100, 0.1, 2);
    number(f, "IK handle size", "ikHandleScale", 1, 0.01, 100, 0.1, 2);
    number(f, "Resolution gate aspect", "resolutionAspect", 960.0 / 540.0, 0.1, 10, 0.01, 3);
    f = page("Selection");
    check(f, "Click and drag select", "clickDrag", false);
    check(f, "Track selection order", "trackSelectionOrder", false);
    check(f, "Preselection highlight", "preSelectHilite", true);
    number(f, "Paint selection radius (pixels)", "paintSelectRadius", 22, 2, 300, 1, 0);
    f = page("Snapping");
    number(f, "Grid spacing", "gridSpacing", 1, 0.001, 1000, 0.1, 3);
    number(f, "Move step (0 off)", "stepMove", 0, 0, 1000, 0.1, 3);
    number(f, "Rotate step in degrees (0 off)", "stepRotate", 0, 0, 180, 1, 2);
    number(f, "Scale step (0 off)", "stepScale", 0, 0, 100, 0.1, 3);
    f = page("Manipulators");
    number(f, "Manipulator size", "manipSize", 1, 0.25, 4, 0.25, 2);
    check(f, "Numeric readout while dragging", "manipReadout", true);
    f = page("Animation");
    QStringList tans{"auto", "spline", "clamped", "linear", "flat", "step", "plateau"};
    choice(f, "Default in tangent", "defaultInTangent", tans, "auto");
    choice(f, "Default out tangent", "defaultOutTangent", tans, "auto");
    check(f, "Auto key on at start", "autoKeyStart", false);
    f = page("Time Slider");
    choice(f, "Key ticks", "keyTicks", {"Active", "Selected", "None"}, "Active");
    number(f, "Key tick width (pixels)", "keyTickSize", 2, 1, 63, 1, 0);
    choice(f, "Playback speed", "playbackSpeed", {"Play every frame", "Real-time", "Half", "Twice"}, "Real-time");
    choice(f, "Looping", "playbackLoop", {"Once", "Oscillate", "Continuous"}, "Continuous");
    f = page("Settings");
    // World Coordinate System: a preference kept between scenes. Saving a change turns the views with the world
    // (upAxis -rv), as the reference does.
    f->addRow(new QLabel("<b>World Coordinate System</b>", this));
    auto* up_row = new QWidget(this);
    auto* up_lay = new QHBoxLayout(up_row);
    up_lay->setContentsMargins(0, 0, 0, 0);
    auto* up_y = new QRadioButton("Y", up_row);
    auto* up_z = new QRadioButton("Z", up_row);
    up_lay->addWidget(up_y);
    up_lay->addWidget(up_z);
    up_lay->addStretch(1);
    const bool z_now = text_of(mel("upAxis -q -ax")) == "z";
    (z_now ? up_z : up_y)->setChecked(true);
    f->addRow("Up axis:", up_row);
    savers->append([up_z, z_now](QSettings&) {
        if (up_z->isChecked() != z_now) az::exec("upAxis", QJsonObject{{"axis", up_z->isChecked() ? "z" : "y"}, {"rotateView", true}});
    });
    // Working Units: the current scene's, saved with it. A new scene starts in File > New Scene's options.
    f->addRow(new QLabel("<b>Working Units</b>", this));
    const QString lin_now = text_of(mel("currentUnit -q -l")), ang_now = text_of(mel("currentUnit -q -a")), time_now = text_of(mel("currentUnit -q -t"));
    auto* lin = unit_combo(this, kLinearUnits, lin_now);
    auto* ang = unit_combo(this, kAngularUnits, ang_now);
    auto* tim = unit_combo(this, kTimeUnits, time_now);
    auto* keep = new QCheckBox("Keep keys at current frame", this);
    keep->setToolTip("When the time unit changes, keys keep their frame numbers instead of their time in seconds.");
    f->addRow("Linear:", lin);
    f->addRow("Angular:", ang);
    f->addRow("Time:", tim);
    f->addRow("", keep);
    savers->append([lin, ang, tim, keep, lin_now, ang_now, time_now](QSettings&) {
        QJsonObject call;
        if (lin->currentData().toString() != lin_now) call["linear"] = lin->currentData().toString();
        if (ang->currentData().toString() != ang_now) call["angle"] = ang->currentData().toString();
        if (tim->currentData().toString() != time_now) {
            call["time"] = tim->currentData().toString();
            call["updateAnimation"] = !keep->isChecked();
        }
        if (!call.isEmpty()) az::exec("currentUnit", call);
    });
    auto* new_opts = new QPushButton("New Scene Options...", this);
    connect(new_opts, &QPushButton::clicked, this, [this] {
        if (auto* w = qobject_cast<MainWindow*>(parentWidget())) w->new_scene_options();
    });
    f->addRow("", new_opts);
    check(f, "Construction history on at start", "constructionHistory", true);
    number(f, "Undo queue length (0 infinite)", "undoQueue", 0, 0, 100000, 10, 0);
    check(f, "Autosave", "autosave", false);
    number(f, "Autosave every (minutes)", "autosaveMinutes", 10, 1, 240, 1, 0);
    f = page("Modeling");
    number(f, "Face normal length", "normalSize", 0.4, 0.001, 100, 0.1, 3);
    check(f, "Keep faces together on extrude", "keepFacesTogether", true);
    auto* note = new QLabel("Files, hotkeys and shelves keep their own settings: see Hotkey Editor and Shelf Editor.", this);
    note->setWordWrap(true);
    f->addRow(note);

    connect(cats, &QListWidget::currentRowChanged, pages, &QStackedWidget::setCurrentIndex);
    cats->setCurrentRow(0);
    auto* bb = new QDialogButtonBox(QDialogButtonBox::Save | QDialogButtonBox::Cancel | QDialogButtonBox::RestoreDefaults, this);
    right->addWidget(bb);
    connect(bb, &QDialogButtonBox::rejected, this, &QDialog::reject);
    connect(bb, &QDialogButtonBox::accepted, this, [this, savers] {
        QSettings st;
        for (auto& fn : *savers) fn(st);
        emit changed();
        accept();
    });
    connect(bb->button(QDialogButtonBox::RestoreDefaults), &QAbstractButton::clicked, this, [this] {
        QSettings().remove("prefs");
        emit changed();
        reject();
    });
    resize(640, 420);
}

void MainWindow::show_preferences() {
    auto* p = new Preferences(this);
    p->setAttribute(Qt::WA_DeleteOnClose);
    connect(p, &Preferences::changed, this, &MainWindow::apply_prefs);
    p->show();
}

void MainWindow::apply_prefs() {
    QSettings s;
    QFont f = qApp->font();
    f.setPointSize(s.value("prefs/fontSize", 9).toInt());
    qApp->setFont(f);
    statusBar()->setVisible(s.value("prefs/helpLine", true).toBool());
    auto bar = [this, &s](const char* name, const char* key) {
        if (QToolBar* t = findChild<QToolBar*>(name)) t->setVisible(s.value(QString("prefs/") + key, true).toBool());
    };
    bar("statusLine", "uiStatusLine");
    bar("shelf", "uiShelf");
    bar("toolbox", "uiToolbox");
    bar("timeBar", "uiTimeSlider");
    bar("commandLine", "uiCommandLine");
    for (ViewportWidget* v : panels_->views()) {
        v->set("background", QString::number(s.value("prefs/background", 0.36).toDouble()));
        v->set("manipSize", QString::number(s.value("prefs/manipSize", 1.0).toDouble()));
        v->set("stepMove", QString::number(s.value("prefs/stepMove", 0.0).toDouble()));
        v->set("stepRotate", QString::number(s.value("prefs/stepRotate", 0.0).toDouble()));
        v->set("stepScale", QString::number(s.value("prefs/stepScale", 0.0).toDouble()));
        v->set("disp", QString("{\"normalSize\":%1}").arg(s.value("prefs/normalSize", 0.4).toDouble()));
    }
    // The grid goes through the grid command: its numbers are in the current linear unit, as the reference's are.
    exec_quiet("grid", QJsonObject{{"spacing", s.value("prefs/gridSpacing", 1.0).toDouble() * 5.0}, {"divisions", 5}, {"size", s.value("prefs/gridSize", 12.0).toDouble()}});
    auto ui_pref = [](const char* name, const QJsonValue& v) { exec_quiet("uiPref", QJsonObject{{"args", QJsonArray{name, v}}}); };
    ui_pref("manipReadout", s.value("prefs/manipReadout", true).toBool());
    ui_pref("paintSelectRadius", s.value("prefs/paintSelectRadius", 22.0).toDouble());
    ui_pref("jointScale", s.value("prefs/jointScale", 1.0).toDouble());
    ui_pref("ikHandleScale", s.value("prefs/ikHandleScale", 1.0).toDouble());
    ui_pref("resolutionAspect", s.value("prefs/resolutionAspect", 960.0 / 540.0).toDouble());
    exec_quiet("selectPref", QJsonObject{{"clickDrag", s.value("prefs/clickDrag", false).toBool()},
                                         {"trackSelectionOrder", s.value("prefs/trackSelectionOrder", false).toBool()},
                                         {"preSelectHilite", s.value("prefs/preSelectHilite", true).toBool()}});
    exec_quiet("keyTangent", QJsonObject{{"global", true}, {"inTangentType", s.value("prefs/defaultInTangent", "auto").toString()}, {"outTangentType", s.value("prefs/defaultOutTangent", "auto").toString()}});
    int undo_len = s.value("prefs/undoQueue", 0).toInt();
    exec_quiet("undoInfo", QJsonObject{{"limit", undo_len}});
    if (time_slider_) time_slider_->set_tick_width(s.value("prefs/keyTickSize", 2).toInt());
    time_slider_->set_ticks_shown(s.value("prefs/keyTicks", "Active").toString() != "None");
    play_every_frame_ = s.value("prefs/playbackSpeed", "Real-time").toString() == "Play every frame";
    QString sp = s.value("prefs/playbackSpeed", "Real-time").toString();
    play_speed_ = sp == "Half" ? 0.5 : sp == "Twice" ? 2.0 : 1.0;
    loop_mode_ = s.value("prefs/playbackLoop", "Continuous").toString();
    if (s.value("prefs/autosave", false).toBool()) {
        if (!autosave_) {
            autosave_ = new QTimer(this);
            connect(autosave_, &QTimer::timeout, this, [this] {
                QString file = state()["file"].toString();
                // Beside the scene, numbered; an untitled scene isn't autosaved.
                if (file.isEmpty()) return;
                QString to = file + ".autosave";
                aztek_free(aztek_file("save", to.toUtf8().constData()));
                aztek_free(aztek_file("save", file.toUtf8().constData()));
                log("// Autosaved " + to + " //");
            });
        }
        autosave_->start(s.value("prefs/autosaveMinutes", 10).toInt() * 60000);
    } else if (autosave_) {
        autosave_->stop();
    }
}

} // namespace az
