// The Plug-in Manager (Windows > Settings/Preferences > Plug-in Manager): every plug-in file in the folders of the
// plug-in path, grouped by folder, with Loaded and Auto load boxes and an information button, as the reference's has.
// A plug-in compiled for the reference is listed and marked: it can't load until it is rebuilt with Aztek's kit.

#include <QCheckBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFileDialog>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QJsonArray>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QPointer>
#include <QPushButton>
#include <QTimer>
#include <QToolButton>
#include <QTreeWidget>
#include <QVBoxLayout>

#include "host.h"
#include "mainwindow.h"
#include "shelves.h"

namespace az {

namespace {

enum Column { kName, kLoaded, kAuto, kState, kInfo };

QString list_text(const QJsonValue& v) {
    QStringList out;
    for (const QJsonValue& x : v.toArray()) out << x.toString();
    return out.isEmpty() ? "(none)" : out.join(", ");
}

// A box that shows against the list's dark background when it is empty.
QCheckBox* tick_box(QWidget* parent, bool on, const QString& tip) {
    auto* b = new QCheckBox(parent);
    QPalette pal = b->palette();
    pal.setColor(QPalette::Base, QColor(0x6E, 0x6E, 0x6E));
    b->setPalette(pal);
    b->setChecked(on);
    b->setToolTip(tip);
    return b;
}

void show_info(QWidget* parent, const QJsonObject& p) {
    const QString name = p["name"].toString();
    QString text = QString("<b>%1</b><br>%2").arg(name.toHtmlEscaped(), p["path"].toString().toHtmlEscaped());
    if (p["kind"].toString() == "foreign") {
        text += "<br><br>" + p["note"].toString().toHtmlEscaped();
    } else if (p["loaded"].toBool()) {
        auto about = [&name](const char* flag) { return ask("pluginInfo", QJsonObject{{"args", QJsonArray{name}}, {"query", true}, {flag, true}}); };
        text += QString("<br><br>Vendor: %1<br>Version: %2<br>API version: %3<br><br>Commands: %4<br>Node types: %5")
                    .arg(about("vendor").toString().toHtmlEscaped(), about("version").toString().toHtmlEscaped(), about("apiVersion").toVariant().toString().toHtmlEscaped(),
                         list_text(about("command")).toHtmlEscaped(), list_text(about("dependNode")).toHtmlEscaped());
    } else {
        text += "<br><br>Not loaded. Its vendor, version, commands and node types show once it is.";
    }
    QMessageBox box(QMessageBox::NoIcon, "Plug-in Information", text, QMessageBox::Close, parent);
    box.setTextFormat(Qt::RichText);
    box.exec();
}

} // namespace

void MainWindow::show_plugin_manager() {
    static QPointer<QDialog> open;
    if (open) {
        open->raise();
        open->activateWindow();
        return;
    }
    auto* d = new QDialog(this);
    open = d;
    d->setAttribute(Qt::WA_DeleteOnClose);
    d->setWindowTitle("Plug-in Manager");
    d->setObjectName("pluginManager");
    d->resize(760, 520);
    auto* lay = new QVBoxLayout(d);
    auto* filter = new QLineEdit(d);
    filter->setPlaceholderText("Search plug-ins...");
    filter->setClearButtonEnabled(true);
    lay->addWidget(filter);
    auto* tree = new QTreeWidget(d);
    tree->setObjectName("pluginTree");
    tree->setHeaderLabels({"Plug-in", "Loaded", "Auto load", "", ""});
    tree->setRootIsDecorated(true);
    tree->setUniformRowHeights(true);
    tree->header()->setStretchLastSection(false);
    tree->header()->setSectionResizeMode(kName, QHeaderView::Stretch);
    tree->header()->resizeSection(kLoaded, 70);
    tree->header()->resizeSection(kAuto, 80);
    tree->header()->resizeSection(kState, 170);
    tree->header()->resizeSection(kInfo, 34);
    lay->addWidget(tree, 1);
    auto* hint = new QLabel(d);
    hint->setWordWrap(true);
    lay->addWidget(hint);

    auto* fill = new std::function<void()>;
    QObject::connect(d, &QObject::destroyed, [fill] { delete fill; });
    *fill = [tree, filter, hint, d, fill] {
        const QString want = filter->text().trimmed();
        tree->clear();
        const QJsonArray rows = ask("aztekPlugins").toArray();
        QHash<QString, QTreeWidgetItem*> folders;
        int foreign = 0;
        for (const QJsonValue& v : rows) {
            const QJsonObject p = v.toObject();
            const QString file = p["file"].toString(), folder = p["folder"].toString(), kind = p["kind"].toString(), path = p["path"].toString();
            if (!want.isEmpty() && !file.contains(want, Qt::CaseInsensitive)) continue;
            QTreeWidgetItem* group = folders.value(folder);
            if (!group) {
                group = new QTreeWidgetItem(tree, {folder});
                group->setFirstColumnSpanned(true);
                QFont f = group->font(0);
                f.setBold(true);
                group->setFont(0, f);
                group->setExpanded(true);
                folders.insert(folder, group);
            }
            auto* row = new QTreeWidgetItem(group, {file});
            row->setToolTip(kName, path);
            if (kind == "foreign") {
                // Listed so it is clear why it isn't there to load, with what to do about it.
                ++foreign;
                const QString note = p["note"].toString();
                row->setText(kState, "Compiled for Maya");
                for (int c = 0; c < tree->columnCount(); ++c) {
                    row->setToolTip(c, note);
                    row->setForeground(c, QColor(0xE0, 0x9A, 0x3A));
                }
                row->setData(kName, Qt::UserRole, "foreign");
            } else {
                auto* loaded = tick_box(tree, p["loaded"].toBool(), "Load or unload " + file);
                tree->setItemWidget(row, kLoaded, loaded);
                QObject::connect(loaded, &QCheckBox::clicked, d, [path, file, fill, d](bool on) {
                    const QJsonValue r = on ? exec("loadPlugin", QJsonObject{{"args", QJsonArray{path}}}) : exec("unloadPlugin", QJsonObject{{"args", QJsonArray{QFileInfo(file).completeBaseName()}}});
                    if (r.isUndefined()) {
                        // The reason is in the Script Editor's history; an unload refused over nodes in use can be forced.
                        if (!on && QMessageBox::question(d, "Unload Plug-in", file + " could not be unloaded: the reason is in the Script Editor. If the scene still uses its nodes, they stop working when it goes.\n\nUnload it anyway?") == QMessageBox::Yes)
                            exec("unloadPlugin", QJsonObject{{"args", QJsonArray{QFileInfo(file).completeBaseName()}}, {"force", true}});
                    }
                    // Later: filling again deletes this box, which is still sending its click.
                    QTimer::singleShot(0, d, [fill] { (*fill)(); });
                });
                auto* autoload = tick_box(tree, p["autoload"].toBool(), "Load " + file + " each time Aztek starts");
                tree->setItemWidget(row, kAuto, autoload);
                QObject::connect(autoload, &QCheckBox::clicked, d, [path](bool on) {
                    exec_quiet("pluginInfo", QJsonObject{{"args", QJsonArray{path}}, {"edit", true}, {"autoload", on}});
                });
                row->setText(kState, kind == "python" ? "Python" : "");
            }
            auto* info = new QToolButton(tree);
            info->setText("i");
            info->setToolTip("Information");
            tree->setItemWidget(row, kInfo, info);
            QObject::connect(info, &QToolButton::clicked, d, [d, p] {
                // Asked again: it may have been loaded since the list was made.
                QJsonObject now = p;
                now["loaded"] = ask("pluginInfo", QJsonObject{{"args", QJsonArray{p["name"].toString()}}, {"query", true}, {"loaded", true}}).toBool();
                show_info(d, now);
            });
        }
        if (rows.isEmpty()) {
            hint->setText("No plug-ins were found. They are looked for in the user folder's plug-ins folders (Preferences > User Folder), in the folders of AZTEK_PLUG_IN_PATH and MAYA_PLUG_IN_PATH, and in modules. Browse loads one from anywhere.");
        } else if (foreign > 0) {
            hint->setText(QString("%1 plug-in%2 compiled for Maya can't load here: a compiled plug-in has to be rebuilt from its source with Aztek's plug-in kit. Python plug-ins load as they are.").arg(foreign).arg(foreign == 1 ? "" : "s"));
        } else {
            hint->clear();
        }
        hint->setVisible(!hint->text().isEmpty());
    };
    (*fill)();
    QObject::connect(filter, &QLineEdit::textChanged, d, [fill] { (*fill)(); });

    auto* bb = new QDialogButtonBox(d);
    auto* browse = bb->addButton("Browse...", QDialogButtonBox::ActionRole);
    auto* refresh = bb->addButton("Refresh", QDialogButtonBox::ActionRole);
    bb->addButton(QDialogButtonBox::Close);
    lay->addWidget(bb);
    QObject::connect(browse, &QPushButton::clicked, d, [d, fill] {
        const QString f = QFileDialog::getOpenFileName(d, "Load Plug-in", QString(), "Plug-ins (*.so *.dll *.mll *.bundle *.py);;All files (*)");
        if (f.isEmpty()) return;
        if (exec("loadPlugin", QJsonObject{{"args", QJsonArray{f}}}).isUndefined())
            QMessageBox::warning(d, "Load Plug-in", QFileInfo(f).fileName() + " did not load. The reason is in the Script Editor.");
        (*fill)();
    });
    QObject::connect(refresh, &QPushButton::clicked, d, [fill] { (*fill)(); });
    QObject::connect(bb, &QDialogButtonBox::rejected, d, &QDialog::close);
    d->show();
}

int MainWindow::import_shelves(const QStringList& files) {
    int added = 0;
    for (const QString& file : files) {
        QString name = QFileInfo(file).baseName();
        if (name.startsWith("shelf_")) name = name.mid(6);
        bool there = false;
        for (const QJsonValue& s : shelf_bar_->data()) there |= s["name"].toString() == name;
        if (there) continue;
        const int before = shelf_bar_->data().size();
        shelf_bar_->import_mel(file);
        added += shelf_bar_->data().size() > before;
    }
    return added;
}

} // namespace az
