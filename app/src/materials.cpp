// Materials from the interface: Assign New Material and Assign Existing Material, the Lighting/Shading menu, and the
// Hypershade window, lists of materials and textures with what can be done to them.
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

const QList<QPair<QString, QString>> kMaterials{{"Lambert", "lambert"}, {"Blinn", "blinn"}, {"Phong", "phong"}, {"Phong E", "phongE"}, {"Anisotropic", "anisotropic"}, {"Surface Shader", "surfaceShader"}, {"Standard Surface", "standardSurface"}};

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
    auto* win = new AttributeEditor(nullptr, node);
    win->setAttribute(Qt::WA_DeleteOnClose);
    win->setWindowFlag(Qt::Window);
    win->setWindowTitle(node);
    win->resize(400, 620);
    win->show();
}

// An image scaled to a swatch.
QPixmap image_swatch(const QString& path, int size) {
    static QHash<QString, QPixmap> cache;
    auto it = cache.find(path);
    if (it != cache.end()) return *it;
    QPixmap out;
    int w = 0, h = 0;
    QByteArray utf = path.toUtf8();
    if (unsigned char* data = aztek_image_load(utf.constData(), &w, &h)) {
        out = QPixmap::fromImage(QImage(data, w, h, QImage::Format_RGBA8888).scaled(size, size, Qt::KeepAspectRatio, Qt::SmoothTransformation));
        aztek_image_free(data, w, h);
    }
    cache.insert(path, out);
    return out;
}

QIcon color_swatch(const QJsonArray& c, int size) {
    auto enc = [](double x) { return qBound(0, int(std::pow(qBound(0.0, x, 1.0), 1.0 / 2.2) * 255.0 + 0.5), 255); };
    QPixmap px(size, size);
    px.fill(c.isEmpty() ? QColor(150, 150, 150) : QColor(enc(c.at(0).toDouble()), enc(c.at(1).toDouble()), enc(c.at(2).toDouble())));
    return QIcon(px);
}

class Hypershade : public Panel {
public:
    explicit Hypershade(QWidget* parent) : Panel(parent) {
        setWindowTitle("Hypershade");
        auto* v = new QVBoxLayout(this);
        v->setContentsMargins(0, 0, 0, 0);
        v->setSpacing(0);
        auto* bar = new QToolBar(this);
        auto* create = new QMenu(this);
        for (const auto& m : kMaterials) {
            QString type = m.second;
            create->addAction(m.first, [type] { new_material(type); });
        }
        create->addSeparator();
        create->addAction("File Texture...", [this] {
            QString path = QFileDialog::getOpenFileName(this, "Choose an image", QString(), "Images (*.png *.jpg *.jpeg *.tga *.bmp *.tif *.tiff);;All Files (*)");
            if (path.isEmpty()) return;
            QString file = exec("shadingNode", QJsonObject{{"args", QJsonArray{"file"}}, {"asTexture", true}}).toString();
            QString place = exec("shadingNode", QJsonObject{{"args", QJsonArray{"place2dTexture"}}, {"asUtility", true}}).toString();
            if (file.isEmpty() || place.isEmpty()) return;
            exec("defaultNavigation", QJsonObject{{"connectToExisting", true}, {"source", place}, {"destination", file}});
            exec("setAttr", QJsonObject{{"args", QJsonArray{file + ".fileTextureName", path}}, {"type", "string"}});
        });
        create->addAction("2D Placement", [] { exec("shadingNode", QJsonObject{{"args", QJsonArray{"place2dTexture"}}, {"asUtility", true}}); });
        QAction* c = bar->addAction("Create");
        c->setMenu(create);
        connect(c, &QAction::triggered, this, [create] { create->exec(QCursor::pos()); });
        bar->addAction("Assign to Selection", [this] {
            QString n = current();
            if (!n.isEmpty()) exec("hyperShade", QJsonObject{{"assign", n}});
        });
        bar->addAction("Select Objects", [this] {
            QString n = current();
            if (!n.isEmpty()) exec("hyperShade", QJsonObject{{"objects", n}});
        });
        bar->addAction("Attributes", [this] {
            QString n = current();
            if (!n.isEmpty()) open_attributes(n);
        });
        bar->addAction("Graph", [this] {
            QString n = current();
            if (n.isEmpty()) return;
            exec("select", one(n));
            mel("NodeEditorWindow;");
        });
        bar->addAction("Rename", [this] {
            QString n = current();
            if (n.isEmpty()) return;
            bool ok = false;
            QString to = QInputDialog::getText(this, "Rename", "Name:", QLineEdit::Normal, n, &ok);
            if (ok && !to.trimmed().isEmpty()) exec("rename", QJsonObject{{"args", QJsonArray{n, to.trimmed()}}});
        });
        bar->addAction("Delete", [this] {
            QString n = current();
            if (!n.isEmpty()) exec("delete", one(n));
        });
        v->addWidget(bar);
        tabs_ = new QTabWidget(this);
        for (auto [title, list] : {std::pair{"Materials", &materials_}, {"Textures", &textures_}, {"Utilities", &utilities_}}) {
            auto* l = new QListWidget(this);
            l->setViewMode(QListView::IconMode);
            l->setResizeMode(QListView::Adjust);
            l->setMovement(QListView::Static);
            l->setIconSize(QSize(64, 64));
            l->setGridSize(QSize(120, 96));
            l->setWordWrap(true);
            connect(l, &QListWidget::itemDoubleClicked, this, [](QListWidgetItem* it) { open_attributes(it->text()); });
            *list = l;
            tabs_->addTab(l, title);
        }
        v->addWidget(tabs_, 1);
        resize(620, 420);
    }

protected:
    void refresh() override {
        QJsonObject all = query("shading").toObject();
        fill(materials_, all["materials"].toArray());
        fill(textures_, all["textures"].toArray());
        fill(utilities_, all["utilities"].toArray());
    }

private:
    QString current() const {
        auto* l = qobject_cast<QListWidget*>(tabs_->currentWidget());
        return l && l->currentItem() ? l->currentItem()->text() : QString();
    }
    static void fill(QListWidget* l, const QJsonArray& items) {
        QString keep = l->currentItem() ? l->currentItem()->text() : QString();
        l->clear();
        for (const QJsonValue& v : items) {
            QJsonObject o = v.toObject();
            QIcon ic = color_swatch(o["color"].toArray(), 64);
            QString image = o["image"].toString();
            if (!image.isEmpty()) {
                QPixmap px = image_swatch(image, 64);
                if (!px.isNull()) ic = QIcon(px);
            }
            auto* it = new QListWidgetItem(ic, o["name"].toString(), l);
            QString tip = o["type"].toString();
            if (o.contains("written")) tip += "\n" + (image.isEmpty() ? "Not found: " + o["written"].toString() : image);
            it->setToolTip(tip);
            if (it->text() == keep) l->setCurrentItem(it);
        }
    }
    QTabWidget* tabs_ = nullptr;
    QListWidget* materials_ = nullptr;
    QListWidget* textures_ = nullptr;
    QListWidget* utilities_ = nullptr;
};

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

void MainWindow::show_hypershade() {
    if (!hypershade_window_) {
        hypershade_window_ = new Hypershade(this);
        hypershade_window_->setWindowFlag(Qt::Window);
    }
    hypershade_window_->show();
    hypershade_window_->raise();
}

} // namespace az
