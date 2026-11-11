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
#include <QScrollArea>
#include <QCheckBox>
#include <QComboBox>
#include <QDialogButtonBox>
#include <QDoubleSpinBox>
#include <QFileDialog>
#include <QLineEdit>
#include <QMessageBox>
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
        // Scrolls when a page is taller than the window.
        auto* scroll = new QScrollArea(pages);
        scroll->setWidgetResizable(true);
        scroll->setFrameShape(QFrame::NoFrame);
        scroll->viewport()->setAutoFillBackground(false);
        auto* w = new QWidget(scroll);
        w->setAutoFillBackground(false);
        auto* f = new QFormLayout(w);
        scroll->setWidget(w);
        pages->addWidget(scroll);
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
    // User Folder: where scripts, plug-ins, modules and preferences kept between versions are looked for. Aztek's
    // own, an existing folder used where it is, or any other folder laid out the same way.
    f = page("User Folder");
    {
        const QJsonObject uf = ask("aztekUserFolder", QJsonObject{{"query", true}}).toObject();
        const QString mode = uf["mode"].toString("aztek"), detected = uf["detected"].toString();
        auto* own = new QRadioButton("Aztek's own folder", this);
        auto* link = new QRadioButton("Use a Maya folder where it is", this);
        auto* custom = new QRadioButton("A custom folder", this);
        own->setObjectName("userFolderOwn");
        link->setObjectName("userFolderLink");
        custom->setObjectName("userFolderCustom");
        (mode == "link" ? link : mode == "custom" ? custom : own)->setChecked(true);
        auto folder_row = [this](const QString& text, const QString& title) {
            auto* w = new QWidget(this);
            auto* h = new QHBoxLayout(w);
            h->setContentsMargins(22, 0, 0, 0);
            auto* edit = new QLineEdit(text, w);
            auto* browse = new QPushButton("Browse...", w);
            h->addWidget(edit, 1);
            h->addWidget(browse);
            connect(browse, &QPushButton::clicked, this, [this, edit, title] {
                const QString d = QFileDialog::getExistingDirectory(this, title, edit->text());
                if (!d.isEmpty()) edit->setText(d);
            });
            return std::make_pair(w, edit);
        };
        auto note = [this](const QString& text) {
            auto* l = new QLabel(text, this);
            l->setWordWrap(true);
            l->setContentsMargins(22, 0, 0, 6);
            l->setStyleSheet("color: palette(mid);");
            return l;
        };
        f->addRow(own);
        f->addRow(note(uf["ownFolder"].toString() + ", with scripts, plug-ins, modules and prefs folders inside."));
        f->addRow(link);
        auto [link_w, link_edit] = folder_row(mode == "link" ? uf["folder"].toString() : detected, "Maya Folder");
        link_edit->setPlaceholderText(detected.isEmpty() ? "No Maya folder was found: browse to one" : detected);
        f->addRow(link_w);
        auto* version = new QComboBox(this);
        auto fill_versions = [version, link_edit](const QString& keep) {
            version->clear();
            version->addItem("Newest", "");
            const QJsonArray vs = ask("aztekUserFolder", QJsonObject{{"query", true}, {"listVersions", link_edit->text()}}).toArray();
            for (const QJsonValue& v : vs) version->addItem(v.toString(), v.toString());
            const int i = version->findData(keep);
            version->setCurrentIndex(i < 0 ? 0 : i);
        };
        fill_versions(mode == "link" ? uf["version"].toString() : QString());
        connect(link_edit, &QLineEdit::editingFinished, this, [fill_versions, version] { fill_versions(version->currentData().toString()); });
        auto* version_w = new QWidget(this);
        auto* version_h = new QHBoxLayout(version_w);
        version_h->setContentsMargins(22, 0, 0, 0);
        version_h->addWidget(new QLabel("Version:", version_w));
        version_h->addWidget(version);
        version_h->addStretch(1);
        f->addRow(version_w);
        f->addRow(note("Its scripts, plug-ins, modules, Maya.env and saved preferences are read from there. Aztek never writes into it."));
        f->addRow(custom);
        auto [custom_w, custom_edit] = folder_row(mode == "custom" ? uf["folder"].toString() : QString(), "User Folder");
        f->addRow(custom_w);
        f->addRow(note("Any folder laid out the same way: scripts, plug-ins, modules and prefs, with or without a version's folder."));
        auto enable = [own, link, custom, link_w, version_w, custom_w] {
            link_w->setEnabled(link->isChecked());
            version_w->setEnabled(link->isChecked());
            custom_w->setEnabled(custom->isChecked());
        };
        enable();
        for (QRadioButton* r : {own, link, custom}) connect(r, &QRadioButton::toggled, this, enable);
        auto* user_setup = new QCheckBox("Run userSetup.mel and userSetup.py at start", this);
        user_setup->setChecked(uf["userSetup"].toBool(true));
        f->addRow(user_setup);

        auto* copy = new QPushButton("Copy a Maya Folder into Aztek's...", this);
        copy->setToolTip("Copies its scripts, plug-ins, modules and preferences into Aztek's own folder and uses that. Nothing already there is replaced, and the folder copied from is left as it is.");
        auto* shelves = new QPushButton("Import Its Shelves", this);
        shelves->setToolTip("Adds the shelves saved in the folder in use (prefs/shelves/shelf_*.mel) to the shelf bar.");
        auto* plugins = new QPushButton("Plug-in Manager...", this);
        auto* actions = new QWidget(this);
        auto* actions_h = new QHBoxLayout(actions);
        actions_h->setContentsMargins(0, 8, 0, 0);
        actions_h->addWidget(copy);
        actions_h->addWidget(shelves);
        actions_h->addWidget(plugins);
        actions_h->addStretch(1);
        f->addRow(actions);
        auto* summary = new QLabel(this);
        summary->setObjectName("userFolderSummary");
        summary->setWordWrap(true);
        summary->setTextInteractionFlags(Qt::TextSelectableByMouse);
        f->addRow(summary);
        auto describe = [summary, shelves](const QJsonObject& u) {
            auto there = [](const QJsonValue& list) {
                QStringList out;
                for (const QJsonValue& v : list.toArray())
                    if (v["exists"].toBool()) out << v["path"].toString();
                return out;
            };
            QStringList mods;
            for (const QJsonValue& m : u["found"].toArray()) mods << m["name"].toString() + " " + m["version"].toString();
            const int n_shelves = u["shelves"].toArray().size();
            const QStringList scripts = there(u["scripts"]), plugs = there(u["plugins"]);
            // Short, so the page fits: the folders themselves are in the tooltip.
            summary->setText(QString("<b>In use:</b> %1%2<br>Script folders: %3. Plug-in folders: %4. Saved shelves: %5.<br>Modules: %6")
                                 .arg(u["root"].toString().toHtmlEscaped(), u["usedVersion"].toString().isEmpty() ? QString() : " (version " + u["usedVersion"].toString().toHtmlEscaped() + ")")
                                 .arg(scripts.size())
                                 .arg(plugs.size())
                                 .arg(n_shelves)
                                 .arg(mods.isEmpty() ? QString("(none)") : mods.join(", ").toHtmlEscaped()));
            summary->setToolTip("Script folders:\n" + (scripts.isEmpty() ? QString("(none there yet)") : scripts.join("\n")) + "\n\nPlug-in folders:\n" + (plugs.isEmpty() ? QString("(none there yet)") : plugs.join("\n")));
            shelves->setEnabled(n_shelves > 0);
        };
        describe(uf);
        connect(plugins, &QPushButton::clicked, this, [this] {
            if (auto* w = qobject_cast<MainWindow*>(parentWidget())) w->show_plugin_manager();
        });
        connect(shelves, &QPushButton::clicked, this, [this] {
            QStringList files;
            for (const QJsonValue& v : ask("aztekUserFolder", QJsonObject{{"query", true}}).toObject()["shelves"].toArray()) files << v.toString();
            if (auto* w = qobject_cast<MainWindow*>(parentWidget())) {
                const int n = w->import_shelves(files);
                QMessageBox::information(this, "Import Shelves", QString("%1 shelf file%2 found, %3 added. A shelf already on the bar by the same name is left as it is.").arg(files.size()).arg(files.size() == 1 ? "" : "s").arg(n));
            }
        });
        connect(copy, &QPushButton::clicked, this, [this, link_edit, version, own, describe] {
            const QString from = QFileDialog::getExistingDirectory(this, "Folder to Copy", link_edit->text());
            if (from.isEmpty()) return;
            const QString to = ask("aztekUserFolder", QJsonObject{{"query", true}}).toObject()["ownFolder"].toString();
            if (QMessageBox::question(this, "Copy Folder", QString("Copy the scripts, plug-ins, modules and preferences of\n%1\ninto\n%2 ?\n\nNothing already there is replaced, and the folder copied from is left as it is.").arg(from, to)) != QMessageBox::Yes) return;
            QApplication::setOverrideCursor(Qt::WaitCursor);
            const QJsonObject r = az::exec("aztekUserFolder", QJsonObject{{"copyFrom", from}, {"version", version->currentData().toString()}}).toObject();
            QApplication::restoreOverrideCursor();
            if (r.isEmpty()) {
                QMessageBox::warning(this, "Copy Folder", "The copy failed. The reason is in the Script Editor.");
                return;
            }
            own->setChecked(true);
            describe(ask("aztekUserFolder", QJsonObject{{"query", true}}).toObject());
            QString text = QString("%1 files copied, %2 already there and kept.").arg(r["copied"].toInt()).arg(r["kept"].toInt());
            const int failed = r["failed"].toArray().size();
            if (failed > 0) text += QString("\n%1 could not be copied; the first is %2").arg(failed).arg(r["failed"].toArray().first().toString());
            QMessageBox::information(this, "Copy Folder", text);
        });
        savers->append([own, link, custom, link_edit, custom_edit, version, user_setup, uf](QSettings&) {
            const QString mode = link->isChecked() ? "link" : custom->isChecked() ? "custom" : "aztek";
            const QString folder = link->isChecked() ? link_edit->text().trimmed() : custom->isChecked() ? custom_edit->text().trimmed() : QString();
            const QString ver = link->isChecked() ? version->currentData().toString() : QString();
            // The folder in use may have changed under a Copy since the page was made: compare with what is saved now.
            const QJsonObject now = ask("aztekUserFolder", QJsonObject{{"query", true}}).toObject();
            if (mode == now["mode"].toString() && folder == now["folder"].toString() && ver == now["version"].toString() && user_setup->isChecked() == now["userSetup"].toBool(true)) return;
            if (mode != "aztek" && folder.isEmpty()) {
                log("// Warning: User Folder: no folder was given, so the choice was left as it was //", 4);
                return;
            }
            QJsonObject call{{"mode", mode}, {"userSetup", user_setup->isChecked()}};
            if (mode != "aztek") {
                call["folder"] = folder;
                call["version"] = ver;
            }
            if (!az::exec("aztekUserFolder", call).isUndefined())
                log("// The user folder's scripts, plug-ins and modules are in use now. Its userSetup scripts and saved preferences are read when Aztek starts. //");
        });
    }
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
    resize(640, 540);
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
