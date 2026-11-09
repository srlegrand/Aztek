// The Reference Editor and File > Create Reference: everything goes through `file`, `referenceQuery` and `referenceEdit`.

#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFileDialog>
#include <QFileInfo>
#include <QFormLayout>
#include <QHeaderView>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QJsonArray>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMenu>
#include <QMenuBar>
#include <QMessageBox>
#include <QPointer>
#include <QPushButton>
#include <QFile>
#include <QColor>
#include <QSettings>
#include <QTreeWidget>
#include <QVBoxLayout>

#include "host.h"
#include "mainwindow.h"

namespace az {

namespace {

const char* kFilter = "Scenes (*.ma *.mb *.aztek);;ASCII scenes (*.ma);;Binary scenes (*.mb);;Aztek scenes (*.aztek);;All files (*)";

QJsonObject with_file(const QString& path, QJsonObject call) {
    call["args"] = QJsonArray{path};
    return call;
}

class ReferenceEditor : public QDialog {
public:
    explicit ReferenceEditor(MainWindow* main) : QDialog(main, Qt::Window), main_(main) {
        setWindowTitle("Reference Editor");
        resize(820, 420);
        auto* v = new QVBoxLayout(this);
        v->setContentsMargins(4, 0, 4, 4);
        auto* bar = new QMenuBar(this);
        v->setMenuBar(bar);

        QMenu* file = bar->addMenu("File");
        file->addAction("Create Reference...", [this] { main_->create_reference_dialog(); });
        file->addAction("Import Objects from Reference", [this] { run_on_selected("Import", true, [](const QString& rfn) { az::exec("file", QJsonObject{{"importReference", true}, {"referenceNode", rfn}}); }); });
        file->addAction("Save Reference Edits as Text...", [this] { save_edits(); });
        QMenu* ref = bar->addMenu("Reference");
        ref->addAction("Load Reference", [this] { load_selected(false); });
        ref->addAction("Reload Reference", [this] { load_selected(true); });
        ref->addAction("Unload Reference", [this] { unload_selected(); });
        ref->addSeparator();
        ref->addAction("Replace Reference...", [this] { replace_selected(); });
        ref->addAction("Duplicate Reference", [this] { duplicate_selected(); });
        ref->addAction("Remove Reference", [this] { remove_selected(); });
        ref->addSeparator();
        ref->addAction("Select File Contents", [this] { select_contents(); });
        ref->addAction("Change Namespace...", [this] { change_namespace(); });
        ref->addSeparator();
        ref->addAction("List Reference Edits...", [this] { list_edits(); });
        ref->addAction("List Unresolved Edits...", [this] { list_edits(true); });
        ref->addAction("Clean Up Reference...", [this] { clean_up(); });
        ref->addAction("Reapply Failed Edits", [this] { reapply(); });

        tree_ = new QTreeWidget(this);
        tree_->setColumnCount(5);
        tree_->setHeaderLabels({"Loaded", "Namespace", "Reference Node", "Edits", "File"});
        tree_->setSelectionMode(QAbstractItemView::ExtendedSelection);
        tree_->setUniformRowHeights(true);
        tree_->setContextMenuPolicy(Qt::CustomContextMenu);
        tree_->header()->setStretchLastSection(true);
        tree_->setColumnWidth(0, 70);
        tree_->setColumnWidth(1, 130);
        tree_->setColumnWidth(2, 150);
        tree_->setColumnWidth(3, 60);
        connect(tree_, &QTreeWidget::customContextMenuRequested, this, [this, ref](const QPoint& p) { ref->exec(tree_->viewport()->mapToGlobal(p)); });
        connect(tree_, &QTreeWidget::itemChanged, this, [this](QTreeWidgetItem* it, int col) {
            if (col != 0 || refreshing_) return;
            QString rfn = it->data(0, Qt::UserRole).toString();
            if (it->checkState(0) == Qt::Checked) az::exec("file", QJsonObject{{"loadReference", rfn}});
            else az::exec("file", QJsonObject{{"unloadReference", rfn}});
            Host::get()->poke();
        });
        connect(tree_, &QTreeWidget::itemSelectionChanged, this, [this] { show_paths(); });
        v->addWidget(tree_, 1);

        auto* form = new QFormLayout;
        unresolved_ = new QLineEdit(this);
        resolved_ = new QLabel(this);
        resolved_->setTextInteractionFlags(Qt::TextSelectableByMouse);
        form->addRow("Unresolved name", unresolved_);
        form->addRow("Resolved name", resolved_);
        v->addLayout(form);
        connect(unresolved_, &QLineEdit::returnPressed, this, [this] {
            QString rfn = current();
            if (rfn.isEmpty() || unresolved_->text().isEmpty()) return;
            az::exec("file", with_file(unresolved_->text(), QJsonObject{{"loadReference", rfn}}));
            Host::get()->poke();
        });

        auto* row = new QHBoxLayout;
        auto add = [&](const QString& label, std::function<void()> fn) {
            auto* b = new QPushButton(label, this);
            connect(b, &QPushButton::clicked, this, fn);
            row->addWidget(b);
        };
        add("Create Reference...", [this] { main_->create_reference_dialog(); });
        add("Reload", [this] { load_selected(true); });
        add("Unload", [this] { unload_selected(); });
        add("Replace...", [this] { replace_selected(); });
        add("Remove", [this] { remove_selected(); });
        add("List Edits...", [this] { list_edits(); });
        row->addStretch(1);
        v->addLayout(row);

        connect(Host::get(), &Host::sceneChanged, this, [this] { if (isVisible()) refresh(); });
        refresh();
    }

    void refresh() {
        QJsonArray list = query("references").toArray();
        if (list == last_) return;
        last_ = list;
        QStringList keep = selected();
        refreshing_ = true;
        tree_->clear();
        QHash<int, QTreeWidgetItem*> items;
        for (const auto& v : list) items[v.toObject()["id"].toInt()] = nullptr;
        // Parents come before those below them in the table.
        for (const auto& v : list) {
            QJsonObject e = v.toObject();
            QTreeWidgetItem* up = items.value(e["parent"].toInt(), nullptr);
            auto* it = up ? new QTreeWidgetItem(up) : new QTreeWidgetItem(tree_);
            items[e["id"].toInt()] = it;
            it->setFlags(it->flags() | Qt::ItemIsUserCheckable);
            it->setCheckState(0, e["loaded"].toBool() ? Qt::Checked : Qt::Unchecked);
            it->setData(0, Qt::UserRole, e["rfn"].toString());
            it->setText(1, e["namespace"].toString());
            it->setText(2, e["rfn"].toString());
            int failed = e["failed"].toInt();
            it->setText(3, failed ? QString("%1 (%2 failed)").arg(e["edits"].toInt()).arg(failed) : QString::number(e["edits"].toInt()));
            it->setText(4, e["path"].toString());
            it->setToolTip(4, e["unresolved"].toString());
            if (!e["found"].toBool()) {
                it->setForeground(4, QColor(240, 90, 80));
                it->setToolTip(4, "File not found: " + e["unresolved"].toString());
            }
            it->setSelected(keep.contains(e["rfn"].toString()));
            it->setExpanded(true);
        }
        refreshing_ = false;
        show_paths();
    }

private:
    QStringList selected() const {
        QStringList out;
        for (QTreeWidgetItem* it : tree_->selectedItems()) out << it->data(0, Qt::UserRole).toString();
        return out;
    }

    QString current() const {
        QStringList s = selected();
        return s.isEmpty() ? QString() : s.first();
    }

    QJsonObject entry(const QString& rfn) const {
        for (const auto& v : last_)
            if (v.toObject()["rfn"].toString() == rfn) return v.toObject();
        return {};
    }

    void show_paths() {
        QJsonObject e = entry(current());
        unresolved_->setText(e["unresolved"].toString());
        resolved_->setText(e["path"].toString());
    }

    void run_on_selected(const QString& what, bool ask_first, const std::function<void(const QString&)>& fn) {
        QStringList s = selected();
        if (s.isEmpty()) return;
        if (ask_first && QMessageBox::question(this, what, QString("%1 %2?").arg(what, s.join(", "))) != QMessageBox::Yes) return;
        for (const QString& rfn : s) fn(rfn);
        Host::get()->poke();
    }

    void load_selected(bool) {
        run_on_selected("Load", false, [](const QString& rfn) { az::exec("file", QJsonObject{{"loadReference", rfn}}); });
    }

    void unload_selected() {
        run_on_selected("Unload", false, [](const QString& rfn) { az::exec("file", QJsonObject{{"unloadReference", rfn}}); });
    }

    void remove_selected() {
        run_on_selected("Remove", true, [](const QString& rfn) { az::exec("file", QJsonObject{{"removeReference", true}, {"referenceNode", rfn}}); });
    }

    void replace_selected() {
        QString rfn = current();
        if (rfn.isEmpty()) return;
        QString f = QFileDialog::getOpenFileName(this, "Replace Reference", QFileInfo(entry(rfn)["path"].toString()).path(), kFilter);
        if (f.isEmpty()) return;
        az::exec("file", with_file(f, QJsonObject{{"loadReference", rfn}}));
        Host::get()->poke();
    }

    void duplicate_selected() {
        run_on_selected("Duplicate", false, [this](const QString& rfn) {
            QJsonObject e = entry(rfn);
            az::exec("file", with_file(e["path"].toString(), QJsonObject{{"reference", true}, {"namespace", e["namespace"].toString().section(':', -1)}}));
        });
    }

    void select_contents() {
        QJsonArray names;
        for (const QString& rfn : selected()) {
            const QJsonArray nodes = ask("referenceQuery", QJsonObject{{"args", QJsonArray{rfn}}, {"nodes", true}}).toArray();
            for (const auto& n : nodes) names.append(n);
        }
        if (!names.isEmpty()) az::exec("select", QJsonObject{{"args", names}});
    }

    void change_namespace() {
        QString rfn = current();
        if (rfn.isEmpty()) return;
        bool ok = false;
        QString ns = QInputDialog::getText(this, "Change Namespace", "New namespace:", QLineEdit::Normal, entry(rfn)["namespace"].toString(), &ok);
        if (!ok || ns.isEmpty()) return;
        az::exec("file", QJsonObject{{"edit", true}, {"namespace", ns}, {"referenceNode", rfn}});
        Host::get()->poke();
    }

    void reapply() {
        run_on_selected("Reapply", false, [](const QString& rfn) { az::exec("referenceEdit", QJsonObject{{"args", QJsonArray{rfn}}, {"applyFailedEdits", true}}); });
    }

    void clean_up() {
        QString rfn = current();
        if (rfn.isEmpty()) return;
        QStringList kinds{"All edits", "setAttr", "addAttr", "deleteAttr", "connectAttr", "disconnectAttr", "parent"};
        bool ok = false;
        QString kind = QInputDialog::getItem(this, "Clean Up Reference", "Remove from " + rfn + ":", kinds, 0, false, &ok);
        if (!ok) return;
        bool was_loaded = entry(rfn)["loaded"].toBool();
        // Edits only come off an unloaded reference.
        if (was_loaded) az::exec("file", QJsonObject{{"unloadReference", rfn}});
        QJsonObject call{{"cleanReference", rfn}};
        if (kind != "All edits") call["editCommand"] = kind;
        az::exec("file", call);
        if (was_loaded) az::exec("file", QJsonObject{{"loadReference", rfn}});
        Host::get()->poke();
    }

    void save_edits() {
        QString rfn = current();
        if (rfn.isEmpty()) return;
        QString f = QFileDialog::getSaveFileName(this, "Save Reference Edits as Text", rfn + "_edits.txt", "Text (*.txt)");
        if (f.isEmpty()) return;
        QFile out(f);
        if (!out.open(QIODevice::WriteOnly)) return;
        for (const auto& s : entry(rfn)["strings"].toArray()) out.write((s.toObject()["text"].toString() + "\n").toUtf8());
    }

    /// Lists a reference's edits; those picked can be taken off, which needs it unloaded for a moment, as the reference does.
    void list_edits(bool failed_only = false) {
        QString rfn = current();
        if (rfn.isEmpty()) return;
        auto* d = new QDialog(this, Qt::Window);
        d->setAttribute(Qt::WA_DeleteOnClose);
        d->setWindowTitle((failed_only ? "Unresolved Edits: " : "Reference Edits: ") + rfn);
        d->resize(780, 460);
        auto* v = new QVBoxLayout(d);
        auto* list = new QListWidget(d);
        list->setSelectionMode(QAbstractItemView::ExtendedSelection);
        for (const auto& s : entry(rfn)["strings"].toArray()) {
            QJsonObject e = s.toObject();
            if (failed_only && !e["failed"].toBool()) continue;
            auto* it = new QListWidgetItem(e["text"].toString(), list);
            it->setData(Qt::UserRole, e["kind"].toString());
            if (e["failed"].toBool()) it->setForeground(QColor(240, 90, 80));
        }
        v->addWidget(list, 1);
        auto* note = new QLabel(QString("%1 edits. Red ones failed on the last load and are kept.").arg(list->count()), d);
        v->addWidget(note);
        auto* row = new QHBoxLayout;
        auto* close = new QPushButton("Close", d);
        connect(close, &QPushButton::clicked, d, &QDialog::close);
        row->addStretch(1);
        row->addWidget(close);
        v->addLayout(row);
        d->show();
    }

    MainWindow* main_;
    QTreeWidget* tree_ = nullptr;
    QLineEdit* unresolved_ = nullptr;
    QLabel* resolved_ = nullptr;
    QJsonArray last_;
    bool refreshing_ = false;
};

} // namespace

void MainWindow::show_reference_editor() {
    static QPointer<ReferenceEditor> editor;
    if (!editor) editor = new ReferenceEditor(this);
    editor->refresh();
    editor->show();
    editor->raise();
    editor->activateWindow();
}

void MainWindow::create_reference_dialog(bool options) {
    QSettings s;
    QString f = QFileDialog::getOpenFileName(this, "Create Reference", s.value("reference/folder").toString(), kFilter);
    if (f.isEmpty()) return;
    s.setValue("reference/folder", QFileInfo(f).path());
    QJsonObject call{{"reference", true}};
    QString stem = QFileInfo(f).completeBaseName();
    if (options) {
        QDialog d(this);
        d.setWindowTitle("Create Reference Options");
        auto* form = new QFormLayout(&d);
        auto* ns = new QLineEdit(stem, &d);
        auto* merge = new QCheckBox("Merge into an existing namespace", &d);
        auto* load = new QCheckBox("Load the reference now", &d);
        load->setChecked(true);
        auto* lock = new QCheckBox("Lock its nodes and attributes", &d);
        auto* group = new QCheckBox("Group its top nodes", &d);
        auto* group_name = new QLineEdit(&d);
        auto* locator = new QCheckBox("Use a locator for the group", &d);
        form->addRow("Namespace", ns);
        form->addRow("", merge);
        form->addRow("", load);
        form->addRow("", lock);
        form->addRow("", group);
        form->addRow("Group name", group_name);
        form->addRow("", locator);
        auto* box = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &d);
        connect(box, &QDialogButtonBox::accepted, &d, &QDialog::accept);
        connect(box, &QDialogButtonBox::rejected, &d, &QDialog::reject);
        form->addRow(box);
        if (d.exec() != QDialog::Accepted) return;
        call["namespace"] = ns->text();
        if (merge->isChecked()) call["mergeNamespacesOnClash"] = true;
        if (!load->isChecked()) call["deferReference"] = true;
        if (lock->isChecked()) call["lockReference"] = true;
        if (group->isChecked()) {
            call["groupReference"] = true;
            if (!group_name->text().isEmpty()) call["groupName"] = group_name->text();
            if (locator->isChecked()) call["groupLocator"] = true;
        }
    } else {
        call["namespace"] = stem;
    }
    az::exec("file", with_file(f, call));
    Host::get()->poke();
}

} // namespace az
