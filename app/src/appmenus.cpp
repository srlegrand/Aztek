// The menus every menu set has (File, Edit, Select, Modify, Display, Windows), the marking menus, workspaces and the small
// actions they share.
#include "mainwindow.h"
#include "renderui.h"
#include "aztek.h"
#include "colorchooser.h"
#include "editors.h"
#include "host.h"
#include "fbxui.h"
#include "fileoptions.h"
#include "usdui.h"
#include "icons.h"
#include "markingmenu.h"
#include "panels.h"
#include "shelves.h"
#include "toolsettings.h"
#include "viewport.h"
#include "widgets.h"

#include <QHash>
#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QAction>
#include <QApplication>
#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QVBoxLayout>
#include <QDir>
#include <QDockWidget>
#include <QDoubleSpinBox>
#include <QFileDialog>
#include <QFileInfo>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QJsonArray>
#include <QJsonDocument>
#include <QLabel>
#include <QLineEdit>
#include <QMenuBar>
#include <QMessageBox>
#include <QMouseEvent>
#include <QPushButton>
#include <QRegularExpression>
#include <QSettings>
#include <QStatusBar>
#include <QTimer>
#include <QToolBar>
#include <QVBoxLayout>
#include <algorithm>
#include <cmath>

namespace az {

QWidget* human_ik_panel(QWidget* parent);

namespace {
// Whether a plug-in is loaded: the file formats plug-ins bring are offered only while they are.
bool plugin_loaded(const QString& name) {
    return ask("pluginInfo", QJsonObject{{"args", QJsonArray{name}}, {"query", true}, {"loaded", true}}).toBool();
}

} // namespace

// A file imported by the options its type's import keeps: the Import browser's and a drop on the window take the same
// way, so a dropped file is read as File > Import would read it (#110).
void import_file(const QString& f) {
        const bool is_fbx = f.endsWith(".fbx", Qt::CaseInsensitive);
        if (is_fbx || f.endsWith(".glb", Qt::CaseInsensitive) || f.endsWith(".gltf", Qt::CaseInsensitive)) {
            if (!plugin_loaded(is_fbx ? "fbx" : "gltf")) {
                log(QString("// Error: %1 files are read by the %2 plug-in, which is not loaded. Load it in Windows > Settings/Preferences > Plug-in Manager. //").arg(is_fbx ? "FBX" : "glTF", is_fbx ? "fbx" : "gltf"), 2);
                return;
            }
            if (is_fbx) {
                // By the FBX Import settings, as the reference's plug-in reads them.
                fbx_apply_saved();
                exec("FBXImport", QJsonObject{{"file", f}});
            } else {
                // The file's up axis as the glTF import options keep it: Y Up (the format's), Z Up, or As Is.
                static const char* kUp[] = {"y", "z", "none"};
                exec("importGltf", QJsonObject{{"args", QJsonArray{f}}, {"upAxis", kUp[std::clamp(QSettings().value("gltf/importUp", 0).toInt(), 0, 2)]}});
            }
            return;
        }
        if (is_usd_file(f)) {
            if (!plugin_loaded("mayaUsdPlugin")) {
                log("// Error: USD files are read by the mayaUsdPlugin plug-in, which is not loaded. Load it in Windows > Settings/Preferences > Plug-in Manager. //", 2);
                return;
            }
            usd_import(f);
            return;
        }
        if (f.endsWith(".ply", Qt::CaseInsensitive) || f.endsWith(".stl", Qt::CaseInsensitive)) {
            exec(f.endsWith(".ply", Qt::CaseInsensitive) ? "importPly" : "importStl", QJsonObject{{"args", QJsonArray{f}}});
            return;
        }
        if (f.endsWith(".abc", Qt::CaseInsensitive) && !plugin_loaded("AbcImport")) {
            log("// Error: Alembic files are read by the AbcImport plug-in, which is not loaded. Load it in Windows > Settings/Preferences > Plug-in Manager. //", 2);
            return;
        }
        // Scenes, and OBJ through the objExport plug-in, which the file command names when it isn't loaded.
        exec("file", QJsonObject{{"args", QJsonArray{f}}, {"import", true}});
}

namespace {

} // namespace

namespace {

QJsonObject args(const QJsonArray& a) {
    QJsonObject o;
    o["args"] = a;
    return o;
}

QStringList selection_names() {
    QStringList out;
    for (const QJsonValue& v : state()["selection"].toArray()) out << v.toString();
    return out;
}

// Each item of the hierarchy under a node, itself first.
void hierarchy_of(const QJsonObject& n, QStringList& out) {
    out << n["path"].toString();
    for (const QJsonValue& c : n["children"].toArray()) hierarchy_of(c.toObject(), out);
}

void walk(const QJsonArray& nodes, const std::function<void(const QJsonObject&)>& fn) {
    for (const QJsonValue& v : nodes) {
        fn(v.toObject());
        walk(v.toObject()["children"].toArray(), fn);
    }
}

// The window asks a name and acts on each selected node's hierarchy.
void rename_hierarchy(QWidget* parent, const std::function<QString(const QString&)>& next) {
    QStringList sel = selection_names();
    if (sel.isEmpty()) return;
    QStringList all;
    walk(query("outliner", "shapes").toArray(), [&](const QJsonObject& n) {
        for (const QString& s : sel)
            if (n["path"].toString() == s || n["path"].toString().startsWith(s + "|") || n["name"].toString() == s) {
                hierarchy_of(n, all);
                return;
            }
    });
    all.removeDuplicates();
    // Deepest first, so paths above stay good while renaming.
    std::sort(all.begin(), all.end(), [](const QString& a, const QString& b) { return a.count('|') > b.count('|'); });
    (void)parent;
    for (const QString& p : all) {
        QString leaf = p.section('|', -1);
        QString to = next(leaf);
        if (to != leaf) exec("rename", args(QJsonArray{p, to}));
    }
}

} // namespace

// ---------------------------------------------------------------------------------------------------------------------
// File

void MainWindow::increment_save() {
    QString f = state()["file"].toString();
    if (f.isEmpty()) {
        save_scene(true);
        return;
    }
    // name.0003.ma after name.0002.ma; name.ma is copied to name.0001.ma, as the reference's.
    QFileInfo fi(f);
    QString stem = fi.completeBaseName();
    QRegularExpression re("^(.*?)\\.?(\\d+)$");
    auto m = re.match(stem);
    QString base = m.hasMatch() ? m.captured(1) : stem;
    int width = m.hasMatch() ? m.captured(2).size() : 4;
    int n = m.hasMatch() ? m.captured(2).toInt() + 1 : 1;
    QString next = fi.dir().filePath(QString("%1.%2.%3").arg(base).arg(n, width, 10, QChar('0')).arg(fi.suffix()));
    QString err = take(aztek_file("save", next.toUtf8().constData()));
    Host::get()->poke();
    if (err.isEmpty()) update_title();
}

void MainWindow::set_project() {
    QString d = QFileDialog::getExistingDirectory(this, "Set Project", QSettings().value("project/current").toString());
    if (d.isEmpty()) return;
    exec("workspace", QJsonObject{{"openWorkspace", d}});
    add_recent_project(d);
}

void MainWindow::add_recent_project(const QString& dir) {
    QSettings s;
    s.setValue("project/current", dir);
    QStringList r = s.value("project/recent").toStringList();
    r.removeAll(dir);
    r.prepend(dir);
    s.setValue("project/recent", r.mid(0, 10));
}

// File > Recent Files and the welcome screen's list: the scenes opened or saved, newest first. A scene's name changing
// by any route (Open, Save As, Increment and Save, a script's file -save or -rename) puts it at the top.
void MainWindow::add_recent_file(const QString& path) {
    if (path.isEmpty()) return;
    QSettings s;
    QStringList r = s.value("recentFiles").toStringList();
    r.removeAll(path);
    r.prepend(path);
    s.setValue("recentFiles", r.mid(0, 10));
}

QMenu* MainWindow::file_menu() {
    auto* m = new QMenu("File", this);
    m->addAction(new OptionRow("New Scene", "Ctrl+N", [this] { new_scene(); }, [this] { new_scene_options(); }, m));
    m->addAction(icon("open"), "Open Scene...\tCtrl+O", this, &MainWindow::open_scene);
    m->addAction(icon("save"), "Save Scene\tCtrl+S", [this] { save_scene(false); });
    m->addAction("Save Scene As...\tCtrl+Shift+S", [this] { save_scene(true); });
    m->addAction("Increment and Save\tCtrl+Alt+S", [this] { increment_save(); });
    m->addAction("Revert", [this] {
        QString f = state()["file"].toString();
        if (f.isEmpty()) {
            log("// Warning: the scene has no file to revert to //", 4);
            return;
        }
        if (QMessageBox::question(this, "Revert", "Discard changes and open " + f + " again?") == QMessageBox::Yes) open_file(f);
    });
    m->addAction("Archive Scene", [this] {
        // As the reference: a scene with unsaved changes is saved first.
        if (state()["modified"].toBool() || state()["file"].toString().isEmpty()) save_scene(false);
        if (state()["modified"].toBool() || state()["file"].toString().isEmpty()) return;
        QJsonValue z = exec("archiveScene");
        if (z.isString()) log("// Result: " + z.toString() + " //", 1);
    });
    m->addAction("Optimize Scene Size", [this] { run_command("OptimizeScene"); });
    m->addSeparator();
    // Import through the file browser, opened on the File type the Import Options were left on.
    auto do_import = [](const QString& type) {
        // The formats plug-ins read are offered while their plug-ins are loaded, as the reference lists a translator's.
        const bool fbx = plugin_loaded("fbx"), gltf = plugin_loaded("gltf"), obj = plugin_loaded("objExport"), usd = plugin_loaded("mayaUsdPlugin");
        QString every = "*.ma *.mb *.ply *.stl";
        QString extra;
        if (obj) {
            every += " *.obj";
            extra += ";;OBJ (*.obj)";
        }
        if (usd) {
            every += " *.usd *.usda *.usdc *.usdz";
            extra += QString(";;") + kUsdFilter;
        }
        if (fbx) {
            every += " *.fbx";
            extra += ";;FBX (*.fbx)";
        }
        if (gltf) {
            every += " *.glb *.gltf";
            extra += ";;glTF 2.0 (*.glb *.gltf)";
        }
        if (plugin_loaded("AbcImport")) {
            every += " *.abc";
            extra += ";;Alembic (*.abc)";
        }
        // A browser that leaves Aztek usable while it is open (#109).
        browse_file_with_options(false, FileAction::Import, "Import", QString(), QString("Scenes and meshes (%1);;mayaAscii (*.ma);;mayaBinary (*.mb)%2;;PLY (*.ply);;STL (*.stl)").arg(every, extra), file_type_filter(type), import_file);
    };
    m->addAction(new OptionRow("Import...", QString(), [do_import] { do_import(chosen_file_type(FileAction::Import)); }, [this, do_import] { file_options_window(this, FileAction::Import, do_import); }, m));
    m->addSeparator();
    {
        auto* row = new OptionRow("Create Reference...", "Ctrl+R", [this] { create_reference_dialog(false); }, [this] { create_reference_dialog(true); }, m);
        m->addAction(row);
    }
    m->addAction("Reference Editor", this, &MainWindow::show_reference_editor);
    m->addSeparator();
    // Export through the file browser, opened on the File type the options were left on; with Default file extensions on,
    // a name given without one gets the type's.
    auto do_export = [](bool all, const QString& type) {
        const QString ext = file_type_extension(type);
        // A browser that leaves Aztek usable while it is open (#109).
        browse_file_with_options(true, all ? FileAction::ExportAll : FileAction::ExportSelection, all ? "Export All" : "Export Selection", ext.isEmpty() ? "export.ma" : "export." + ext,
                                                 QString("mayaAscii (*.ma);;mayaBinary (*.mb)%1%2%3%4%5;;PLY (*.ply);;STL (*.stl)").arg(plugin_loaded("fbx") ? ";;FBX (*.fbx)" : "", plugin_loaded("objExport") ? ";;OBJ (*.obj)" : "", plugin_loaded("mayaUsdPlugin") ? QString(";;") + kUsdFilter : QString(), plugin_loaded("gltf") ? ";;glTF 2.0 (*.glb *.gltf)" : "", plugin_loaded("AbcExport") ? ";;Alembic (*.abc)" : ""), file_type_filter(type), [all, ext](QString f) {
        // glTF without an extension: the kind the options pick.
        const QString add = ext == "gltf" && QSettings().value("gltf/format", 0).toInt() == 0 ? "glb" : ext;
        if (QFileInfo(f).suffix().isEmpty() && default_file_extensions() && !add.isEmpty()) f += "." + add;
        // The exporters are plug-ins: one that has been unloaded says so rather than failing on an unknown command.
        auto plugin_ready = [](const QString& plugin, const QString& what) {
            if (plugin_loaded(plugin)) return true;
            log(QString("// Error: %1 files are written by the %2 plug-in, which is not loaded. Load it in Windows > Settings/Preferences > Plug-in Manager. //").arg(what, plugin), 2);
            return false;
        };
        if (f.endsWith(".fbx", Qt::CaseInsensitive)) {
            if (!plugin_ready("fbx", "FBX")) return;
            // By the FBX Export settings.
            fbx_apply_saved();
            const QJsonObject r = exec("FBXExport", QJsonObject{{"file", f}, {"selected", !all}}).toObject();
            for (const QJsonValue& n : r["notes"].toArray()) log("// Warning: " + n.toString() + " //", 4);
            return;
        }
        if ((f.endsWith(".glb", Qt::CaseInsensitive) || f.endsWith(".gltf", Qt::CaseInsensitive)) && !plugin_ready("gltf", "glTF")) return;
        if (f.endsWith(".glb", Qt::CaseInsensitive) || f.endsWith(".gltf", Qt::CaseInsensitive)) {
            // Its own command: the hierarchy, materials and textures go with the meshes.
            QJsonObject call{{"args", QJsonArray{f}}};
            if (all) call["all"] = true;
            // By the glTF options of the browser's side panel or the Export Options window (#122).
            const QJsonObject flags = gltf_export_flags();
            for (auto it = flags.begin(); it != flags.end(); ++it) call[it.key()] = it.value();
            const QJsonObject r = exec("exportGltf", call).toObject();
            for (const QJsonValue& n : r["notes"].toArray()) log("// Warning: " + n.toString() + " //", 4);
            return;
        }
        if (is_usd_file(f)) {
            if (!plugin_ready("mayaUsdPlugin", "USD")) return;
            usd_export(f, !all);
            return;
        }
        if (f.endsWith(".ma", Qt::CaseInsensitive) || f.endsWith(".mb", Qt::CaseInsensitive)) {
            const char* type = f.endsWith(".mb", Qt::CaseInsensitive) ? "mayaBinary" : "mayaAscii";
            exec("file", QJsonObject{{"args", QJsonArray{f}}, {all ? "exportAll" : "exportSelected", true}, {"type", type}});
            return;
        }
        if (f.endsWith(".abc", Qt::CaseInsensitive)) {
            // Through AbcExport, as Cache > Alembic Cache does it: the playback range, UVs, world space (#87).
            if (!plugin_ready("AbcExport", "Alembic")) return;
            QString job = QString("-frameRange %1 %2 -uvWrite -worldSpace")
                              .arg(ask("playbackOptions", QJsonObject{{"query", true}, {"minTime", true}}).toDouble())
                              .arg(ask("playbackOptions", QJsonObject{{"query", true}, {"maxTime", true}}).toDouble());
            if (!all)
                for (const QJsonValue& n : ask("ls", QJsonObject{{"selection", true}, {"long", true}}).toArray()) job += " -root " + n.toString();
            exec("AbcExport", QJsonObject{{"jobArg", job + " -file \"" + f + "\""}});
            return;
        }
        if (f.endsWith(".obj", Qt::CaseInsensitive)) {
            if (!plugin_ready("objExport", "OBJ")) return;
            exec("file", QJsonObject{{"args", QJsonArray{f}}, {all ? "exportAll" : "exportSelected", true}, {"type", "OBJexport"}});
            return;
        }
        QStringList keep = selection_names();
        if (all) mel("SelectAll;");
        QString c = f.endsWith(".ply", Qt::CaseInsensitive) ? "exportPly" : "exportStl";
        exec(c, args(QJsonArray{f}));
        if (all) {
            QJsonArray a;
            for (const QString& k : keep) a.append(k);
            exec("select", a.isEmpty() ? QJsonObject{{"clear", true}} : args(a));
        }
        });
    };
    m->addAction(new OptionRow("Export All...", QString(), [do_export] { do_export(true, chosen_file_type(FileAction::ExportAll)); },
                               [this, do_export] { file_options_window(this, FileAction::ExportAll, [do_export](const QString& t) { do_export(true, t); }); }, m));
    m->addAction(new OptionRow("Export Selection...", QString(), [do_export] { do_export(false, chosen_file_type(FileAction::ExportSelection)); },
                               [this, do_export] { file_options_window(this, FileAction::ExportSelection, [do_export](const QString& t) { do_export(false, t); }); }, m));
    m->addSeparator();
    QMenu* project = m->addMenu("Project");
    project->addAction("Project Window...", [this] {
        QDialog d(this);
        d.setWindowTitle("Project Window");
        auto* form = new QFormLayout(&d);
        auto* where = new QLineEdit(QSettings().value("project/current", QDir::homePath() + "/aztekProject").toString(), &d);
        auto* browse = new QPushButton("Browse...", &d);
        connect(browse, &QPushButton::clicked, &d, [&] {
            QString x = QFileDialog::getExistingDirectory(&d, "Location", where->text());
            if (!x.isEmpty()) where->setText(x);
        });
        form->addRow("Location:", where);
        form->addRow("", browse);
        form->addRow(new QLabel("Accept creates scenes, scripts, images, sourceimages and data folders there.", &d));
        auto* ok = new QPushButton("Accept", &d);
        form->addRow("", ok);
        connect(ok, &QPushButton::clicked, &d, &QDialog::accept);
        if (d.exec() != QDialog::Accepted || where->text().isEmpty()) return;
        // The project's folder and its standard folders, each with its file rule, saved in its workspace.mel.
        exec("workspace", QJsonObject{{"create", where->text()}});
        exec("workspace", QJsonObject{{"openWorkspace", where->text()}});
        const QList<QPair<QString, QString>> rules{{"scene", "scenes"}, {"scripts", "scripts"}, {"images", "images"}, {"sourceImages", "sourceimages"}, {"diskCache", "data"}};
        for (const auto& r : rules) {
            exec("workspace", QJsonObject{{"create", r.second}});
            exec("workspace", QJsonObject{{"fileRule", QJsonArray{r.first, r.second}}});
        }
        exec("workspace", QJsonObject{{"saveWorkspace", true}});
        add_recent_project(where->text());
    });
    project->addAction("Set Project...", [this] { set_project(); });
    QMenu* examples = m->addMenu("Examples");
    connect(examples, &QMenu::aboutToShow, this, [this, examples] {
        examples->clear();
        // Beside the program, in the source tree when running from a build, and where the Linux packages put them.
        const QString base = QCoreApplication::applicationDirPath();
        auto label_of = [](const QString& name) {
            static const QHash<QString, QString> kept = {{"ik", "IK"}, {"fk", "FK"}, {"uv", "UV"}, {"uvs", "UVs"}, {"usd", "USD"}, {"nurbs", "NURBS"}};
            QStringList words = QFileInfo(name).completeBaseName().split('_', Qt::SkipEmptyParts);
            for (QString& w : words) w = kept.value(w.toLower(), w);
            QString label = words.join(' ');
            if (!label.isEmpty()) label[0] = label[0].toUpper();
            return label;
        };
        auto add_scenes = [this, label_of](QMenu* menu, const QDir& dir) {
            for (const QString& name : dir.entryList({"*.ma"}, QDir::Files, QDir::Name)) {
                // Those that start ref_ are what the references example points at.
                if (name.startsWith("ref_")) continue;
                const QString path = dir.absoluteFilePath(name);
                menu->addAction(label_of(name), [this, path] { if (confirm_unsaved()) open_file(path); });
            }
        };
        for (const QString& where : {base + "/examples", base + "/../examples", base + "/../share/aztek/examples"}) {
            const QDir root(where);
            if (!root.exists() || root.entryList({"*.ma"}, QDir::Files).isEmpty()) continue;
            // A submenu per area (examples/modelling, examples/rigging...), then the scenes at the top, then the scripts.
            for (const QString& sub : root.entryList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name)) {
                const QDir dir(root.absoluteFilePath(sub));
                if (sub == "scripts" || dir.entryList({"*.ma"}, QDir::Files).isEmpty()) continue;
                add_scenes(examples->addMenu(label_of(sub)), dir);
            }
            examples->addSeparator();
            add_scenes(examples, root);
            const QDir scripts(root.absoluteFilePath("scripts"));
            const QStringList files = scripts.entryList({"*.py", "*.mel"}, QDir::Files, QDir::Name);
            if (!files.isEmpty()) {
                QMenu* sm = examples->addMenu("Scripts");
                for (const QString& name : files) {
                    const QString path = scripts.absoluteFilePath(name);
                    sm->addAction(name, [this, path] {
                        show_script_editor();
                        if (script_editor_) script_editor_->load_file(path);
                    });
                }
            }
            break;
        }
        if (examples->isEmpty()) examples->addAction("(none found)")->setEnabled(false);
    });
    QMenu* recent = m->addMenu("Recent Files");
    connect(recent, &QMenu::aboutToShow, this, [this, recent] {
        recent->clear();
        for (const QString& f : QSettings().value("recentFiles").toStringList()) recent->addAction(f, [this, f] { if (confirm_unsaved()) open_file(f); });
        if (recent->isEmpty()) recent->addAction("(none)")->setEnabled(false);
    });
    QMenu* projects = m->addMenu("Recent Projects");
    connect(projects, &QMenu::aboutToShow, this, [this, projects] {
        projects->clear();
        for (const QString& f : QSettings().value("project/recent").toStringList())
            projects->addAction(f, [this, f] {
                exec("workspace", QJsonObject{{"openWorkspace", f}});
                add_recent_project(f);
            });
        if (projects->isEmpty()) projects->addAction("(none)")->setEnabled(false);
    });
    m->addSeparator();
    m->addAction("Run Script...", [] {
        QString f = QFileDialog::getOpenFileName(nullptr, "Run Script", QSettings().value("project/current").toString(), "Scripts (*.mel *.py)");
        if (f.isEmpty()) return;
        QFile file(f);
        if (!file.open(QIODevice::ReadOnly)) return;
        QString src = QString::fromUtf8(file.readAll());
        if (f.endsWith(".py", Qt::CaseInsensitive)) python(src);
        else mel(src);
    });
    m->addSeparator();
    m->addAction("Exit\tCtrl+Q", this, &QWidget::close);
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Edit

void MainWindow::remember_command(const QString& text) {
    QString t = text.trimmed();
    if (t.isEmpty() || t.startsWith("//") || t.startsWith("select ") || t.startsWith("selectMode")) return;
    recent_commands_.removeAll(t);
    recent_commands_.prepend(t);
    while (recent_commands_.size() > 30) recent_commands_.removeLast();
}

QMenu* MainWindow::edit_menu() {
    auto* m = new QMenu("Edit", this);
    m->addAction(icon("undo"), "Undo\tZ", [] { aztek_undo(); Host::get()->poke(); });
    m->addAction(icon("redo"), "Redo\tShift+Z", [] { aztek_redo(); Host::get()->poke(); });
    m->addAction("Repeat\tG", this, &MainWindow::repeat_last);
    QMenu* recents = m->addMenu("Recent Commands List");
    recents->setObjectName("recentCommands");
    connect(recents, &QMenu::aboutToShow, this, [this, recents] {
        recents->clear();
        for (const QString& c : recent_commands_) recents->addAction(c.left(70), [c] { mel(c); });
        if (recents->isEmpty()) recents->addAction("(none)")->setEnabled(false);
    });
    m->addSeparator();
    m->addAction("Cut\tCtrl+X", [this] { run_command("CutSelected"); });
    m->addAction("Copy\tCtrl+C", [this] { run_command("CopySelected"); });
    m->addAction("Paste\tCtrl+V", [this] { run_command("PasteSelected"); });
    m->addSeparator();
    cmd(m, "Delete", "delete", {}, "Delete");
    QMenu* by_type = m->addMenu("Delete by Type");
    cmd(by_type, "History", "delete", QJsonObject{{"constructionHistory", true}}, "Alt+Shift+D");
    // As the reference's: the runtime command, which runs doBakeNonDefHistory with the option box's choice (#172).
    by_type->addAction(new OptionRow("Non-Deformer History", "", [] { mel("BakeNonDefHistory;"); }, [] { OptionsDialog::open("bakePartialHistory", "Delete Non Deformer History"); }, by_type));
    cmd(by_type, "Channels", "cutKey");
    cmd(by_type, "Static Channels", "delete", QJsonObject{{"staticChannels", true}});
    by_type->addAction("Constraints", [] {
        QJsonArray con;
        for (const QJsonValue& s : state()["selection"].toArray())
            for (const QJsonValue& c : ask("listRelatives", QJsonObject{{"args", QJsonArray{s}}, {"type", "constraint"}}).toArray()) con.append(c);
        if (!con.isEmpty()) exec("delete", args(con));
    });
    QMenu* all_type = m->addMenu("Delete All by Type");
    all_type->addAction("History", [] {
        QJsonArray all;
        for (const QJsonValue& v : query("outliner").toArray()) all.append(v.toObject()["path"].toString());
        QJsonObject c = args(all);
        c["constructionHistory"] = true;
        exec("delete", c);
    });
    all_type->addAction(new OptionRow("Non-Deformer History", "", [] { mel("BakeAllNonDefHistory;"); }, [] { OptionsDialog::open("bakePartialHistory", "Delete Non Deformer History"); }, all_type));
    for (const char* t : {"joint", "ikHandle", "nurbsCurve", "locator", "camera"}) {
        QString ty = t;
        QString label = ty.left(1).toUpper() + ty.mid(1) + "s";
        all_type->addAction(label, [ty] {
            QJsonArray hits;
            for (const QJsonValue& n : ask("ls", QJsonObject{{"type", ty}}).toArray()) {
                QString p = n.toString();
                if (ty == "camera" && (p.startsWith("persp") || p.startsWith("top") || p.startsWith("front") || p.startsWith("side"))) continue;
                hits.append(p);
            }
            if (!hits.isEmpty()) exec("delete", args(hits));
        });
    }
    m->addSeparator();
    cmd(m, "Duplicate", "duplicate", {}, "Ctrl+D");
    m->addAction("Duplicate Special\tCtrl+Shift+D", [] { OptionsDialog::open("duplicate", "Duplicate Special"); });
    cmd(m, "Duplicate with Transform", "duplicateWithTransform", {}, "Shift+D");
    cmd(m, "Instance", "instance", {}, QString(), true);
    cmd(m, "Transfer Attribute Values", "transferAttributes", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Group", "group", {}, "Ctrl+G", true);
    cmd(m, "Ungroup", "ungroup", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Parent", "parent", {}, "P", true);
    cmd(m, "Unparent", "parent", QJsonObject{{"world", true}}, "Shift+P");
    m->addSeparator();
    QMenu* tools = m->addMenu("Tools");
    tools->addAction("Select Tool\tQ", [this] { set_tool("select"); });
    tools->addAction("Lasso Tool", [this] { set_tool("lasso"); });
    tools->addAction("Paint Selection Tool", [this] { set_tool("paintSelect"); });
    tools->addAction("Move Tool\tW", [this] { set_tool("move"); });
    tools->addAction("Rotate Tool\tE", [this] { set_tool("rotate"); });
    tools->addAction("Scale Tool\tR", [this] { set_tool("scale"); });
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Select

void MainWindow::select_by_type(const QString& type) {
    QJsonArray hits = ask("ls", QJsonObject{{"type", type}}).toArray();
    if (hits.isEmpty()) {
        log(QString("// Warning: nothing of type %1 //").arg(type), 4);
        return;
    }
    // Shapes select through their transforms, as the reference's All by Type does.
    QJsonArray out;
    for (const QJsonValue& h : hits) {
        QString p = h.toString();
        QJsonArray parents = (type == "joint" || type == "ikHandle") ? QJsonArray() : ask("listRelatives", QJsonObject{{"args", QJsonArray{p}}, {"parent", true}}).toArray();
        out.append(parents.isEmpty() ? p : parents.at(0));
    }
    exec("select", args(out));
}

void MainWindow::select_shell(bool border) {
    QStringList sel = selection_names();
    QRegularExpression re("^(.*)\\.(vtx|e|f)\\[(\\d+)");
    QStringList done;
    bool first = true;
    for (const QString& s : sel) {
        auto m = re.match(s);
        if (!m.hasMatch() || done.contains(m.captured(1))) continue;
        done << m.captured(1);
        QString kind = m.captured(2) == "vtx" ? "vertex" : m.captured(2) == "e" ? "edge" : "face";
        QJsonObject c{{"args", QJsonArray{m.captured(1)}}, {"id", m.captured(3).toInt()}, {"kind", kind}};
        if (!first) c["add"] = true;
        exec("selectConnected", c);
        first = false;
    }
    if (done.isEmpty()) log("// Warning: select a component of the shell //", 4);
    else if (border) exec("selectBorder");
}

QMenu* MainWindow::select_menu() {
    auto* m = new QMenu("Select", this);
    m->addAction("All\tCtrl+Shift+A", [] { mel("SelectAll;"); });
    QMenu* by = m->addMenu("All by Type");
    struct T { const char* label; const char* type; };
    for (T t : {T{"Joints", "joint"}, T{"IK Handles", "ikHandle"}, T{"NURBS Curves", "nurbsCurve"}, T{"Polygon Geometry", "mesh"}, T{"Locators", "locator"}, T{"Cameras", "camera"}, T{"Clusters", "cluster"}, T{"Sets", "objectSet"}}) {
        QString ty = t.type;
        by->addAction(t.label, [this, ty] { select_by_type(ty); });
    }
    cmd(m, "Deselect All", "select", QJsonObject{{"clear", true}});
    cmd(m, "Hierarchy", "selectHierarchy");
    cmd(m, "Inverse", "invertSelection", {}, "Ctrl+Shift+I");
    m->addAction("Similar", [] {
        // Everything of the lead's type.
        QStringList sel = selection_names();
        if (sel.isEmpty()) return;
        QString lead = sel.last().section('.', 0, 0);
        QJsonArray shapes = ask("listRelatives", QJsonObject{{"args", QJsonArray{lead}}, {"shapes", true}}).toArray();
        QString ty = ask("nodeType", args(QJsonArray{shapes.isEmpty() ? lead : shapes.at(0).toString()})).toString();
        QJsonArray hits;
        for (const QJsonValue& h : ask("ls", QJsonObject{{"type", ty}}).toArray()) {
            QJsonArray ps = shapes.isEmpty() ? QJsonArray() : ask("listRelatives", QJsonObject{{"args", QJsonArray{h}}, {"parent", true}}).toArray();
            hits.append(ps.isEmpty() ? h : ps.at(0));
        }
        exec("select", args(hits));
    });
    m->addSeparator();
    m->addAction("Object/Component\tF8", [] { mel("ToggleSelectMode;"); });
    cmd(m, "Vertex", "selectMode", args(QJsonArray{"vertex"}), "F9");
    cmd(m, "Edge", "selectMode", args(QJsonArray{"edge"}), "F10");
    cmd(m, "Face", "selectMode", args(QJsonArray{"face"}), "F11");
    // UVs, UV shells, vertex faces, and Multi: vertices, edges and faces picked together.
    cmd(m, "UV", "selectMode", args(QJsonArray{"uv"}));
    cmd(m, "UV Shell", "selectMode", args(QJsonArray{"uvShell"}));
    cmd(m, "Vertex Face", "selectMode", args(QJsonArray{"vertexFace"}));
    cmd(m, "Multi", "selectMode", args(QJsonArray{"multi"}));
    m->addAction("UV Editor\tF12", [this] { show_uv_editor(); });
    m->addSeparator();
    cmd(m, "Grow", "growSelection", {}, ">");
    cmd(m, "Shrink", "shrinkSelection", {}, "<");
    cmd(m, "Grow Along Loop", "growSelection", QJsonObject{{"alongLoop", true}});
    cmd(m, "Shrink Along Loop", "shrinkSelection", QJsonObject{{"alongLoop", true}});
    m->addSeparator();
    cmd(m, "Contiguous Edges", "selectLoop");
    cmd(m, "Edge Ring", "selectLoop", QJsonObject{{"ring", true}});
    cmd(m, "Border", "selectBorder");
    m->addAction("Shell", [this] { select_shell(false); });
    m->addAction("Shell Border", [this] { select_shell(true); });
    cmd(m, "Select by Angle", "selectByAngle", {}, QString(), true);
    m->addAction("Shortest Edge Path Tool", [this] { set_tool("shortestPath"); });
    if (!has_command_named("polySelectConstraint"))
        todo(m, "Use Constraints...");
    else
        m->addAction("Use Constraints...", [this] { show_named_window("polySelectConstraint"); });
    m->addSeparator();
    m->addAction("Lasso Tool", [this] { set_tool("lasso"); });
    m->addAction("Paint Selection Tool", [this] { set_tool("paintSelect"); });
    m->addSeparator();
    m->addAction("Create Quick Select Set...", [this] {
        bool ok = false;
        QString n = QInputDialog::getText(this, "Create Quick Select Set", "Name:", QLineEdit::Normal, "set1", &ok);
        if (ok && !n.isEmpty()) exec("createSet", QJsonObject{{"name", n}});
    });
    QMenu* quick = m->addMenu("Quick Select Sets");
    connect(quick, &QMenu::aboutToShow, this, [quick] {
        quick->clear();
        for (const QJsonValue& v : query("outliner").toArray()) {
            QJsonObject o = v.toObject();
            if (o["type"].toString() != "objectSet" || o["name"].toString().startsWith("default")) continue;
            QString name = o["name"].toString();
            quick->addAction(name, [name] { exec("selectSetMembers", args(QJsonArray{name})); });
        }
        if (quick->isEmpty()) quick->addAction("(none)")->setEnabled(false);
    });
    m->addSeparator();
    QMenu* conv = m->addMenu("Convert Selection");
    cmd(conv, "To Vertices", "convertSelection", args(QJsonArray{"vertex"}), "Ctrl+F9");
    cmd(conv, "To Edges", "convertSelection", args(QJsonArray{"edge"}), "Ctrl+F10");
    cmd(conv, "To Faces", "convertSelection", args(QJsonArray{"face"}), "Ctrl+F11");
    conv->addSeparator();
    cmd(conv, "To Edge Loop", "selectLoop");
    cmd(conv, "To Edge Ring", "selectLoop", QJsonObject{{"ring", true}});
    cmd(conv, "To Shell Border", "selectBorder");
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Modify

QMenu* MainWindow::modify_menu() {
    auto* m = new QMenu("Modify", this);
    QMenu* xf = m->addMenu("Transformation Tools");
    xf->addAction("Move Tool\tW", [this] { set_tool("move"); });
    xf->addAction("Rotate Tool\tE", [this] { set_tool("rotate"); });
    xf->addAction("Scale Tool\tR", [this] { set_tool("scale"); });
    xf->addAction("Move Normal Tool", [this] {
        set_tool("move");
        exec_quiet("manipMoveContext", QJsonObject{{"edit", true}, {"args", QJsonArray{"Move"}}, {"mode", 3}});
    });
    xf->addAction("Show Manipulator Tool\tT", [] { mel("ShowManipulators;"); });
    cmd(m, "Freeze Transformations", "freezeTransforms", {}, QString(), true);
    cmd(m, "Reset Transformations", "resetTransforms", {}, QString(), true);
    m->addSeparator();
    QMenu* pivot = m->addMenu("Pivot");
    cmd(pivot, "Center Pivot", "centerPivot");
    cmd(pivot, "Bake Pivot", "bakePivot");
    cmd(pivot, "Reset Pivot", "resetPivot");
    cmd(pivot, "Zero Pivot", "zeroPivot");
    QMenu* match = m->addMenu("Match Transformations");
    cmd(match, "Match Translation", "matchTransform", QJsonObject{{"position", true}});
    cmd(match, "Match Rotation", "matchTransform", QJsonObject{{"rotation", true}});
    cmd(match, "Match Scaling", "matchTransform", QJsonObject{{"scale", true}});
    cmd(match, "Match All Transforms", "matchTransform");
    cmd(match, "Match Pivots", "matchPivot");
    QMenu* snap = m->addMenu("Snap Align");
    snap->addAction("Align Objects...", [this] { show_named_window("alignObjects"); });
    snap->addAction("Point to Point", [] {
        // Two vertices: the first's object moves so the first lands on the second.
        QJsonArray sel = ask("ls", QJsonObject{{"selection", true}, {"flatten", true}}).toArray();
        if (sel.size() != 2) {
            log("// Warning: select a vertex on the object to move, then the vertex to snap it to //", 4);
            return;
        }
        QJsonArray a = ask("pointPosition", args(QJsonArray{sel[0]})).toArray(), b = ask("pointPosition", args(QJsonArray{sel[1]})).toArray();
        QString obj = sel[0].toString().section('.', 0, 0);
        exec("selectMode", args(QJsonArray{"object"}));
        exec("select", args(QJsonArray{obj}));
        exec("move", QJsonObject{{"by", QJsonArray{b[0].toDouble() - a[0].toDouble(), b[1].toDouble() - a[1].toDouble(), b[2].toDouble() - a[2].toDouble()}}});
    });
    snap->addAction("Snap Together", [] {
        // Whole objects: the first's pivot onto the second's.
        QStringList sel = selection_names();
        if (sel.size() != 2) {
            log("// Warning: select the object to move, then the one to snap it to //", 4);
            return;
        }
        QJsonArray to = ask("xform", QJsonObject{{"args", QJsonArray{sel[1]}}, {"query", true}, {"worldSpace", true}, {"rotatePivot", true}}).toArray();
        if (to.size() < 3) return;
        exec("select", args(QJsonArray{sel[0]}));
        exec("move", QJsonObject{{"to", QJsonArray{to[0], to[1], to[2]}}});
    });
    m->addSeparator();
    m->addAction("Prefix Hierarchy Names...", [this] {
        bool ok = false;
        QString p = QInputDialog::getText(this, "Prefix Hierarchy Names", "Prefix:", QLineEdit::Normal, "prefix_", &ok);
        if (ok && !p.isEmpty()) rename_hierarchy(this, [p](const QString& n) { return p + n; });
    });
    m->addAction("Search and Replace Names...", [this] { show_named_window("searchReplace"); });
    m->addSeparator();
    m->addAction("Add Attribute...", [this] { show_named_window("addAttribute"); });
    m->addAction("Edit Attribute...", [this] { show_named_window("editAttribute"); });
    m->addAction("Delete Attribute...", [this] { show_named_window("deleteAttribute"); });
    m->addSeparator();
    QMenu* convert = m->addMenu("Convert");
    cmd(convert, "NURBS to Polygons", "nurbsToPoly", QJsonObject{{"format", 3}, {"polygonType", 1}}, QString(), true);
    cmd(convert, "Instance to Object", "makeUnique");
    cmd(convert, "Object to Instance", "convertToInstance");
    cmd(convert, "Polygon Edges to Curve", "polyToCurve", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Soft Select", "softSelect", QJsonObject{{"softSelectEnabled", 1}}, "B", true);
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Display

QMenu* MainWindow::display_menu() {
    auto* m = new QMenu("Display", this);
    QMenu* grid = m->addMenu("Grid");
    grid->addAction("Show", [this] { for (ViewportWidget* v : panels_->views()) v->set("grid", "true"); });
    grid->addAction("Hide", [this] { for (ViewportWidget* v : panels_->views()) v->set("grid", "false"); });
    grid->addAction("Grid Options...", [this] { show_named_window("gridOptions"); });
    QMenu* hud = m->addMenu("Heads Up Display");
    struct H { const char* label; const char* id; };
    QList<QPair<QAction*, QString>> hud_items;
    for (H h : {H{"Poly Count", "polyCount"}, H{"Object Details", "objectDetails"}, H{"Selection", "selection"}, H{"Frame Rate", "frameRate"}, H{"Current Frame", "currentFrame"}, H{"Camera Names", "cameraNames"}, H{"View Axis", "viewAxis"}}) {
        QString id = h.id;
        QAction* a = hud->addAction(h.label);
        a->setCheckable(true);
        connect(a, &QAction::triggered, this, [this, id](bool on) {
            exec("headsUpDisplay", QJsonObject{{"args", QJsonArray{id}}, {"visible", on}});
            panels_->redraw_all();
        });
        hud_items.append({a, id});
    }
    connect(hud, &QMenu::aboutToShow, this, [hud_items] {
        for (const auto& it : hud_items) it.first->setChecked(ask("headsUpDisplay", QJsonObject{{"args", QJsonArray{it.second}}, {"exists", true}}).toBool());
    });
    QMenu* ui = m->addMenu("UI Elements");
    auto ui_item = [ui, this](const QString& label, QWidget* w) {
        QAction* a = ui->addAction(label);
        a->setCheckable(true);
        connect(ui, &QMenu::aboutToShow, a, [a, w] { a->setChecked(w->isVisible()); });
        connect(a, &QAction::triggered, w, [w](bool on) { w->setVisible(on); });
    };
    ui_item("Status Line", findChild<QToolBar*>("statusLine"));
    ui_item("Shelf", findChild<QToolBar*>("shelf"));
    ui_item("Tool Box", findChild<QToolBar*>("toolbox"));
    ui_item("Time Slider", time_dock_);
    ui_item("Range Slider", range_dock_);
    ui_item("Command Line", findChild<QToolBar*>("commandLine"));
    ui_item("Help Line", statusBar());
    ui->addSeparator();
    ui_item("Attribute Editor", attr_dock_);
    ui_item("Tool Settings", tool_dock_);
    ui_item("Channel Box / Layer Editor", channel_dock_);
    ui_item("Outliner", outliner_dock_);
    m->addSeparator();
    QMenu* hide = m->addMenu("Hide");
    cmd(hide, "Hide Selection", "hide", {}, "Ctrl+H");
    hide->addAction("Hide Unselected Objects\tAlt+H", [] {
        // Top level objects other than what is selected and its parents.
        QJsonArray sel = state()["selection"].toArray();
        QJsonArray hidden;
        for (const QJsonValue& o : mel("ls -assemblies;").toArray()) {
            QString name = o.toString();
            bool keep = false;
            for (const QJsonValue& s : sel) {
                QString path = s.toString().section('.', 0, 0);
                if (path == name || path.startsWith("|" + name) || path.startsWith(name + "|")) keep = true;
            }
            if (!keep && !QStringList{"persp", "top", "front", "side"}.contains(name)) hidden.append(name);
        }
        if (!hidden.isEmpty()) exec("hide", QJsonObject{{"args", hidden}});
    });
    QMenu* show = m->addMenu("Show");
    cmd(show, "Show Selection", "show", {}, "Shift+H");
    cmd(show, "All", "showHidden", QJsonObject{{"all", true}});
    cmd(show, "Show Last Hidden", "showLastHidden", {}, "Ctrl+Shift+H");
    // H: a tap runs it, as the reference's; held with a click in a view, the menu sets (#175).
    cmd(m, "Toggle Show/Hide", "toggleVisibility", {}, "H");
    m->addSeparator();
    QMenu* obj = m->addMenu("Object Display");
    cmd(obj, "Template", "toggle", QJsonObject{{"template", true}, {"state", true}});
    cmd(obj, "Untemplate", "toggle", QJsonObject{{"template", true}, {"state", false}});
    // A node's drawing override shows it as its box.
    auto box = [](bool on) {
        return [on] {
            for (const QJsonValue& s : state()["selection"].toArray()) {
                QString o = s.toString().section('.', 0, 0);
                exec("setAttr", QJsonObject{{"args", QJsonArray{o + ".overrideEnabled", on}}});
                exec("setAttr", QJsonObject{{"args", QJsonArray{o + ".overrideLevelOfDetail", on ? 1 : 0}}});
            }
        };
    };
    obj->addAction("Bounding Box", box(true));
    obj->addAction("No Bounding Box", box(false));
    m->addAction("Wireframe Color...", this, [this] { show_wireframe_color_window(this); });
    QMenu* xf = m->addMenu("Transform Display");
    cmd(xf, "Local Rotation Axes", "toggle", QJsonObject{{"localAxis", true}});
    cmd(xf, "Rotate Pivots", "toggle", QJsonObject{{"rotatePivot", true}});
    cmd(xf, "Scale Pivots", "toggle", QJsonObject{{"scalePivot", true}});
    cmd(xf, "Selection Handles", "toggle", QJsonObject{{"selectHandle", true}});
    QMenu* poly = m->addMenu("Polygons");
    auto poly_flag = [this, poly](const QString& label, const char* key) {
        QString k = key;
        QAction* a = poly->addAction(label);
        a->setCheckable(true);
        connect(a, &QAction::triggered, this, [this, k](bool on) {
            for (ViewportWidget* v : panels_->views()) v->set("disp", QString("{\"%1\":%2}").arg(k, on ? "true" : "false"));
        });
        return a;
    };
    poly_flag("Backface Culling", "backface");
    poly_flag("Border Edges", "borderEdges");
    poly_flag("Crease Edges", "creaseEdges");
    poly_flag("Hard Edges", "hardEdges");
    poly_flag("Face Normals", "faceNormals");
    poly_flag("Vertex Normals", "vertexNormals");
    poly_flag("Limit to Selected", "limitToSelected")->setChecked(true);
    poly->addAction("Normals Size...", [this] {
        bool ok = false;
        double x = QInputDialog::getDouble(this, "Normals Size", "Length:", 0.4, 0.001, 100, 3, &ok);
        if (ok)
            for (ViewportWidget* v : panels_->views()) v->set("disp", QString("{\"normalSize\":%1}").arg(x));
    });
    poly->addSeparator();
    cmd(poly, "Soften Edge", "polySoftEdge", QJsonObject{{"angle", 180.0}});
    cmd(poly, "Harden Edge", "polySoftEdge", QJsonObject{{"angle", 0.0}});
    QMenu* nurbs = m->addMenu("NURBS");
    auto curve_attr = [nurbs](const QString& label, const char* attr, bool on) {
        QString a = attr;
        nurbs->addAction(label, [a, on] {
            for (const QString& s : selection_names())
                for (const QJsonValue& sh : ask("listRelatives", QJsonObject{{"args", QJsonArray{s}}, {"shapes", true}}).toArray())
                    exec("setAttr", args(QJsonArray{sh.toString() + "." + a, on}));
        });
    };
    curve_attr("CVs", "dispCV", true);
    curve_attr("No CVs", "dispCV", false);
    curve_attr("Hulls", "dispHull", true);
    curve_attr("No Hulls", "dispHull", false);
    QMenu* anim = m->addMenu("Animation");
    auto size = [this, anim](const QString& label, const char* pref) {
        QString p = pref;
        anim->addAction(label, [this, label, p] {
            bool ok = false;
            double cur = ask("uiPref", args(QJsonArray{p})).toDouble(1.0);
            double x = QInputDialog::getDouble(this, label, "Size:", cur, 0.01, 100, 2, &ok);
            if (!ok) return;
            exec_quiet("uiPref", args(QJsonArray{p, x}));
            QSettings().setValue("prefs/" + p, x);
            panels_->redraw_all();
        });
    };
    size("Joint Size...", "jointScale");
    size("IK Handle Size...", "ikHandleScale");
    QAction* jx = anim->addAction("Joint X-Ray");
    jx->setCheckable(true);
    connect(jx, &QAction::triggered, this, [this](bool on) {
        for (ViewportWidget* v : panels_->views()) v->set("disp", QString("{\"xrayJoints\":%1}").arg(on ? "true" : "false"));
    });
    m->addSeparator();
    QMenu* sp = m->addMenu("Smooth Preview");
    sp->addAction("Off\t1", [] { mel("SmoothPreviewOff;"); });
    sp->addAction("Cage + Smooth\t2", [] { mel("SmoothPreviewCage;"); });
    sp->addAction("Smooth\t3", [] { mel("SmoothPreviewSmooth;"); });
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Windows and workspaces

void MainWindow::show_named_window(const QString& name) {
    if (name == "modelingToolkit") {
        show_modeling_toolkit();
        return;
    }
    if (name == "humanIK") {
        show_human_ik();
        return;
    }
    QWidget* w = named_windows_.value(name);
    if (!w) {
        w = make_editor_window(name, this);
        if (!w) return;
        named_windows_[name] = w;
        connect(w, &QObject::destroyed, this, [this, name] { named_windows_.remove(name); });
    }
    w->show();
    w->raise();
    w->activateWindow();
}

QStringList MainWindow::workspace_names() {
    // The reference's list, with Aztek's names for its systems (the host's workspaceLayoutManager holds it); MASH and
    // Motion Graphics are left out until TRASH is in.
    static QStringList names;
    if (names.isEmpty())
        for (const QJsonValue& v : ask("workspaceLayoutManager", QJsonObject{{"listLayouts", true}}).toArray()) names << v.toString();
    return names;
}

// Humanoid IK as a tab on the right with the Channel Box and the Attribute Editor, not a window of its own (#177).
void MainWindow::show_human_ik() {
    if (!hik_dock_) {
        hik_dock_ = new QDockWidget("Humanoid IK", this);
        hik_dock_->setObjectName("humanIKDock");
        hik_dock_->setWidget(human_ik_panel(hik_dock_));
        addDockWidget(Qt::RightDockWidgetArea, hik_dock_);
        tabifyDockWidget(attr_dock_, hik_dock_);
    }
    hik_dock_->show();
    hik_dock_->raise();
}

// The panels as a fresh layout has them: the Outliner and the Tool Settings tabbed on the left, the Channel Box, the
// Attribute Editor and the other panels tabbed on the right, rather than stacked as another workspace may have left
// them (#177).
void MainWindow::default_dock_layout() {
    addDockWidget(Qt::LeftDockWidgetArea, outliner_dock_);
    addDockWidget(Qt::LeftDockWidgetArea, tool_dock_);
    tabifyDockWidget(outliner_dock_, tool_dock_);
    addDockWidget(Qt::RightDockWidgetArea, channel_dock_);
    for (QDockWidget* d : {attr_dock_, mtk_dock_, hik_dock_}) {
        if (!d) continue;
        addDockWidget(Qt::RightDockWidgetArea, d);
        tabifyDockWidget(channel_dock_, d);
    }
    for (QDockWidget* d : {time_dock_, range_dock_}) d->setFloating(false);
    addDockWidget(Qt::BottomDockWidgetArea, time_dock_, Qt::Vertical);
    splitDockWidget(time_dock_, range_dock_, Qt::Vertical);
}

// An editor window docked into the main window for a workspace; release_docked_editors gives it back as a window.
QDockWidget* MainWindow::dock_editor(const QString& id, QWidget* editor, const QString& title, Qt::DockWidgetArea area) {
    if (!editor) return nullptr;
    QDockWidget* d = ws_docks_.value(id);
    if (!d) {
        d = new QDockWidget(title, this);
        d->setObjectName("workspace_" + id);
        ws_docks_[id] = d;
    }
    editor->setWindowFlags(Qt::Widget);
    d->setWidget(editor);
    editor->show();
    d->setFloating(false);
    addDockWidget(area, d, area == Qt::BottomDockWidgetArea ? Qt::Vertical : Qt::Horizontal);
    d->show();
    if (area == Qt::BottomDockWidgetArea) seat_slider_docks();
    return d;
}

// The Render View, Hypershade, UV Editor and Node Editor open as floating panels: dragged by the title onto an edge of
// the main window or onto another panel, they dock there, beside it or as a tab, as the reference's do (#184).
QDockWidget* MainWindow::float_dockable(const QString& id, QWidget* editor, const QString& title) {
    if (!editor) return nullptr;
    if (QDockWidget* d = ws_docks_.value(id); d && d->widget() == editor) {
        d->show();
        d->raise();
        return d;
    }
    const QSize size = editor->size().expandedTo(QSize(420, 320));
    QDockWidget* d = dock_editor(id, editor, title, Qt::RightDockWidgetArea);
    d->setFloating(true);
    d->resize(size);
    const QRect g = geometry();
    d->move(g.x() + (g.width() - d->width()) / 2, g.y() + 120);
    d->show();
    d->raise();
    return d;
}

QMenu* MainWindow::createPopupMenu() {
    QDockWidget* on = nullptr;
    for (QDockWidget* d : findChildren<QDockWidget*>()) {
        if (!d->isVisible()) continue;
        const QPoint at = d->mapFromGlobal(QCursor::pos());
        // The title bar: above the panel's own widget.
        const int top = d->widget() ? d->widget()->geometry().top() : 24;
        if (at.x() >= 0 && at.x() < d->width() && at.y() >= 0 && at.y() < top) on = d;
    }
    if (!on) return QMainWindow::createPopupMenu();
    auto* m = new QMenu(this);
    if (!on->isFloating()) m->addAction("Undock Tab", [on] { on->setFloating(true); });
    m->addAction("Close Tab", [on] { on->close(); });
    return m;
}

void MainWindow::release_docked_editors() {
    for (QDockWidget* d : ws_docks_) {
        if (QWidget* w = d->widget()) {
            d->setWidget(nullptr);
            w->setParent(this, editor_window_flags());
            w->hide();
        }
        removeDockWidget(d);
        d->deleteLater();
    }
    ws_docks_.clear();
}

void MainWindow::apply_workspace(const QString& name) {
    QSettings s;
    if (workspace_box_) {
        QSignalBlocker b(workspace_box_);
        if (workspace_box_->findText(name) < 0) workspace_box_->addItem(name);
        workspace_box_->setCurrentText(name);
    }
    s.setValue("workspaces_current", name);
    query("uiWorkspace", name);
    release_docked_editors();
    // A saved one first: the arrangement the user kept under this name.
    QByteArray saved = s.value("workspaces/" + name).toByteArray();
    if (!saved.isEmpty()) {
        restore_layout(saved);
        return;
    }
    if (!workspace_names().contains(name)) return;
    auto bar = [this](const char* id, bool on) {
        if (auto* b = findChild<QWidget*>(id, Qt::FindDirectChildrenOnly)) b->setVisible(on);
    };
    // The panels on the edges; the rest of the reference's editors are docked in below.
    auto only = [this](bool outliner, bool channels, bool attr, bool tool) {
        default_dock_layout();
        outliner_dock_->setVisible(outliner);
        channel_dock_->setVisible(channels);
        attr_dock_->setVisible(attr);
        tool_dock_->setVisible(tool);
        if (mtk_dock_) mtk_dock_->hide();
        if (hik_dock_) hik_dock_->hide();
    };
    auto set_menus = [this](const QString& set) {
        set_menu_set(set);
    };
    auto shelf = [this](const QString& n) {
        if (shelf_bar_) shelf_bar_->select_shelf(n);
    };
    for (const char* id : {"statusLine", "shelf", "toolbox", "TimeSlider", "RangeSlider", "commandLine"}) bar(id, true);
    panels_->set_layout(0);
    if (name == "General" || name == "Bicrust Fluids") {
        only(true, true, true, false);
        outliner_dock_->raise();
        channel_dock_->raise();
    } else if (name == "Modeling - Standard") {
        // The Modeling Toolkit on the right, tabbed with the Attribute Editor; no time slider.
        only(false, false, true, false);
        show_modeling_toolkit();
        if (mtk_dock_) {
            addDockWidget(Qt::RightDockWidgetArea, mtk_dock_);
            tabifyDockWidget(attr_dock_, mtk_dock_);
            mtk_dock_->show();
            mtk_dock_->raise();
        }
        bar("TimeSlider", false);
        bar("RangeSlider", false);
        set_menus("Modeling");
        shelf("Poly Modeling");
    } else if (name == "Modeling - Expert") {
        // The view alone: no shelf, tool box, time slider or panels.
        only(false, false, false, false);
        for (const char* id : {"shelf", "toolbox", "TimeSlider", "RangeSlider"}) bar(id, false);
        set_menus("Modeling");
    } else if (name == "Sculpting" || name == "Pose Sculpting") {
        // The Shape Editor over the Tool Settings on the right.
        only(false, name == "Pose Sculpting", true, true);
        show_named_window("shapeEditor");
        if (QDockWidget* d = dock_editor("shapeEditor", named_windows_.value("shapeEditor"), "Shape Editor", Qt::RightDockWidgetArea)) {
            addDockWidget(Qt::RightDockWidgetArea, tool_dock_);
            splitDockWidget(d, tool_dock_, Qt::Vertical);
            if (name == "Pose Sculpting") {
                splitDockWidget(tool_dock_, attr_dock_, Qt::Horizontal);
                tabifyDockWidget(attr_dock_, channel_dock_);
            } else {
                tabifyDockWidget(tool_dock_, attr_dock_);
            }
            tool_dock_->raise();
        }
        bar("TimeSlider", false);
        bar("RangeSlider", false);
        set_menus("Modeling");
        shelf("Sculpting");
    } else if (name == "UV Editing") {
        // The UV Editor beside the view, as wide.
        only(false, false, false, false);
        show_uv_editor();
        if (QDockWidget* d = dock_editor("uvEditor", uv_window_, "UV Editor", Qt::RightDockWidgetArea)) resizeDocks({d}, {width() / 2}, Qt::Horizontal);
        bar("TimeSlider", false);
        bar("RangeSlider", false);
        set_menus("Modeling");
        shelf("UV Editing");
    } else if (name == "XJEN" || name == "XJEN - Interactive Groom") {
        // The groom editor on the right, tabbed with the Channel Box, Attribute Editor and Outliner.
        only(true, true, true, false);
        show_named_window("interactiveGroomEditor");
        if (QDockWidget* d = dock_editor("interactiveGroomEditor", named_windows_.value("interactiveGroomEditor"), "XJEN", Qt::RightDockWidgetArea)) {
            addDockWidget(Qt::RightDockWidgetArea, outliner_dock_);
            tabifyDockWidget(d, channel_dock_);
            tabifyDockWidget(d, attr_dock_);
            tabifyDockWidget(d, outliner_dock_);
            d->raise();
        }
        shelf("Hair");
    } else if (name == "Rigging" || name == "Animation") {
        // The Outliner left, the Channel Box right, and the Node Editor (rigging) or Graph Editor (animation) under the view.
        only(true, true, false, false);
        if (name == "Rigging") {
            show_node_editor();
            dock_editor("nodeEditor", node_window_, "Node Editor", Qt::BottomDockWidgetArea);
        } else {
            show_graph_editor();
            dock_editor("graphEditor", graph_window_, "Graph Editor", Qt::BottomDockWidgetArea);
        }
        outliner_dock_->raise();
        channel_dock_->raise();
        set_menus(name);
        shelf(name);
    } else if (name.startsWith("Rendering")) {
        // The Outliner left, the Script Editor under the view, the Attribute Editor right; Expert puts the Render View
        // beside the view too.
        only(true, false, true, false);
        show_script_editor();
        dock_editor("scriptEditor", script_window_, "Script Editor", Qt::BottomDockWidgetArea);
        attr_dock_->raise();
        if (name == "Rendering - Expert") {
            show_render_view(this);
            QWidget* rv = ws_docks_.contains("renderView") ? ws_docks_["renderView"]->widget() : nullptr;
            for (QWidget* w : findChildren<QWidget*>(QString(), Qt::FindDirectChildrenOnly))
                if (!rv && w->isWindow() && w->windowTitle().contains("RenderView")) rv = w;
            dock_editor("renderView", rv, "Render View", Qt::LeftDockWidgetArea);
        }
        set_menus("Rendering");
        shelf("Rendering");
    }
}

void MainWindow::toggle_main_pane() {
    if (!main_pane_) {
        main_pane_state_ = saveState();
        for (QToolBar* t : findChildren<QToolBar*>(QString(), Qt::FindDirectChildrenOnly)) t->hide();
        for (QDockWidget* d : findChildren<QDockWidget*>(QString(), Qt::FindDirectChildrenOnly)) d->hide();
        statusBar()->hide();
        // In a multi-view layout the view under the pointer fills the pane too (#189); the way back restores it.
        main_pane_split_ = panels_ && !panels_->single();
        if (main_pane_split_) panels_->toggle();
        main_pane_ = true;
    } else {
        if (main_pane_split_ && panels_ && panels_->single()) panels_->toggle();
        main_pane_split_ = false;
        restoreState(main_pane_state_);
        statusBar()->setVisible(QSettings().value("prefs/helpLine", true).toBool());
        main_pane_ = false;
    }
}

QMenu* MainWindow::windows_menu() {
    auto* m = new QMenu("Windows", this);
    m->addAction("USD Layer Editor", [this] { show_usd_layer_editor(this); });
    m->addAction("USD Stats", [this] { show_usd_stats(this); });
    QMenu* ws = m->addMenu("Workspaces");
    for (const QString& name : workspace_names()) ws->addAction(name, [this, name] { apply_workspace(name); });
    ws->addSeparator();
    ws->addAction("Reset Current Workspace", [this] {
        const QString n = QSettings().value("workspaces_current", "General").toString();
        if (workspace_names().contains(n)) QSettings().remove("workspaces/" + n);
        apply_workspace(n);
    });
    ws->addAction("Save Current Workspace As...", [this] {
        bool ok = false;
        QString n = QInputDialog::getText(this, "Save Workspace", "Name:", QLineEdit::Normal, "myWorkspace", &ok);
        if (ok && !n.isEmpty()) QSettings().setValue("workspaces/" + n, saveState());
    });
    QMenu* mine = ws->addMenu("Saved");
    connect(mine, &QMenu::aboutToShow, this, [this, mine] {
        mine->clear();
        QSettings s;
        s.beginGroup("workspaces");
        for (const QString& k : s.childKeys()) mine->addAction(k, [this, k] { apply_workspace(k); });
        if (mine->isEmpty()) mine->addAction("(none)")->setEnabled(false);
    });
    m->addSeparator();
    QMenu* ge = m->addMenu("General Editors");
    auto named = [this](QMenu* menu, const QString& label, const QString& id) { menu->addAction(label, [this, id] { show_named_window(id); }); };
    named(ge, "Attribute Spread Sheet", "spreadSheet");
    named(ge, "Component Editor", "componentEditor");
    named(ge, "Connection Editor", "connectionEditor");
    named(ge, "Hypergraph: Hierarchy", "hypergraphHierarchy");
    named(ge, "Hypergraph: Connections", "hypergraphConnections");
    named(ge, "Namespace Editor", "namespaceEditor");
    named(ge, "Channel Control", "channelControl");
    ge->addSeparator();
    ge->addAction("Script Editor", this, &MainWindow::show_script_editor);
    ge->addAction("Attribute Editor\tCtrl+A", this, &MainWindow::show_attribute_editor);
    ge->addAction("Channel Box / Layer Editor", [this] { channel_dock_->show(); channel_dock_->raise(); });
    QMenu* model_ed = m->addMenu("Modeling Editors");
    model_ed->addAction("UV Set Editor", [this] { show_named_window("uvSetEditor"); });
    model_ed->addAction("UV Editor\tF12", this, &MainWindow::show_uv_editor);
    named(model_ed, "Modeling Toolkit", "modelingToolkit");
    QMenu* anim_ed = m->addMenu("Animation Editors");
    anim_ed->addAction("Graph Editor", this, &MainWindow::show_graph_editor);
    anim_ed->addAction("Dope Sheet", this, &MainWindow::show_dope_sheet);
    named(anim_ed, "Shape Editor", "shapeEditor");
    if (has_command_named("expression")) named(anim_ed, "Expression Editor", "expressionEditor");
    else todo(anim_ed, "Expression Editor");
    QMenu* render_ed = m->addMenu("Rendering Editors");
    render_ed->addAction(icon("renderView", 16), "Render View", [this] { show_render_view(this); });
    render_ed->addAction(icon("renderSettings", 16), "Render Settings...", [this] { show_render_settings(this); });
    render_ed->addAction("Hypershade", this, &MainWindow::show_hypershade);
    QMenu* rig_ed = m->addMenu("Rigging Editors");
    rig_ed->addAction("Node Editor", this, &MainWindow::show_node_editor);
    rig_ed->addAction("Set Driven Key", this, &MainWindow::show_set_driven_key);
    rig_ed->addAction("Paint Skin Weights", [this] { set_tool("paintSkin"); show_tool_settings(); });
    QMenu* rel = m->addMenu("Relationship Editors");
    for (auto [label, kind] : {std::pair{"Character Sets", "characters"}, {"Deformer Sets", "deformerSets"}, {"Display Layers", "displayLayers"},
                               {"Sets", "sets"}, {"Shading Groups", "shadingGroups"}}) {
        QString k = kind;
        rel->addAction(label, [this, k] { show_relationship_editor(this, k); });
    }
    QMenu* ll = rel->addMenu("Light Linking");
    ll->addAction("Light Centric", [this] { show_relationship_editor(this, "lightCentric"); });
    ll->addAction("Object Centric", [this] { show_relationship_editor(this, "objectCentric"); });
    m->addAction("Outliner", [this] { outliner_dock_->show(); outliner_dock_->raise(); });
    m->addSeparator();
    QMenu* layouts = m->addMenu("View Arrangement");
    struct L { const char* label; int kind; };
    for (L l : {L{"Single Pane", 0}, L{"Two Panes Side by Side", 1}, L{"Two Panes Stacked", 2}, L{"Three Panes", 3}, L{"Four Panes", 4}}) {
        int k = l.kind;
        layouts->addAction(l.label, [this, k] { panels_->set_layout(k); });
    }
    m->addAction("Main Pane Only\tCtrl+Space", [this] { toggle_main_pane(); });
    m->addSeparator();
    m->addAction("Tool Settings...\tCtrl+Shift+T", this, &MainWindow::show_tool_settings);
    QMenu* prefs = m->addMenu("Settings/Preferences");
    prefs->addAction("Preferences...", this, &MainWindow::show_preferences);
    prefs->addAction("Hotkey Editor...", [this] { show_hotkey_editor(); });
    prefs->addAction("Shelf Editor...", [this] { shelf_bar_->open_editor(); });
    prefs->addAction("Plug-in Manager...", [this] { show_plugin_manager(); });
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Small actions

void MainWindow::toggle_view_flag(const QString& key) {
    ViewportWidget* v = active_view();
    if (!v) return;
    bool on = parse(v->get("disp")).toObject()[key].toBool();
    v->set("disp", QString("{\"%1\":%2}").arg(key, on ? "false" : "true"));
}

void MainWindow::go_to_time(double t) {
    // Playing on, the next tick would put the time back where the clock says it should be.
    if (play_timer_ && play_timer_->isActive()) stop_play();
    time_slider_->set_time(t);
}

void MainWindow::step_time(int by) {
    go_to_time(time_slider_->time() + by);
}

void MainWindow::step_key(const QString& which) {
    QJsonValue f = ask("findKeyframe", QJsonObject{{"which", which}});
    if (f.isDouble()) go_to_time(f.toDouble());
}

void MainWindow::scale_manip(double by) {
    QSettings s;
    double m = qBound(0.25, s.value("prefs/manipSize", 1.0).toDouble() + by, 4.0);
    s.setValue("prefs/manipSize", m);
    apply_prefs();
}

void MainWindow::nudge(int dx, int dy) {
    // A pixel in the view's plane, as the reference's Alt+arrow.
    ViewportWidget* v = active_view();
    if (!v) return;
    double upp = parse(v->get("unitsPerPixel")).toDouble(0.01);
    QJsonObject ax = parse(v->get("viewAxes")).toObject();
    QJsonArray r = ax["right"].toArray(), u = ax["up"].toArray();
    QJsonArray by;
    for (int i = 0; i < 3; ++i) by.append((r[i].toDouble() * dx + u[i].toDouble() * dy) * upp);
    bool comps = state()["mode"].toString() != "object";
    exec(comps ? "moveComponents" : "move", QJsonObject{{"by", by}});
}

void MainWindow::show_history_manips() {
    // The node's handles in the view, and its numbers in the in-view editor.
    set_tool("showManip");
    help_line_->setText("Show Manipulator Tool: drag a handle to change the node's attribute.");
}

void MainWindow::add_to_shelf(QAction* a) {
    QString label = a->text().section('\t', 0, 0);
    QString code = a->property("melCommand").toString();
    if (code.isEmpty()) {
        // Items without a command of their own run by their place in the menus.
        QStringList path{label};
        for (QObject* o = a->parent(); o; o = o->parent())
            if (auto* m = qobject_cast<QMenu*>(o)) path.prepend(m->title());
        shelf_bar_->add_to_current(label, QString(), path.join("/"), "menu");
    } else {
        shelf_bar_->add_to_current(label, a->property("shelfIcon").toString(), code, "mel");
    }
    log(QString("// Added %1 to the shelf //").arg(label));
}

void MainWindow::run_menu_path(const QString& path) {
    QStringList parts = path.split('/');
    QList<QAction*> level;
    for (QMenu* m : set_menus_) level << m->menuAction();
    QAction* found = nullptr;
    for (const QString& part : parts) {
        found = nullptr;
        for (QAction* a : level)
            if (a->text().section('\t', 0, 0) == part) {
                found = a;
                break;
            }
        if (!found) {
            log(QString("// Warning: %1 is not in these menus //").arg(path), 4);
            return;
        }
        if (found->menu()) level = found->menu()->actions();
    }
    if (found) found->trigger();
}

void MainWindow::view_scrub(QObject* o, QEvent* e) {
    auto* me = static_cast<QMouseEvent*>(e);
    Q_UNUSED(o);
    if (e->type() == QEvent::MouseButtonPress && me->button() == Qt::LeftButton) {
        scrubbing_ = true;
        scrub_from_ = me->position().x();
        scrub_time_ = time_slider_->time();
    } else if (e->type() == QEvent::MouseMove && scrubbing_) {
        // A frame per four pixels.
        double to = scrub_time_ + std::round((me->position().x() - scrub_from_) / 4.0);
        if (view_hold_ == "scrub") time_slider_->set_time(to);
    } else if (e->type() == QEvent::MouseButtonRelease && scrubbing_) {
        scrubbing_ = false;
        double by = std::round((me->position().x() - scrub_from_) / 4.0);
        if (view_hold_ == "nudgeKeys" && by != 0) {
            // I: the keys on this frame move with the drag.
            exec("keyframe", QJsonObject{{"edit", true}, {"time", QString("%1").arg(scrub_time_)}, {"relative", true}, {"timeChange", by}});
            time_slider_->set_time(scrub_time_ + by);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Marking menus

void MainWindow::tool_marking_menu(QPoint global, const QString& hold) {
    QList<MarkItem> items;
    QString kind = hold == "rotate" ? "manipRotateContext" : hold == "scale" ? "manipScaleContext" : "manipMoveContext";
    QString ctx = hold == "rotate" ? "Rotate" : hold == "scale" ? "Scale" : "Move";
    auto mode = [kind, ctx](int m) {
        return [kind, ctx, m] { exec_quiet(kind, QJsonObject{{"edit", true}, {"args", QJsonArray{ctx}}, {"mode", m}}); };
    };
    auto flip = [kind, ctx](const char* flag) {
        QString f = flag;
        return [kind, ctx, f] {
            bool on = ask(kind, QJsonObject{{"query", true}, {"queried", QJsonArray{f}}, {"args", QJsonArray{ctx}}}).toBool();
            exec_quiet(kind, QJsonObject{{"edit", true}, {"args", QJsonArray{ctx}}, {f, !on}});
        };
    };
    auto soft = [](bool on) { return [on] { exec("softSelect", QJsonObject{{"softSelectEnabled", on ? 1 : 0}}); }; };
    auto step = [this](const char* key, double val) {
        QString k = key;
        return [this, k, val] {
            for (ViewportWidget* v : panels_->views()) v->set(k, QString::number(val));
        };
    };
    if (hold == "select") {
        items = {{"Select Tool", 0, [this] { set_tool("select"); }}, {"Lasso Tool", 2, [this] { set_tool("lasso"); }}, {"Paint Selection Tool", 6, [this] { set_tool("paintSelect"); }},
                 {"Soft Select", 4, soft(true)}, {"Soft Select Off", -1, soft(false)}, {"Tool Settings...", -1, [this] { show_tool_settings(); }}};
    } else if (hold == "move") {
        items = {{"World", 0, mode(2)}, {"Object", 2, mode(0)}, {"Component", 4, mode(10)}, {"Parent", 6, mode(1)}, {"Normal", 1, mode(3)},
                 {"Soft Select", 3, soft(true)}, {"Soft Select Off", 5, soft(false)}, {"Custom", 7, mode(6)},
                 {"Along Rotation Axis", -1, mode(4)}, {"Along Live Object Axis", -1, mode(5)}, {"Tweak Mode", -1, flip("tweakMode")}, {"Preserve Children", -1, flip("preserveChildPosition")},
                 {"Step Snap Off", -1, step("stepMove", 0)}, {"Step Snap 1", -1, step("stepMove", 1)}, {"Tool Settings...", -1, [this] { show_tool_settings(); }}};
    } else if (hold == "rotate") {
        items = {{"World", 0, mode(1)}, {"Object", 2, mode(0)}, {"Component", 4, mode(10)}, {"Gimbal", 6, mode(2)}, {"Custom", 7, mode(3)},
                 {"Step Snap Off", 3, step("stepRotate", 0)}, {"Step Snap 15", 5, step("stepRotate", 15)}, {"Preserve Children", -1, flip("preserveChildPosition")}, {"Tool Settings...", -1, [this] { show_tool_settings(); }}};
    } else if (hold == "scale") {
        items = {{"World", 0, mode(2)}, {"Object", 2, mode(0)}, {"Component", 4, mode(10)}, {"Normal", 6, mode(3)}, {"Parent", 1, mode(1)}, {"Custom", 7, mode(6)},
                 {"Soft Select", 3, soft(true)}, {"Soft Select Off", 5, soft(false)}, {"Preserve Children", -1, flip("preserveChildPosition")}, {"Tool Settings...", -1, [this] { show_tool_settings(); }}};
    }
    if (!items.isEmpty()) MarkingMenu::show_at(global, items);
}

void MainWindow::selection_marking_menu(QPoint global) {
    auto item = [](const QString& label, int dir, std::function<void()> fn) {
        MarkItem m;
        m.label = label;
        m.dir = dir;
        m.fn = std::move(fn);
        return m;
    };
    auto box = [item](const QString& label, int dir, bool on, std::function<void()> fn) {
        MarkItem m = item(label, dir, std::move(fn));
        m.checkable = true;
        m.checked = on;
        return m;
    };
    auto menu = [](const QString& label, int dir, const QList<MarkItem>& sub) {
        MarkItem m;
        m.label = label;
        m.dir = dir;
        m.sub = sub;
        return m;
    };

    // Symmetry: the status line's setting.
    auto sym = [](const char* flag) { return ask("symmetricModelling", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}}); };
    int sym_at = 0;
    if (sym("symmetry").toBool()) {
        const QString about = sym("about").toString(), axis = sym("axis").toString();
        const int k = axis == "z" ? 2 : axis == "y" ? 1 : 0;
        sym_at = about == "topo" ? 7 : about == "world" ? 4 + k : 1 + k;
    }
    QList<MarkItem> symmetry;
    const char* sym_names[] = {"Off", "Object X", "Object Y", "Object Z", "World X", "World Y", "World Z", "Topology"};
    for (int i = 0; i < 8; ++i) {
        symmetry.append(box(sym_names[i], -1, sym_at == i, [i] {
            static const char* axis[] = {"", "x", "y", "z", "x", "y", "z"};
            if (i == 7) exec("symmetricModelling", QJsonObject{{"edit", true}, {"topoSymmetry", true}});
            else if (i == 0) exec("symmetricModelling", QJsonObject{{"edit", true}, {"symmetry", false}});
            else exec("symmetricModelling", QJsonObject{{"edit", true}, {"symmetry", true}, {"about", i >= 4 ? "world" : "object"}, {"axis", axis[i]}});
            Host::get()->poke();
        }));
    }

    const bool soft_on = ask("softSelect", QJsonObject{{"query", true}, {"queried", QJsonArray{"softSelectEnabled"}}}).toInt() != 0;
    const QList<MarkItem> select = {item("Select Tool", -1, [this] { set_tool("select"); }),
                                    box("Soft Select", -1, soft_on, [soft_on] { exec("softSelect", QJsonObject{{"softSelectEnabled", soft_on ? 0 : 1}}); Host::get()->poke(); })};

    const QList<MarkItem> sel_constraints = {item("Off", -1, [] { exec("polySelectConstraint", QJsonObject{{"disable", true}}); }),
                                             item("Use Constraints...", -1, [this] { show_named_window("polySelectConstraint"); })};

    const QString xc = ask("xformConstraint", QJsonObject{{"query", true}}).toString("none");
    QList<MarkItem> xf_constraints;
    for (auto [label, type] : {std::pair{"Off", "none"}, {"Edge", "edge"}, {"Surface", "surface"}}) {
        const QString t = type;
        xf_constraints.append(box(label, -1, xc == t, [t] { exec_quiet("xformConstraint", QJsonObject{{"type", t}}); Host::get()->poke(); }));
    }

    MarkingMenu::show_at(global, {
        menu("Symmetry", 0, symmetry), item("Lasso", 2, [this] { set_tool("lasso"); }),
        menu("Select", 4, select), item("Paint Select", 6, [this] { set_tool("paintSelect"); }),
        menu("Selection Constraints", -1, sel_constraints), menu("Transform Constraints", -1, xf_constraints),
    });
}

QList<MarkItem> MainWindow::hotbox_zone(int zone, QPoint origin) {
    ModelPanel* panel = panels_->panel_at(origin);
    switch (zone) {
    case 0: {
        auto layout = [this](int kind, const QStringList& types, int top = 50, int left = 50) {
            return [this, kind, types, top, left] {
                panels_->set_editor_layout(kind, types, top, left);
                panels_->active()->setFocus(Qt::OtherFocusReason);
            };
        };
        return {{"Single Perspective", 0, layout(0, {"model"})},
                {"Hypershade/Render/Persp", 1, layout(5, {"hypershade", "model", "render"}, 55)},
                {"Hypershade/Persp", 2, layout(1, {"hypershade", "model"})},
                {"Hypershade/Outliner/Persp", 3, layout(5, {"hypershade", "model", "outliner"}, 55)},
                {"Toggle Outliner", 4, [] { mel("ToggleOutliner;"); }},
                {"Persp/Graph/Outliner", 5, layout(5, {"model", "graph", "outliner"}, 55, 78)},
                {"Persp/Graph", 6, layout(2, {"model", "graph"}, 46)},
                {"Persp/Relationship Editor", 7, layout(2, {"model", "relationship"}, 61)}};
    }
    case 1: {
        auto toggle = [](QWidget* widget) {
            return [widget] { widget->setVisible(!widget->isVisible()); };
        };
        auto tool_bar = [this, toggle](const char* name) { return toggle(findChild<QToolBar*>(name)); };
        // The Channel Box and Layer Editor share a dock, as they do in the reference's combined mode.
        MarkItem attributes{"Attributes", 2};
        attributes.sub = {{"Tool Settings", 0, [] { mel("ToggleToolSettings;"); }},
                          {"Channel Box", 2, [] { mel("ToggleChannelBox;"); }},
                          {"Layer Editor", 4, [] { mel("ToggleChannelBox;"); }},
                          {"Attribute Editor", 6, [this] {
                               if (attr_dock_->isVisible() && !attr_dock_->visibleRegion().isEmpty()) attr_dock_->hide();
                               else show_attribute_editor();
                           }}};
        return {{"Help Line", 0, toggle(statusBar())}, {"Status Line", 1, tool_bar("statusLine")}, attributes,
                {"Tool Box", 3, tool_bar("toolbox")}, {"Time Slider", 4, toggle(time_dock_)},
                {"Range Slider", 5, toggle(range_dock_)},
                {"Shelf", 6, tool_bar("shelf")}, {"Command Line", 7, tool_bar("commandLine")}};
    }
    case 2: {
        auto type = [panel](const char* t) {
            QString k = t;
            return [panel, k] {
                panel->set_panel_type(k);
                panel->setFocus(Qt::OtherFocusReason);
            };
        };
        return {{"Outliner", 0, type("outliner")}, {"Sets", 1, type("sets")}, {"Graph Editor", 2, type("graph")},
                {"Dope Sheet", 3, type("dope")}, {"Hypergraph", 4, type("hypergraph")}, {"Node Editor", 5, type("node")},
                {"Hypershade", 6, type("hypershade")}, {"Render View", 7, type("render")}};
    }
    case 3: {
        auto mask = [this](const QString& name) {
            return [this, name] {
                exec("selectScope", args(QJsonArray{name}));
                sync_object_masks();
            };
        };
        return {{"NURBS Mask", 0, mask("nurbsModeling")}, {"Hierarchy", 1, mask("hierarchy")},
                {"Animation Mask", 2, mask("animation")}, {"Deformations Mask", 3, mask("deformations")},
                {"Dynamics Mask", 4, mask("dynamics")}, {"Polygons Mask", 5, mask("polyModeling")},
                {"Rendering Mask", 6, mask("rendering")}, {"Object/Components", 7, [this] {
                    const bool components = state()["mode"].toString() != "object";
                    scope_buttons_.value(components ? "object" : "component")->trigger();
                }}};
    }
    case 4: {
        auto cam = [panel](const QString& name) {
            return [panel, name] {
                panel->set_panel_type("model");
                panel->view()->set("camera", "\"" + name + "\"");
                panel->view()->setFocus(Qt::OtherFocusReason);
            };
        };
        // The Aztek box: the cameras, as the reference's, Perspective at the top and Top View on the left (#192).
        return {{"Perspective", 0, cam("persp")}, {"Right View", 2, cam("side")}, {"Front View", 4, cam("front")}, {"Top View", 6, cam("top")}};
    }
    default: return {};
    }
}

void MainWindow::marking_menu(QPoint global, int mods) {
    if ((mods & 3) == 3) {
        selection_marking_menu(global);
        return;
    }
    ViewportWidget* v = active_view();
    QString mode = state()["mode"].toString();
    QList<MarkItem> items;
    auto run = [](const QString& c, const QJsonObject& call = QJsonObject()) { return [c, call] { exec(c, call); }; };
    // The reference's contextual menus, item for item and in its order (#183). An item with nothing behind it yet is
    // there, greyed out, so the menu keeps its shape.
    auto act = [](const QString& label, int dir, std::function<void()> fn, std::function<void()> option = {}) {
        MarkItem m;
        m.label = label;
        m.dir = dir;
        m.fn = std::move(fn);
        m.option = std::move(option);
        return m;
    };
    // A command, its option box the command's option window.
    auto com = [act](const QString& label, int dir, const QString& c, const QJsonObject& call = QJsonObject(), bool box = false) {
        return act(label, dir, [c, call] { exec(c, call); }, box ? std::function<void()>([c, label, call] { OptionsDialog::open(c, label, call); }) : std::function<void()>());
    };
    // A tool, its option box the tool's settings.
    auto tool = [this, act](const QString& label, int dir, const QString& id, bool box = true) {
        return act(label, dir, [this, id] { set_tool(id); }, box ? std::function<void()>([this, id] { set_tool(id); show_tool_settings(); }) : std::function<void()>());
    };
    auto melc = [act](const QString& label, int dir, const QString& code) { return act(label, dir, [code] { mel(code); }); };
    auto none = [](const QString& label, int dir, bool box = false) {
        MarkItem m;
        m.label = label;
        m.dir = dir;
        m.enabled = false;
        if (box) m.option = [] {};
        return m;
    };
    auto sep = [](MarkItem m) {
        m.separator = true;
        return m;
    };
    auto subm = [](const QString& label, int dir, const QList<MarkItem>& items) {
        MarkItem m;
        m.label = label;
        m.dir = dir;
        m.sub = items;
        return m;
    };
    // The Move Component command for the mode: its handles straight away.
    auto transform_component = [act, mode]() {
        const QString c = mode == "face" ? "polyMoveFacet" : mode == "edge" ? "polyMoveEdge" : "polyMoveVertex";
        return act("Transform Component", -1, [c] { exec(c); }, [c] { OptionsDialog::open(c, "Transform Component"); });
    };
    static QHash<QString, bool> shown;
    auto flip = [](const QString& key, const QString& flag, QJsonObject flags) {
        return [key, flag, flags]() mutable {
            flags[flag] = shown[key] = !shown.value(key);
            exec("polyOptions", flags);
        };
    };
    auto polygon_display = [subm, flip](bool global = false) {
        const auto options = [global](QJsonObject flags) {
            if (global) flags["global"] = true;
            exec("polyOptions", flags);
        };
        auto relative = [options](const char* flag) {
            return [options, flag] { options({{"relative", true}, {flag, true}}); };
        };
        return subm(global ? "Polygon Display All" : "Polygon Display", -1, {
            {"Toggle Backface Culling", -1, flip("bc", "backCulling", {})},
            {"Toggle Border Edges", -1, relative("displayBorder"), true, false, false, true},
            {"Toggle Texture Border Edges", -1, relative("displayMapBorder")},
            {"Toggle Face Normals", -1, flip("fn", "displayNormal", {{"facet", true}}), true, false, false, true},
            {"Toggle Vertex Normals", -1, flip("vn", "displayNormal", {{"point", true}})},
            {"Toggle Face Centers", -1, relative("displayCenter"), true, false, false, true},
            {"Toggle Hidden Triangles", -1, relative("displayTriangle")},
            {"Toggle Vertices", -1, relative("displayVertex")},
            {"Reset Polygon Display", -1, [options] {
                shown.clear();
                options({{"backCulling", false}, {"displayBorder", false}, {"displayMapBorder", false},
                         {"displayCenter", false}, {"displayTriangle", false}, {"displayVertex", false}});
                options({{"displayNormal", false}, {"facet", true}});
                options({{"displayNormal", false}, {"point", true}});
             }, true, false, false, true},
        });
    };
    auto mapping = [subm, com]() {
        return subm("Mapping", -1, {com("Automatic", -1, "polyAutoProjection", {}, true), com("Contour Stretch", -1, "polyContourProjection", {}, true),
                                    com("Cylindrical", -1, "polyCylindricalProjection", {}, true), com("Planar", -1, "polyPlanarProjection", {}, true),
                                    com("Spherical", -1, "polySphericalProjection", {}, true)});
    };
    auto soften_harden = [subm, com, act, flip]() {
        return subm("Soften/Harden Edges", 3, {com("Soften Edge", 1, "polySoftEdge", {{"angle", 180.0}}), com("Soften/Harden", 2, "polySoftEdge", {}, true),
                                              com("Harden Edge", 3, "polySoftEdge", {{"angle", 0.0}}), act("Toggle Soft Edge Display", 4, flip("se", "softEdge", {}))});
    };
    if (mods & 2) {
        // Ctrl: converting the selection, as the reference's (#183).
        auto to = [com](const QString& label, const QString& kind) { return com(label, -1, "convertSelection", args(QJsonArray{kind})); };
        items = {subm("To Edges", 0, {to("To Edges", "edge"), com("To Edge Loop", -1, "selectLoop"), com("To Edge Ring", -1, "selectLoop", {{"ring", true}})}),
                 subm("Grow Selection", 1, {com("Grow Selection", -1, "growSelection"), com("Grow Along Loop", -1, "growSelection", {{"alongLoop", true}})}),
                 subm("To UVs", 2, {to("To UVs", "uv"), com("To UV Shell", -1, "selectMode", args(QJsonArray{"uvShell"}))}),
                 subm("Edge Loop Utilities", 3, {com("To Edge Loop", -1, "selectLoop")}),
                 subm("To Faces", 4, {to("To Faces", "face")}),
                 subm("Edge Ring Utilities", 5, {com("To Edge Ring", -1, "selectLoop", {{"ring", true}})}),
                 subm("To Vertices", 6, {to("To Vertices", "vertex")}),
                 subm("Shrink Selection", 7, {com("Shrink Selection", -1, "shrinkSelection"), com("Shrink Along Loop", -1, "shrinkSelection", {{"alongLoop", true}})}),
                 none("To Face Path", -1),
                 com("To UV Shell", -1, "selectMode", args(QJsonArray{"uvShell"})),
                 act("To Shell", -1, [this] { select_shell(false); }),
                 act("To Shell Border", -1, [this] { select_shell(true); }),
                 sep(none("Select Border Edge Tool", -1)),
                 sep(com("Selection Boundary", -1, "selectBorder")),
                 act("Selection Constraints...", -1, [this] { show_named_window("polySelectConstraint"); })};
    } else if ((mods & 1) && selection_names().isEmpty()) {
        // Shift with nothing selected: the polygon primitives of the Create menu, made as its items make them
        // (Interactive Creation and the option boxes' saved settings apply).
        auto with_icon = [](MarkItem item, const char* glyph) {
            item.icon = icon(glyph, 16);
            return item;
        };
        auto prim = [this, act, with_icon](const char* label, int dir, const char* command, const char* glyph, const char* draw_tool = nullptr) {
            const QString c = command, t = QString::fromLatin1(draw_tool);
            return with_icon(act(label, dir, [this, c, t] {
                if (!t.isEmpty() && QSettings().value("prefs/interactiveCreation", false).toBool())
                    set_tool(t);
                else
                    exec(c, OptionsDialog::saved(c));
            }, [c, label] { OptionsDialog::open(c, label); }), glyph);
        };
        const bool interactive = QSettings().value("prefs/interactiveCreation", false).toBool();
        MarkingMenu::show_at(global, {
            with_icon(tool("Create Polygon Tool", 0, "createPolygon"), "face"),
            prim("Disc", 1, "polyDisc", "disc"),
            prim("Sphere", 2, "polySphere", "sphere", "createSphere"),
            prim("Torus", 3, "polyTorus", "torus", "createTorus"),
            prim("Cube", 4, "polyCube", "cube", "createCube"),
            prim("Cone", 5, "polyCone", "cone", "createCone"),
            prim("Cylinder", 6, "polyCylinder", "cylinder", "createCylinder"),
            prim("Plane", 7, "polyPlane", "plane", "createPlane"),
            prim("Platonic Solid", -1, "polyPlatonicSolid", "mesh"),
            prim("Pyramid", -1, "polyPyramid", "cone"),
            prim("Prism", -1, "polyPrism", "cylinder"),
            prim("Pipe", -1, "polyPipe", "pipe"),
            prim("Helix", -1, "polyHelix", "torus"),
            with_icon(none("Gear", -1, true), "options"),
            prim("Soccer Ball", -1, "polySoccerBall", "sphere"),
            sep(with_icon(none("Super Ellipse", -1, true), "sphere")),
            with_icon(none("Spherical Harmonics", -1, true), "sphere"),
            prim("Ultra Shape", -1, "polySuperShape", "sphere"),
            with_icon(none("Type", -1), "script"),
            with_icon(none("SVG", -1), "script"),
            with_icon(tool("Quad Draw Tool", -1, "quadDraw"), "quadDraw"),
            sep({"Interactive Creation", -1, [interactive] { QSettings().setValue("prefs/interactiveCreation", !interactive); }, true, interactive, true}),
            {"Exit On Completion", -1, {}, false, true, true},
            sep(polygon_display(true)),
        }, nullptr, false, true);
        return;
    } else if (mods & 1) {
        // Shift: the modelling tools for the component type, as the reference's (#183).
        if (mode == "face") {
            items = {com("Merge Faces To Center", 0, "polyMergeCenter"), com("Poke Face", 1, "polyPoke", {}, true), com("Bevel Face", 2, "polyBevel", {}, true),
                     subm("Face Normals", 3, {com("Reverse Propagate", 1, "polyNormal", {{"normalMode", 1}}), com("Reverse Normals", 2, "polyNormal", {{"normalMode", 0}}, true),
                                              com("Conform Normals", 3, "polyNormal", {{"normalMode", 2}}), act("Toggle Face Normal Display", 4, flip("fn", "displayNormal", QJsonObject{{"facet", true}}))}),
                     com("Extrude Face", 4, "polyExtrude", {}, true), com("Wedge Face", 5, "polyWedgeFace", {}, true), tool("Multi-Cut", 6, "multiCut"),
                     tool("Paint Select Faces", 7, "paintSelect", false),
                     none("Smart Extrude", -1), com("Smooth Faces", -1, "polySmooth", {}, true), com("Assign Invisible Faces", -1, "polyHole", {}, true),
                     com("Add Divisions To Faces", -1, "polySubdivideFacet", {}, true), com("Circularize Components", -1, "polyCircularize", {}, true),
                     com("Connect Components", -1, "polyConnectComponents", {}, true), melc("Detach Components", -1, "DetachComponent;"),
                     com("Triangulate Faces", -1, "polyTriangulate"), com("Quadrangulate Faces", -1, "polyQuadrangulate", {}, true),
                     com("Reduce Faces", -1, "polyReduce", {}, true), com("Remesh", -1, "polyRemesh", {}, true), com("Bridge Faces", -1, "polyBridge", {}, true),
                     sep(com("Mirror", -1, "polyMirrorFace", {}, true)), com("Extract Faces", -1, "polyExtract", {}, true),
                     com("Duplicate Face", -1, "polyChipOff", {{"duplicate", true}}, true), transform_component(), none("Connect Tool", -1, true),
                     tool("Target Weld Tool", -1, "targetWeld"),
                     sep(mapping()), sep(polygon_display())};
        } else if (mode == "edge") {
            items = {subm("Merge/Collapse Edges", 0, {com("Merge Edges To Center", 0, "polyMergeCenter"), com("Merge Border Edges", 1, "polyMergeEdge", {}, true),
                                                     com("Collapse Edge", 2, "polyCollapseEdge"), tool("Target Weld Tool", 4, "targetWeld")}),
                     subm("Flip/Spin Edge", 1, {com("Flip Triangle Edge", 0, "polyFlipEdge"), com("Spin Forward", 2, "polySpinEdge", {{"offset", 1}}),
                                               com("Spin Backward", 6, "polySpinEdge", {{"offset", -1}})}),
                     com("Bevel Edge", 2, "polyBevel", {}, true), soften_harden(), com("Extrude Edge", 4, "polyExtrude", {}, true),
                     act("Delete Edge", 5, [] { delete_edge_vertex(); }), tool("Multi-Cut", 6, "multiCut"), tool("Paint Select Edges", 7, "paintSelect", false),
                     tool("Crease Tool", -1, "crease"), tool("Offset Edge Loop Tool", -1, "offsetLoop"), tool("Insert Edge Loop Tool", -1, "insertLoop"),
                     tool("Slide Edge Tool", -1, "slideEdge"), com("Circularize Components", -1, "polyCircularize", {}, true),
                     com("Edit Edge Flow", -1, "polyEditEdgeFlow", {}, true),
                     sep(com("Add Divisions To Edge", -1, "polySubdivideEdge", {}, true)), com("Bridge", -1, "polyBridge", {}, true), com("Fill Hole", -1, "polyFillHole"),
                     sep(com("Connect Components", -1, "polyConnectComponents", {}, true)), melc("Detach Components", -1, "DetachComponent;"), transform_component(),
                     none("Connect Tool", -1, true),
                     sep(polygon_display())};
        } else if (mode == "vertex") {
            items = {subm("Merge Vertices", 0, {com("Merge Vertices To Center", 0, "polyMergeCenter"), com("Merge Vertices", 1, "polyMerge", {}, true),
                                               tool("Target Weld Tool", 4, "targetWeld")}),
                     com("Average Vertices", 1, "polyAverageVertex"), com("Chamfer Vertex", 2, "polyChamferVtx", {}, true),
                     subm("Vertex Normals", 3, {com("Average Normals", 1, "polyAverageNormal", {}, true), none("Vertex Normal Edit Tool", 2),
                                               com("Set Normals to Face", 3, "polySetToFaceNormal", {}, true), act("Toggle Vertex Normal Display", 4, flip("vn", "displayNormal", QJsonObject{{"point", true}}))}),
                     com("Extrude Vertex", 4, "polyExtrudeVertex", {}, true), act("Delete Vertex", 5, [] { delete_edge_vertex(); }), tool("Multi-Cut", 6, "multiCut"),
                     tool("Paint Select Vertices", 7, "paintSelect", false),
                     tool("Crease Tool", -1, "crease"), com("Connect Components", -1, "polyConnectComponents", {}, true), melc("Detach Components", -1, "DetachComponent;"),
                     transform_component(), none("Connect Tool", -1, true),
                     sep(com("Circularize Vertices", -1, "polyCircularize", {}, true)), com("Reorder Vertices", -1, "meshReorder"),
                     sep(com("Apply Color", -1, "polyColorPerVertex", {}, true)),
                     sep(polygon_display())};
        } else {
            auto subdiv_proxy = [] {
                // A smoothed copy that follows the original's edits.
                QJsonValue made = exec("polyDuplicateAndConnect");
                QString copy = made.isArray() ? made.toArray().at(0).toString() : made.toString();
                if (!copy.isEmpty()) exec("polySmooth", QJsonObject{{"args", QJsonArray{copy}}, {"divisions", 2}});
            };
            QList<MarkItem> booleans = {com("Union", -1, "polyCBoolOp", {{"operation", 1}}), com("Difference (A - B)", -1, "polyCBoolOp", {{"operation", 2}}),
                                        com("Difference (B - A)", -1, "polyCBoolOp", {{"operation", 4}}), com("Intersection", -1, "polyCBoolOp", {{"operation", 3}})};
            items = {tool("Target Weld Tool", 0, "targetWeld"), com("Fill Holes", 1, "polyFillHole"), tool("Append to Polygon Tool", 2, "appendPolygon"),
                     soften_harden(), com("Extrude", 4, "polyExtrude"), tool("Insert Edge Loop Tool", 5, "insertLoop"), tool("Multi-Cut", 6, "multiCut"),
                     tool("Sculpt Tool", 7, "sculpt"),
                     tool("Offset Edge Loop Tool", -1, "offsetLoop"), com("Smooth", -1, "polySmooth", {}, true), com("Unsmooth", -1, "polyUnsmooth", {}, true),
                     act("Subdiv Proxy", -1, subdiv_proxy, [] { OptionsDialog::open("polySmooth", "Subdiv Proxy"); }), tool("Crease Tool", -1, "crease"),
                     sep(com("Project Curve on mesh", -1, "polyProjectCurve", {}, true)), none("Split mesh with projected curve", -1, true),
                     sep(com("Mirror", -1, "polyMirrorFace", {}, true)), mapping(),
                     sep(com("Triangulate", -1, "polyTriangulate")), com("Quadrangulate", -1, "polyQuadrangulate", {}, true), com("Reduce", -1, "polyReduce", {}, true),
                     com("Remesh", -1, "polyRemesh", {}, true), com("Retopologize", -1, "polyRetopo", {}, true),
                     sep(none("Transfer Vertex Order", -1)),
                     sep(com("Separate", -1, "polySeparate")), com("Combine", -1, "polyUnite", {}, true), subm("Booleans", -1, booleans),
                     sep(com("Cleanup...", -1, "polyCleanup", {{"selectOnly", true}, {"nsided", true}, {"nonManifold", true}, {"lamina", true}, {"zeroGeometry", true}})),
                     none("Connect Tool", -1, true), tool("Quad Draw Tool", -1, "quadDraw"),
                     sep(polygon_display())};
        }
    } else {
        QString hit;
        if (v) {
            QPoint local = v->mapFromGlobal(global);
            double dpr = v->devicePixelRatioF();
            hit = take(aztek_view_hit(v->id(), local.x() * dpr, local.y() * dpr));
        }
        // Off every object: the menu of the last selected one, as the reference gives it (#119), so a joint inside a
        // mesh, say, gets its own menu by being selected and right-clicked away from. Components count for their object.
        QString target = hit;
        if (target.isEmpty()) {
            QStringList sel = selection_names();
            if (sel.isEmpty() && mode != "object") {
                const QJsonArray hl = ask("ls", QJsonObject{{"hilite", true}}).toArray();
                if (!hl.isEmpty()) sel << hl.last().toString();
            }
            if (!sel.isEmpty()) {
                const QString obj = sel.last().section('.', 0, 0);
                if (!ask("ls", QJsonObject{{"args", QJsonArray{obj}}, {"dag", true}}).toArray().isEmpty()) target = obj;
            }
        }
        if (!target.isEmpty()) {
            object_marking_items(target, global, items);
        } else {
            items = {{"Select All", -1, [] { mel("SelectAll;"); }},
                     {"Deselect", -1, run("select", {{"clear", true}})},
                     {"Undo", -1, [] { aztek_undo(); Host::get()->poke(); }},
                     {"Redo", -1, [] { aztek_redo(); Host::get()->poke(); }},
                     {"Repeat Last", -1, [this] { repeat_last(); }},
                     {"Frame All", -1, [v] { if (v) v->set("frame", "\"all\""); }},
                     {"Object Mode", 1, [this] { set_mode("object"); }}};
        }
    }
    MarkingMenu::show_at(global, items);
}

void MainWindow::object_marking_items(const QString& hit, QPoint, QList<MarkItem>& items) {
    auto mode_to = [hit](const QString& m) {
        return [hit, m] {
            // Hilite the object under the cursor, then switch.
            bool in = false;
            for (const QJsonValue& s : state()["selection"].toArray())
                if (s.toString().section('.', 0, 0) == hit) in = true;
            if (!in) exec("select", QJsonObject{{"args", QJsonArray{hit}}});
            exec("selectMode", QJsonObject{{"args", QJsonArray{m}}});
        };
    };
    auto run = [](const QString& c, const QJsonObject& call = QJsonObject()) { return [c, call] { exec(c, call); }; };
    QString type = ask("nodeType", args(QJsonArray{hit})).toString();
    QJsonArray shapes = ask("listRelatives", QJsonObject{{"args", QJsonArray{hit}}, {"shapes", true}}).toArray();
    QString shape_type = shapes.isEmpty() ? QString() : ask("nodeType", args(QJsonArray{shapes.at(0)})).toString();
    QList<MarkItem> common{{"Select", -1, [hit] {
                               // In a component mode it goes back to objects, then selects this one, as the reference's.
                               if (state()["mode"].toString() != "object") exec("selectMode", QJsonObject{{"args", QJsonArray{"object"}}});
                               exec("select", args(QJsonArray{hit}));
                           }},
                           {"Select Hierarchy", -1, [hit] {
                                exec("select", QJsonObject{{"args", QJsonArray{hit}}});
                                exec("selectHierarchy");
                            }},
                           {"Attribute Editor...", -1, [this, hit] {
                                exec("select", QJsonObject{{"args", QJsonArray{hit}}});
                                show_attribute_editor();
                            }},
                           {"Delete", -1, run("delete", args(QJsonArray{hit}))}};
    // Each history node, as the reference's RMB menu lists inputs.
    QList<MarkItem> history;
    for (const QJsonValue& h : ask("listHistory", args(QJsonArray{hit})).toArray()) {
        QString node = h.toString();
        if (node == hit || shapes.contains(node)) continue;
        if (history.size() < 8) history.append({"Input: " + node, -1, [this, node] {
            exec("select", QJsonObject{{"args", QJsonArray{node}}});
            show_attribute_editor();
        }});
    }
    if (type == "joint") {
        items = {{"Joint", 0, run("select", args(QJsonArray{hit}))}, {"Object Mode", 1, mode_to("object")},
                 {"Select Hierarchy", 2, [hit] { exec("select", QJsonObject{{"args", QJsonArray{hit}}}); exec("selectHierarchy"); }}};
        // A joint that drives a skin: Paint Weights opens the brush on that influence.
        QJsonArray dest = ask("listConnections", QJsonObject{{"args", QJsonArray{hit}}, {"type", "skinCluster"}}).toArray();
        if (!dest.isEmpty())
            items.append({"Paint Weights", 4, [this, hit, dest] {
                QJsonArray geo = ask("skinCluster", QJsonObject{{"args", QJsonArray{dest.at(0)}}, {"query", true}, {"geometry", true}}).toArray();
                if (!geo.isEmpty()) exec("select", args(QJsonArray{geo.at(0)}));
                set_tool("paintSkin");
                show_tool_settings();
                exec_quiet("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"influence", hit}});
            }});
    } else if (shape_type == "nurbsCurve") {
        // Edit Point and Curve Point: Vertex mode with the curve's pick set by selectType, as the reference's.
        auto curve_kind = [hit, mode_to](const char* flag) {
            return [hit, mode_to, flag] {
                mode_to("vertex")();
                exec("selectType", QJsonObject{{flag, true}});
            };
        };
        items = {{"Control Vertex", 0, curve_kind("controlVertex")}, {"Object Mode", 1, mode_to("object")},
                 {"Edit Point", 2, curve_kind("editPoint")}, {"Hull", 6, [hit] {
                      for (const QJsonValue& s : ask("listRelatives", QJsonObject{{"args", QJsonArray{hit}}, {"shapes", true}}).toArray()) exec("setAttr", args(QJsonArray{s.toString() + ".dispHull", true}));
                  }},
                 {"Curve Point", 4, curve_kind("curveParameterPoint")}};
    } else {
        // UV opens to UV and UV Shell, as the reference's.
        QList<MarkItem> uv = {{"UV", 2, mode_to("uv")}, {"UV Shell", 4, mode_to("uvShell")}};
        items = {{"Edge", 0, mode_to("edge")}, {"Object Mode", 1, mode_to("object")}, {"UV", 2, mode_to("uv"), true, false, false, false, uv},
                 {"Multi", 3, mode_to("multi")}, {"Face", 4, mode_to("face")}, {"Vertex Face", 5, mode_to("vertexFace")}, {"Vertex", 6, mode_to("vertex")}};
        // A skinned mesh: the brush, as the reference puts it at the top left.
        QJsonValue skin = ask("findRelatedSkinCluster", args(QJsonArray{hit}));
        if (skin.isString() && !skin.toString().isEmpty())
            items.append({"Paint Skin Weights Tool", 7, [this, hit] {
                exec("select", QJsonObject{{"args", QJsonArray{hit}}});
                set_tool("paintSkin");
                show_tool_settings();
            }});
    }
    items.append(common);
    if (shape_type == "mesh" || shape_type == "nurbsSurface") {
        items.append({"Assign New Material...", -1, [this, hit] { assign_new_material(hit); }});
        items.append({"Assign Existing Material...", -1, [this, hit] {
            QMenu menu;
            assign_existing_material_menu(&menu, hit);
            menu.exec(QCursor::pos());
        }});
        items.append({"Material Attributes...", -1, [this, hit] { material_attributes(hit); }});
    }
    items.append(history);
}

} // namespace az
