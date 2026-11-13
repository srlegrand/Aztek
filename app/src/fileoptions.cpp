// The file options windows. The File Type Specific Options come from the formats' own panels (fbxui, usdui).

#include "fileoptions.h"

#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QFormLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QMenuBar>
#include <QPushButton>
#include <QScrollArea>
#include <QSettings>
#include <QVBoxLayout>

#include "fbxui.h"
#include "host.h"
#include "usdui.h"

namespace az {

namespace {

QString key(FileAction a) {
    switch (a) {
    case FileAction::Import: return "fileOptions/import/type";
    case FileAction::ExportAll: return "fileOptions/exportAll/type";
    case FileAction::ExportSelection: return "fileOptions/exportSelection/type";
    }
    return QString();
}

bool loaded(const QString& plugin) {
    return ask("pluginInfo", QJsonObject{{"args", QJsonArray{plugin}}, {"query", true}, {"loaded", true}}).toBool();
}

// The reference's File type names, for the formats Aztek reads or writes; plug-in formats while their plug-in is loaded.
QStringList types(FileAction a) {
    QStringList t;
    if (a == FileAction::Import) {
        t << "mayaAscii" << "mayaBinary";
        if (loaded("fbx")) t << "FBX";
        t << "OBJ" << "USD Import";
        if (loaded("gltf")) t << "glTF";
        t << "PLY" << "STL";
    } else {
        t << "mayaAscii";
        if (loaded("fbx")) t << "FBX export";
        t << "OBJexport" << "USD Export";
        if (loaded("gltf")) t << "glTF export";
        t << "PLY" << "STL";
    }
    return t;
}

class FileOptions : public QDialog {
public:
    FileOptions(QWidget* parent, FileAction action, std::function<void(const QString&)> run) : QDialog(parent), action_(action), run_(std::move(run)) {
        const char* title = action == FileAction::Import ? "Import Options" : action == FileAction::ExportAll ? "Export All Options" : "Export Selection Options";
        const char* verb = action == FileAction::Import ? "Import" : action == FileAction::ExportAll ? "Export All" : "Export Selection";
        setWindowTitle(title);
        resize(520, 720);
        auto* outer = new QVBoxLayout(this);
        auto* bar = new QMenuBar(this);
        QMenu* edit = bar->addMenu("Edit");
        edit->addAction("Save Settings", [this] { save(); });
        edit->addAction("Reset Settings", [this] { reset(); });
        outer->setMenuBar(bar);

        auto* scroll = new QScrollArea(this);
        scroll->setWidgetResizable(true);
        scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        auto* body = new QWidget(scroll);
        auto* v = new QVBoxLayout(body);
        auto* general = new QGroupBox("General Options", body);
        auto* form = new QFormLayout(general);
        type_ = new QComboBox(general);
        type_->addItems(types(action));
        const int at = type_->findText(chosen_file_type(action));
        if (at >= 0) type_->setCurrentIndex(at);
        form->addRow("File type:", type_);
        if (action != FileAction::Import) {
            ext_ = new QCheckBox("Default file extensions", general);
            ext_->setChecked(default_file_extensions());
            form->addRow(ext_);
        }
        v->addWidget(general);
        specific_ = new QGroupBox("File Type Specific Options", body);
        specific_layout_ = new QVBoxLayout(specific_);
        v->addWidget(specific_);
        v->addStretch(1);
        scroll->setWidget(body);
        outer->addWidget(scroll, 1);
        connect(type_, &QComboBox::currentTextChanged, this, [this] { build(); });
        build();

        // The reference's three buttons, of one width.
        auto* buttons = new QHBoxLayout();
        auto* act = new QPushButton(verb, this);
        auto* apply = new QPushButton("Apply", this);
        auto* close = new QPushButton("Close", this);
        for (QPushButton* b : {act, apply, close}) {
            b->setAutoDefault(false);
            buttons->addWidget(b, 1);
        }
        act->setDefault(true);
        connect(act, &QPushButton::clicked, this, [this] {
            save();
            const QString t = type_->currentText();
            accept();
            run_(t);
        });
        connect(apply, &QPushButton::clicked, this, [this] {
            save();
            run_(type_->currentText());
        });
        connect(close, &QPushButton::clicked, this, &QDialog::reject);
        outer->addLayout(buttons);
    }

private:
    void build() {
        if (panel_) {
            specific_layout_->removeWidget(panel_);
            delete panel_;
            panel_ = nullptr;
        }
        save_panel_ = nullptr;
        reset_panel_ = nullptr;
        const QString t = type_->currentText();
        if (t == "FBX" || t == "FBX export") {
            FbxPanel p = action_ == FileAction::Import ? fbx_import_panel(specific_) : fbx_export_panel(specific_);
            panel_ = p.widget;
            reset_panel_ = p.reset;
        } else if (t == "USD Import" || t == "USD Export") {
            OptionsPanel p = action_ == FileAction::Import ? usd_import_panel(specific_) : usd_export_panel(specific_);
            panel_ = p.widget;
            save_panel_ = p.save;
            reset_panel_ = p.reset;
        } else {
            panel_ = new QLabel("This file type has no further options.", specific_);
        }
        specific_layout_->addWidget(panel_);
    }

    void save() {
        QSettings s;
        s.setValue(key(action_), type_->currentText());
        if (ext_) s.setValue("fileOptions/defaultExtensions", ext_->isChecked());
        if (save_panel_) save_panel_();
    }

    void reset() {
        QSettings s;
        s.remove(key(action_));
        s.remove("fileOptions/defaultExtensions");
        if (reset_panel_) reset_panel_();
        const int at = type_->findText(chosen_file_type(action_));
        if (at >= 0) type_->setCurrentIndex(at);
        if (ext_) ext_->setChecked(true);
        build();
    }

    FileAction action_;
    std::function<void(const QString&)> run_;
    QComboBox* type_ = nullptr;
    QCheckBox* ext_ = nullptr;
    QGroupBox* specific_ = nullptr;
    QVBoxLayout* specific_layout_ = nullptr;
    QWidget* panel_ = nullptr;
    std::function<void()> save_panel_, reset_panel_;
};

} // namespace

void file_options_window(QWidget* parent, FileAction action, std::function<void(const QString&)> run) {
    FileOptions d(parent, action, std::move(run));
    d.exec();
}

QString chosen_file_type(FileAction action) {
    return QSettings().value(key(action), "mayaAscii").toString();
}

QString file_type_filter(const QString& t) {
    if (t == "mayaAscii") return "ASCII scenes (*.ma)";
    if (t == "mayaBinary") return "Binary scenes (*.mb)";
    if (t == "FBX" || t == "FBX export") return "FBX (*.fbx)";
    if (t == "OBJ" || t == "OBJexport") return "OBJ (*.obj)";
    if (t == "USD Import" || t == "USD Export") return kUsdFilter;
    if (t == "glTF" || t == "glTF export") return "glTF 2.0 (*.glb *.gltf)";
    if (t == "PLY") return "PLY (*.ply)";
    if (t == "STL") return "STL (*.stl)";
    return QString();
}

QString file_type_extension(const QString& t) {
    if (t == "mayaAscii") return "ma";
    if (t == "mayaBinary") return "mb";
    if (t == "FBX" || t == "FBX export") return "fbx";
    if (t == "OBJ" || t == "OBJexport") return "obj";
    if (t == "USD Import" || t == "USD Export") return "usd";
    if (t == "glTF" || t == "glTF export") return "gltf";
    if (t == "PLY") return "ply";
    if (t == "STL") return "stl";
    return QString();
}

bool default_file_extensions() {
    return QSettings().value("fileOptions/defaultExtensions", true).toBool();
}

} // namespace az
