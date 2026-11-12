// Materials from the interface: Assign New Material and Assign Existing Material, and the Lighting/Shading menu. The
// Hypershade is in hypershade.cpp.
#include "mainwindow.h"
#include "aztek.h"
#include "host.h"
#include "panels.h"

#include <QCursor>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFileDialog>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QJsonArray>
#include <QLabel>
#include <QListWidget>
#include <QMenu>
#include <QPainter>
#include <QPushButton>
#include <QTabWidget>
#include <QToolBar>
#include <QVBoxLayout>
#include <cmath>

namespace az {

namespace {

const QList<QPair<QString, QString>> kMaterials{{"Standard Surface", "standardSurface"}, {"Lambert", "lambert"}, {"Blinn", "blinn"}, {"Phong", "phong"}, {"Phong E", "phongE"}, {"Anisotropic", "anisotropic"}, {"Surface Shader", "surfaceShader"}};

QJsonObject one(const QString& a) {
    return QJsonObject{{"args", QJsonArray{a}}};
}

// The object under the marking menu is what's meant, unless it is already selected (perhaps by its faces).
void make_current(const QString& hit) {
    if (hit.isEmpty()) return;
    for (const QJsonValue& s : state()["selection"].toArray())
        if (s.toString().section('.', 0, 0) == hit) return;
    exec("select", one(hit));
}

QString new_material(const QString& type) {
    return exec("shadingNode", QJsonObject{{"args", QJsonArray{type}}, {"asShader", true}}).toString();
}

void open_attributes(const QString& node) {
    auto* win = new AttributeEditor(main_window_widget(), node);
    win->setAttribute(Qt::WA_DeleteOnClose);
    win->setWindowFlags(editor_window_flags());
    win->setWindowTitle(node);
    win->resize(400, 620);
    win->show();
}

} // namespace

void MainWindow::assign_new_material(const QString& hit) {
    QDialog d(this);
    d.setWindowTitle("Assign New Material");
    auto* v = new QVBoxLayout(&d);
    auto* list = new QListWidget(&d);
    for (const auto& m : kMaterials) {
        auto* it = new QListWidgetItem(m.first, list);
        it->setData(Qt::UserRole, m.second);
    }
    list->setCurrentRow(0);
    v->addWidget(list);
    auto* box = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &d);
    v->addWidget(box);
    connect(box, &QDialogButtonBox::accepted, &d, &QDialog::accept);
    connect(box, &QDialogButtonBox::rejected, &d, &QDialog::reject);
    connect(list, &QListWidget::itemDoubleClicked, &d, &QDialog::accept);
    if (d.exec() != QDialog::Accepted || !list->currentItem()) return;
    make_current(hit);
    QString mat = new_material(list->currentItem()->data(Qt::UserRole).toString());
    if (mat.isEmpty()) return;
    exec("hyperShade", QJsonObject{{"assign", mat}});
    open_attributes(mat);
}

void MainWindow::assign_existing_material_menu(QMenu* menu, const QString& hit) {
    for (const QJsonValue& v : ask("ls", QJsonObject{{"materials", true}}).toArray()) {
        QString mat = v.toString();
        menu->addAction(mat, [hit, mat] {
            make_current(hit);
            exec("hyperShade", QJsonObject{{"assign", mat}});
        });
    }
}

void MainWindow::material_attributes(const QString& hit) {
    make_current(hit);
    QJsonArray before = state()["selection"].toArray();
    exec_quiet("hyperShade", QJsonObject{{"shaderNetworksSelectMaterialNodes", true}});
    QJsonArray mats = ask("ls", QJsonObject{{"selection", true}}).toArray();
    exec_quiet("select", QJsonObject{{"args", before}});
    if (!mats.isEmpty()) open_attributes(mats.at(0).toString());
}

QMenu* MainWindow::lighting_shading_menu() {
    auto* m = new QMenu("Lighting/Shading", this);
    m->addAction("Material Attributes...", [this] { material_attributes(); });
    m->addSeparator();
    m->addAction("Assign New Material...", [this] { assign_new_material(); });
    QMenu* existing = m->addMenu("Assign Existing Material");
    connect(existing, &QMenu::aboutToShow, this, [this, existing] {
        existing->clear();
        assign_existing_material_menu(existing);
    });
    m->addSeparator();
    m->addAction("Hypershade", this, &MainWindow::show_hypershade);
    return m;
}

} // namespace az
