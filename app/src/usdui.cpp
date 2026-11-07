// USD in the interface. Everything goes through commands: mayaUSDImport, mayaUSDExport, usdPrim, mayaUsdLayerEditor,
// mayaUsdEditTarget and usdLayer.

#include "usdui.h"

#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFileDialog>
#include <QFileInfo>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QInputDialog>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QLocale>
#include <QMessageBox>
#include <QPointer>
#include <QPushButton>
#include <QSettings>
#include <QSpinBox>
#include <QToolBar>
#include <QTreeWidget>
#include <QVBoxLayout>

#include "host.h"

namespace az {

const char* const kUsdFilter = "USD (*.usd *.usda *.usdc *.usdz)";

bool is_usd_file(const QString& path) {
    const QString s = QFileInfo(path).suffix().toLower();
    return s == "usd" || s == "usda" || s == "usdc" || s == "usdz";
}

namespace {

QJsonObject stored(const char* key) {
    QSettings s;
    return QJsonDocument::fromJson(s.value(key).toByteArray()).object();
}

void store(const char* key, const QJsonObject& o) {
    QSettings s;
    s.setValue(key, QJsonDocument(o).toJson(QJsonDocument::Compact));
}

const char* const kImportKey = "usd/importOptions";
const char* const kExportKey = "usd/exportOptions";

// A dialog of switches and fields over a stored call. Apply stores it, Close leaves it.
class Options : public QDialog {
public:
    Options(QWidget* parent, const QString& title, const char* key) : QDialog(parent), key_(key), call_(stored(key)) {
        setWindowTitle(title);
        form_ = new QFormLayout();
        auto* v = new QVBoxLayout(this);
        v->addLayout(form_);
        auto* b = new QDialogButtonBox(QDialogButtonBox::Save | QDialogButtonBox::Close, this);
        connect(b, &QDialogButtonBox::accepted, this, [this] {
            store(key_, call_);
            accept();
        });
        connect(b, &QDialogButtonBox::rejected, this, &QDialog::reject);
        v->addWidget(b);
    }
    void check(const QString& label, const QString& flag, bool fallback) {
        auto* c = new QCheckBox(this);
        c->setChecked(call_.contains(flag) ? call_[flag].toBool() : fallback);
        connect(c, &QCheckBox::toggled, this, [this, flag](bool on) { call_[flag] = on; });
        form_->addRow(label, c);
    }
    void choice(const QString& label, const QString& flag, const QStringList& items) {
        auto* c = new QComboBox(this);
        c->addItems(items);
        if (call_.contains(flag)) c->setCurrentText(call_[flag].toString());
        connect(c, &QComboBox::currentTextChanged, this, [this, flag](const QString& t) { call_[flag] = t; });
        form_->addRow(label, c);
    }
    void text(const QString& label, const QString& flag) {
        auto* e = new QLineEdit(call_[flag].toString(), this);
        connect(e, &QLineEdit::textChanged, this, [this, flag](const QString& t) {
            if (t.isEmpty()) call_.remove(flag);
            else call_[flag] = t;
        });
        form_->addRow(label, e);
    }
    // First and last frame, with a switch: no switch, no animation written.
    void range(const QString& label) {
        auto* on = new QCheckBox(this);
        auto* a = new QSpinBox(this);
        auto* b = new QSpinBox(this);
        a->setRange(-100000, 100000);
        b->setRange(-100000, 100000);
        QJsonArray r = call_["frameRange"].toArray();
        on->setChecked(r.size() == 2);
        a->setValue(r.size() == 2 ? r[0].toInt() : 1);
        b->setValue(r.size() == 2 ? r[1].toInt() : 120);
        auto apply = [this, on, a, b] {
            if (on->isChecked()) call_["frameRange"] = QJsonArray{a->value(), b->value()};
            else call_.remove("frameRange");
        };
        connect(on, &QCheckBox::toggled, this, apply);
        connect(a, QOverload<int>::of(&QSpinBox::valueChanged), this, apply);
        connect(b, QOverload<int>::of(&QSpinBox::valueChanged), this, apply);
        auto* row = new QHBoxLayout();
        row->addWidget(on);
        row->addWidget(a);
        row->addWidget(b);
        row->addStretch(1);
        form_->addRow(label, row);
    }

private:
    const char* key_;
    QJsonObject call_;
    QFormLayout* form_;
};

} // namespace

void usd_import(const QString& path) {
    QJsonObject call = stored(kImportKey);
    call["file"] = path;
    if (!call.contains("readAnimData")) call["readAnimData"] = true;
    az::exec("mayaUSDImport", call);
}

void usd_export(const QString& path, bool selection) {
    QJsonObject call = stored(kExportKey);
    call["file"] = path;
    if (selection) call["selection"] = true;
    az::exec("mayaUSDExport", call);
}

void usd_import_options(QWidget* parent) {
    Options d(parent, "USD Import Options", kImportKey);
    d.text("Prim Path", "primPath");
    d.check("Read Animation", "readAnimData", true);
    d.range("Frame Range");
    d.check("Instances", "importInstances", true);
    d.choice("Preferred Material", "preferredMaterial", {"", "lambert", "blinn", "phong", "standardSurface"});
    d.check("Change Up Axis", "upAxis", true);
    d.check("Change Units", "unit", true);
    d.choice("Axis and Unit Method", "axisAndUnitMethod", {"rotateScale", "overwritePrefs"});
    d.exec();
}

void usd_export_options(QWidget* parent) {
    Options d(parent, "USD Export Options", kExportKey);
    d.choice("Format for .usd", "defaultUSDFormat", {"usdc", "usda"});
    d.range("Frame Range");
    d.check("UV Sets", "exportUVs", true);
    d.check("Colour Sets", "exportColorSets", true);
    d.check("Display Colour", "exportDisplayColor", false);
    d.check("Visibility", "exportVisibility", true);
    d.check("Instances", "exportInstances", true);
    d.check("Materials", "exportMaterials", true);
    d.choice("Skeletons", "exportSkels", {"none", "auto", "explicit"});
    d.choice("Skin", "exportSkin", {"none", "auto", "explicit"});
    d.check("Blend Shapes", "exportBlendShapes", false);
    d.check("Merge Transform and Shape", "mergeTransformAndShape", true);
    d.check("Strip Namespaces", "stripNamespaces", false);
    d.check("Euler Filter", "eulerFilter", false);
    d.check("World Space Roots", "worldspace", false);
    d.check("Proxy Shapes as References", "exportStagesAsRefs", true);
    d.choice("Subdivision Scheme", "defaultMeshScheme", {"catmullClark", "loop", "bilinear", "none"});
    d.text("Root Prim", "rootPrim");
    d.text("Materials Scope", "materialsScopeName");
    d.exec();
}

// ---------------------------------------------------------------------------------------------------------------------
// The Layer Editor

namespace {

class LayerEditor : public QDialog {
public:
    explicit LayerEditor(QWidget* parent) : QDialog(parent, Qt::Window) {
        setWindowTitle("USD Layer Editor");
        resize(460, 520);
        auto* v = new QVBoxLayout(this);
        v->setContentsMargins(4, 4, 4, 4);
        stages_ = new QComboBox(this);
        v->addWidget(stages_);
        auto* bar = new QToolBar(this);
        bar->addAction("New Layer", [this] { new_layer(); });
        bar->addAction("Add Sublayer", [this] { add_existing(); });
        bar->addAction("Save All", [this] {
            az::exec("usdLayer", QJsonObject{{"proxy", stage()}, {"saveDirty", true}});
        });
        v->addWidget(bar);
        tree_ = new QTreeWidget(this);
        tree_->setColumnCount(4);
        tree_->setHeaderLabels({"Layer", "Target", "Mute", "Lock"});
        tree_->header()->setStretchLastSection(false);
        tree_->header()->setSectionResizeMode(0, QHeaderView::Stretch);
        tree_->setContextMenuPolicy(Qt::CustomContextMenu);
        v->addWidget(tree_, 1);
        hint_ = new QLabel(this);
        hint_->setWordWrap(true);
        v->addWidget(hint_);
        connect(tree_, &QWidget::customContextMenuRequested, this, [this](const QPoint& p) { menu(p); });
        connect(tree_, &QTreeWidget::itemDoubleClicked, this, [this](QTreeWidgetItem* it, int) { set_target(id_of(it)); });
        connect(tree_, &QTreeWidget::itemChanged, this, [this](QTreeWidgetItem* it, int col) { toggled(it, col); });
        connect(stages_, &QComboBox::currentTextChanged, this, [this] { refresh(true); });
        connect(Host::get(), &Host::sceneChanged, this, [this] {
            if (isVisible()) refresh(false);
        });
        refresh(true);
    }

    void showEvent(QShowEvent* e) override {
        QDialog::showEvent(e);
        refresh(true);
    }

private:
    QString stage() const { return stages_->currentText(); }
    static QString id_of(QTreeWidgetItem* it) { return it ? it->data(0, Qt::UserRole).toString() : QString(); }

    void refresh(bool force) {
        QJsonArray list = ask("ls", QJsonObject{{"type", "mayaUsdProxyShapeBase"}, {"long", true}}).toArray();
        QStringList names;
        for (const QJsonValue& n : list) names << n.toString();
        QStringList had;
        for (int i = 0; i < stages_->count(); ++i) had << stages_->itemText(i);
        if (had != names) {
            QString keep = stages_->currentText();
            stages_->blockSignals(true);
            stages_->clear();
            stages_->addItems(names);
            if (names.contains(keep)) stages_->setCurrentText(keep);
            stages_->blockSignals(false);
            force = true;
        }
        if (stage().isEmpty()) {
            tree_->clear();
            hint_->setText("No stage. Create > USD makes one.");
            last_.clear();
            return;
        }
        QJsonValue t = ask("usdLayer", QJsonObject{{"proxy", stage()}, {"query", true}});
        QByteArray now = QJsonDocument(t.toArray()).toJson(QJsonDocument::Compact);
        if (!force && now == last_) return;
        last_ = now;
        updating_ = true;
        tree_->clear();
        for (const QJsonValue& n : t.toArray()) add(nullptr, n.toObject());
        tree_->expandAll();
        updating_ = false;
        hint_->setText("Double click a layer to send edits to it. An asterisk marks unsaved edits.");
    }

    void add(QTreeWidgetItem* parent, const QJsonObject& n) {
        auto* it = parent ? new QTreeWidgetItem(parent) : new QTreeWidgetItem(tree_);
        const QString id = n["identifier"].toString();
        const bool session = n["role"].toString() == "session";
        const bool root = n["role"].toString() == "root";
        QString label = n["name"].toString();
        if (n["dirty"].toBool()) label += " *";
        if (n["missing"].toBool()) label += " (missing)";
        it->setText(0, label);
        it->setToolTip(0, n["path"].toString().isEmpty() ? id : n["path"].toString());
        it->setData(0, Qt::UserRole, id);
        it->setData(0, Qt::UserRole + 1, n["role"].toString());
        it->setData(0, Qt::UserRole + 2, n["anonymous"].toBool());
        if (n["editTarget"].toBool()) {
            QFont f = it->font(0);
            f.setBold(true);
            it->setFont(0, f);
            it->setText(1, "target");
            it->setForeground(0, QColor(120, 220, 160));
        }
        if (!root) it->setCheckState(2, n["muted"].toBool() ? Qt::Checked : Qt::Unchecked);
        if (!session) it->setCheckState(3, n["locked"].toBool() ? Qt::Checked : Qt::Unchecked);
        for (const QJsonValue& k : n["sublayers"].toArray()) add(it, k.toObject());
    }

    void set_target(const QString& id) {
        if (!id.isEmpty()) az::exec("mayaUsdEditTarget", QJsonObject{{"args", QJsonArray{stage()}}, {"edit", true}, {"editTarget", id}});
    }

    void toggled(QTreeWidgetItem* it, int col) {
        if (updating_) return;
        const QString id = id_of(it);
        if (col == 2) az::exec("mayaUsdLayerEditor", QJsonObject{{"args", QJsonArray{id}}, {"muteLayer", QJsonArray{it->checkState(2) == Qt::Checked, stage()}}});
        if (col == 3) az::exec("mayaUsdLayerEditor", QJsonObject{{"args", QJsonArray{id}}, {"lockLayer", QJsonArray{it->checkState(3) == Qt::Checked ? 1 : 0, stage()}}});
    }

    QString root_or_selected() {
        auto* it = tree_->currentItem();
        if (it && it->data(0, Qt::UserRole + 1).toString() != "session") return id_of(it);
        return ask("AL_usdmaya_LayerGetLayers", QJsonObject{{"proxy", stage()}, {"rootLayer", true}}).toString();
    }

    void new_layer() {
        bool ok = false;
        QString name = QInputDialog::getText(this, "New Layer", "Name:", QLineEdit::Normal, "anonymousLayer1", &ok);
        if (ok && !name.isEmpty()) az::exec("mayaUsdLayerEditor", QJsonObject{{"args", QJsonArray{root_or_selected()}}, {"addAnonymous", name}});
    }

    void add_existing() {
        QString f = QFileDialog::getOpenFileName(this, "Add Sublayer", QString(), kUsdFilter);
        if (!f.isEmpty()) az::exec("mayaUsdLayerEditor", QJsonObject{{"args", QJsonArray{root_or_selected()}}, {"insertSubPath", QJsonArray{0, f}}});
    }

    void save_as(const QString& id) {
        QString f = QFileDialog::getSaveFileName(this, "Save Layer As", QString(), kUsdFilter);
        if (!f.isEmpty()) az::exec("usdLayer", QJsonObject{{"args", QJsonArray{id}}, {"proxy", stage()}, {"saveAs", f}});
    }

    void menu(const QPoint& p) {
        auto* it = tree_->itemAt(p);
        if (!it) return;
        const QString id = id_of(it);
        const QString role = it->data(0, Qt::UserRole + 1).toString();
        const bool anon = it->data(0, Qt::UserRole + 2).toBool();
        const QString st = stage();
        QMenu m;
        m.addAction("Set as Edit Target", [this, id] { set_target(id); });
        m.addSeparator();
        if (role != "session") {
            auto* save = m.addAction("Save", [id, st] { az::exec("usdLayer", QJsonObject{{"args", QJsonArray{id}}, {"proxy", st}, {"save", true}}); });
            save->setEnabled(!anon);
        }
        m.addAction("Save As...", [this, id] { save_as(id); });
        m.addAction("Reload", [id, st] { az::exec("usdLayer", QJsonObject{{"args", QJsonArray{id}}, {"proxy", st}, {"reload", true}}); });
        m.addAction("Clear", [id] { az::exec("mayaUsdLayerEditor", QJsonObject{{"args", QJsonArray{id}}, {"clear", true}}); });
        m.addSeparator();
        m.addAction("Add New Sublayer", [this, id] {
            az::exec("mayaUsdLayerEditor", QJsonObject{{"args", QJsonArray{id}}, {"addAnonymous", "anonymousLayer1"}});
        });
        m.addAction("Add Existing Sublayer...", [this, id] {
            QString f = QFileDialog::getOpenFileName(this, "Add Sublayer", QString(), kUsdFilter);
            if (!f.isEmpty()) az::exec("mayaUsdLayerEditor", QJsonObject{{"args", QJsonArray{id}}, {"insertSubPath", QJsonArray{0, f}}});
        });
        if (role == "sublayer" && it->parent()) {
            const QString parent_id = id_of(it->parent());
            const int index = it->parent()->indexOfChild(it);
            m.addAction("Remove Sublayer", [parent_id, index, st] {
                az::exec("mayaUsdLayerEditor", QJsonObject{{"args", QJsonArray{parent_id}}, {"removeSubPath", QJsonArray{index, st}}});
            });
        }
        if (role == "root") {
            m.addSeparator();
            m.addAction("Flatten Into Root", [id] { az::exec("mayaUsdLayerEditor", QJsonObject{{"args", QJsonArray{id}}, {"flatten", true}}); });
            m.addAction("Flatten To File...", [this, st] {
                QString f = QFileDialog::getSaveFileName(this, "Flatten To", QString(), kUsdFilter);
                if (!f.isEmpty()) az::exec("usdLayer", QJsonObject{{"proxy", st}, {"flattenTo", f}});
            });
        }
        m.exec(tree_->viewport()->mapToGlobal(p));
    }

    QComboBox* stages_;
    QTreeWidget* tree_;
    QLabel* hint_;
    QByteArray last_;
    bool updating_ = false;
};

} // namespace

void show_usd_layer_editor(QWidget* parent) {
    static QPointer<LayerEditor> editor;
    if (!editor) editor = new LayerEditor(parent);
    editor->show();
    editor->raise();
    editor->activateWindow();
}

void show_usd_stats(QWidget* parent) {
    auto* d = new QDialog(parent, Qt::Window);
    d->setAttribute(Qt::WA_DeleteOnClose);
    d->setWindowTitle("USD Stats");
    d->resize(520, 560);
    auto* v = new QVBoxLayout(d);
    auto* tree = new QTreeWidget(d);
    tree->setColumnCount(2);
    tree->setHeaderLabels({"", "Value"});
    tree->header()->setSectionResizeMode(0, QHeaderView::Stretch);
    v->addWidget(tree);
    auto row = [](QTreeWidgetItem* up, const QString& a, const QString& b) {
        auto* it = new QTreeWidgetItem(up);
        it->setText(0, a);
        it->setText(1, b);
        it->setTextAlignment(1, Qt::AlignRight);
        return it;
    };
    for (const QJsonValue& st : ask("ls", QJsonObject{{"type", "mayaUsdProxyShapeBase"}, {"long", true}}).toArray()) {
        const QJsonObject o = ask("usdStats", QJsonObject{{"args", QJsonArray{st.toString()}}}).toObject();
        auto* top = new QTreeWidgetItem(tree);
        top->setText(0, st.toString());
        for (const char* k : {"prims", "active", "models", "instances", "prototypes"}) row(top, QString(k), QString::number(o[k].toInteger()));
        auto* types = row(top, "By Type", "");
        const QJsonObject by = o["byType"].toObject();
        for (auto it = by.begin(); it != by.end(); ++it) row(types, it.key(), QString::number(it.value().toInteger()));
        auto* layers = row(top, "Layers", "");
        for (const QJsonValue& l : o["layers"].toArray()) {
            const QJsonObject x = l.toObject();
            QString name = QFileInfo(x["identifier"].toString()).fileName();
            if (x["dirty"].toBool()) name += " *";
            if (x["muted"].toBool()) name += " (muted)";
            row(layers, name, QString("%1 specs, %2").arg(x["specs"].toInteger()).arg(QLocale().formattedDataSize(x["bytes"].toInteger())));
        }
    }
    tree->expandAll();
    d->show();
}

void usd_create_menu(QMenu* m, QWidget* parent) {
    QMenu* usd = m->addMenu("USD");
    usd->addAction("Stage from File...", [parent] {
        QString f = QFileDialog::getOpenFileName(parent, "Create Stage from File", QString(), kUsdFilter);
        if (!f.isEmpty()) az::exec("mayaUsd_createStageFromFile", QJsonObject{{"args", QJsonArray{f}}});
    });
    usd->addAction("Stage with New Layer", [] { az::exec("mayaUsd_createStageWithNewLayer"); });
    usd->addSeparator();
    usd->addAction("USD Layer Editor", [parent] { show_usd_layer_editor(parent); });
    usd->addAction("USD Stats", [parent] { show_usd_stats(parent); });
}

void usd_prim_menu(QMenu* m, const QString& prim, QWidget* parent) {
    const QJsonObject info = ask("usdPrim", QJsonObject{{"args", QJsonArray{prim}}, {"query", true}}).toObject();
    const QString type = info["type"].toString();
    QMenu* usd = m->addMenu(QString("USD: %1").arg(prim.section('/', -1)));
    for (const QJsonValue& vs : info["variantSets"].toArray()) {
        const QJsonObject set = vs.toObject();
        const QString name = set["name"].toString();
        QMenu* sub = usd->addMenu("Variant: " + name);
        for (const QJsonValue& o : set["options"].toArray()) {
            const QString option = o.toString();
            auto* a = sub->addAction(option, [prim, name, option] { az::exec("usdPrim", QJsonObject{{"args", QJsonArray{prim}}, {"setVariant", QJsonArray{name, option}}}); });
            a->setCheckable(true);
            a->setChecked(option == set["selection"].toString());
        }
    }
    if (info["hasPayload"].toBool()) {
        const bool loaded = info["loaded"].toBool();
        usd->addAction(loaded ? "Unload" : "Load", [prim, loaded] { az::exec("usdPrim", QJsonObject{{"args", QJsonArray{prim}}, {loaded ? "unload" : "load", true}}); });
    }
    const bool active = info["active"].toBool();
    usd->addAction(active ? "Deactivate" : "Activate", [prim, active] { az::exec("usdPrim", QJsonObject{{"args", QJsonArray{prim}}, {"active", !active}}); });
    const bool inst = info["instanceable"].toBool();
    usd->addAction(inst ? "Not Instanceable" : "Instanceable", [prim, inst] { az::exec("usdPrim", QJsonObject{{"args", QJsonArray{prim}}, {"instanceable", !inst}}); });
    usd->addSeparator();
    QMenu* add = usd->addMenu("Add New Prim");
    for (const char* t : {"Xform", "Scope", "Mesh", "Cube", "Sphere", "Cylinder", "Cone", "Capsule", "Camera", "Material"}) {
        const QString ty = t;
        add->addAction(ty, [prim, ty, parent] {
            bool ok = false;
            QString n = QInputDialog::getText(parent, "Add New Prim", "Name:", QLineEdit::Normal, ty + "1", &ok);
            if (ok && !n.isEmpty()) az::exec("usdPrim", QJsonObject{{"args", QJsonArray{prim + "/" + n}}, {"define", ty}});
        });
    }
    usd->addAction("Add Reference...", [prim, parent] {
        QString f = QFileDialog::getOpenFileName(parent, "Add Reference", QString(), kUsdFilter);
        if (!f.isEmpty()) az::exec("usdPrim", QJsonObject{{"args", QJsonArray{prim}}, {"addReference", f}});
    });
    usd->addAction("Add Payload...", [prim, parent] {
        QString f = QFileDialog::getOpenFileName(parent, "Add Payload", QString(), kUsdFilter);
        if (!f.isEmpty()) az::exec("usdPrim", QJsonObject{{"args", QJsonArray{prim}}, {"addPayload", f}});
    });
    usd->addAction("Assign Material...", [prim, parent] {
        bool ok = false;
        QString mat = QInputDialog::getText(parent, "Assign Material", "Material prim path:", QLineEdit::Normal, "/", &ok);
        if (ok && !mat.isEmpty()) az::exec("usdPrim", QJsonObject{{"args", QJsonArray{prim}}, {"assignMaterial", mat}});
    });
    usd->addSeparator();
    usd->addAction("Edit As Scene Data", [prim] { az::exec("mayaUsdEditAsMaya", QJsonObject{{"args", QJsonArray{prim}}}); });
    usd->addAction("Duplicate As Scene Data", [prim] { az::exec("mayaUsdDuplicate", QJsonObject{{"args", QJsonArray{prim}}}); });
    usd->addSeparator();
    usd->addAction("Delete Prim", [prim] { az::exec("usdPrim", QJsonObject{{"args", QJsonArray{prim}}, {"remove", true}}); });
}

void usd_node_menu(QMenu* m, const QString& node) {
    const QJsonValue linked = ask("attributeQuery", QJsonObject{{"args", QJsonArray{"USD_pulledPrimPath"}}, {"node", node}, {"exists", true}});
    if (!linked.toBool()) {
        // Anything else can be copied into a stage.
        const QJsonArray stages = ask("ls", QJsonObject{{"type", "mayaUsdProxyShapeBase"}, {"long", true}}).toArray();
        if (stages.isEmpty()) return;
        QMenu* usd = m->addMenu("Duplicate As USD Data");
        for (const QJsonValue& st : stages) {
            const QString name = st.toString();
            usd->addAction(name, [node, name] { az::exec("mayaUsdDuplicate", QJsonObject{{"args", QJsonArray{node, name + ",/"}}}); });
        }
        return;
    }
    m->addAction("Merge to USD", [node] { az::exec("mayaUsdMergeToUsd", QJsonObject{{"args", QJsonArray{node}}}); });
    m->addAction("Discard Edits", [node] { az::exec("mayaUsdDiscardEdits", QJsonObject{{"args", QJsonArray{node}}}); });
}

bool usd_confirm_dirty_layers(QWidget* parent) {
    QStringList dirty;
    QJsonArray stages = ask("ls", QJsonObject{{"type", "mayaUsdProxyShapeBase"}, {"long", true}}).toArray();
    if (stages.isEmpty()) return true;
    std::function<void(const QJsonArray&)> walk = [&](const QJsonArray& a) {
        for (const QJsonValue& v : a) {
            const QJsonObject o = v.toObject();
            if (o["dirty"].toBool() && o["role"].toString() != "session" && !o["anonymous"].toBool()) dirty << o["name"].toString();
            walk(o["sublayers"].toArray());
        }
    };
    for (const QJsonValue& st : stages) walk(ask("usdLayer", QJsonObject{{"proxy", st.toString()}, {"query", true}}).toArray());
    if (dirty.isEmpty()) return true;
    QMessageBox box(QMessageBox::Question, "Unsaved USD Layers", "These layers have unsaved edits:\n\n" + dirty.join("\n"), QMessageBox::NoButton, parent);
    box.setInformativeText("Save them to their files, or keep the edits in the scene file.");
    auto* to_usd = box.addButton("Save to USD", QMessageBox::AcceptRole);
    auto* keep = box.addButton("Keep in Scene", QMessageBox::ActionRole);
    box.addButton("Cancel", QMessageBox::RejectRole);
    box.setDefaultButton(keep);
    box.exec();
    if (box.clickedButton() == to_usd) {
        for (const QJsonValue& st : stages) az::exec("usdLayer", QJsonObject{{"proxy", st.toString()}, {"saveDirty", true}});
        return true;
    }
    return box.clickedButton() == keep;
}

} // namespace az
