// The Hypershade, laid out as the reference's: the Browser along the top with a tab per kind of node, the Create panel
// under it, the work area (a node graph per tab) in the middle, the Material Viewer and the Property Editor on the right.
#include "mainwindow.h"
#include "aztek.h"
#include "host.h"
#include "nodeeditor.h"
#include "panels.h"
#include "rendernodes.h"

#include <QApplication>
#include <QDockWidget>
#include <QFileSystemModel>
#include <QInputDialog>
#include <QJsonArray>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMainWindow>
#include <QMenu>
#include <QMenuBar>
#include <QPainter>
#include <QTabBar>
#include <QTabWidget>
#include <QTimer>
#include <QToolBar>
#include <QToolButton>
#include <QTreeView>
#include <QVBoxLayout>
#include <cmath>

namespace az {

namespace {

QJsonObject one(const QString& a) {
    return QJsonObject{{"args", QJsonArray{a}}};
}

bool surface_shader(const QString& type) {
    for (const QJsonValue& v : ask("listNodeTypes", one("shader/surface")).toArray())
        if (v.toString() == type) return true;
    return false;
}

struct V3 {
    double x = 0, y = 0, z = 0;
    V3 operator+(V3 o) const { return {x + o.x, y + o.y, z + o.z}; }
    V3 operator*(double s) const { return {x * s, y * s, z * s}; }
    V3 operator*(V3 o) const { return {x * o.x, y * o.y, z * o.z}; }
};

V3 colour_of(const QString& plug, V3 fallback) {
    QJsonValue v = ask("getAttr", one(plug));
    QJsonArray a = v.toArray();
    if (a.size() == 1 && a.at(0).isArray()) a = a.at(0).toArray();
    if (a.size() < 3) return fallback;
    return {a.at(0).toDouble(), a.at(1).toDouble(), a.at(2).toDouble()};
}

double number_of(const QString& plug, double fallback) {
    QJsonValue v = ask("getAttr", one(plug));
    return v.isDouble() ? v.toDouble() : fallback;
}

// What a ball needs of a material: its colour, how much of it is mirror-like, and how tight the highlight is.
struct Look {
    V3 base{0.5, 0.5, 0.5}, spec{0, 0, 0}, glow{0, 0, 0};
    double rough = 0.5, metal = 0;
};

Look look_of(const QString& name, const QString& type) {
    Look l;
    auto p = [&name](const char* a) { return name + "." + a; };
    if (type == "standardSurface" || type == "aiStandardSurface") {
        const bool snake = type == "aiStandardSurface";
        l.base = colour_of(p(snake ? "base_color" : "baseColor"), l.base) * number_of(p("base"), 1.0);
        l.spec = colour_of(p(snake ? "specular_color" : "specularColor"), {1, 1, 1}) * number_of(p("specular"), 1.0);
        l.rough = number_of(p(snake ? "specular_roughness" : "specularRoughness"), 0.2);
        l.metal = number_of(p("metalness"), 0.0);
        l.glow = colour_of(p(snake ? "emission_color" : "emissionColor"), {1, 1, 1}) * number_of(p("emission"), 0.0);
    } else if (type == "surfaceShader" || type == "aiFlat") {
        l.base = {0, 0, 0};
        l.glow = colour_of(p(type == "aiFlat" ? "color" : "outColor"), {0.5, 0.5, 0.5});
    } else {
        l.base = colour_of(p("color"), l.base) * number_of(p("diffuse"), 0.8);
        l.glow = colour_of(p("incandescence"), {0, 0, 0});
        if (type == "blinn") {
            l.spec = colour_of(p("specularColor"), {0.5, 0.5, 0.5});
            l.rough = qBound(0.05, number_of(p("eccentricity"), 0.3), 1.0);
        } else if (type == "phong") {
            l.spec = colour_of(p("specularColor"), {0.5, 0.5, 0.5});
            l.rough = qBound(0.05, std::sqrt(2.0 / (number_of(p("cosinePower"), 20.0) + 2.0)), 1.0);
        } else if (type == "phongE" || type == "anisotropic") {
            l.spec = colour_of(p("specularColor"), {0.5, 0.5, 0.5});
            l.rough = qBound(0.05, number_of(p("roughness"), 0.5), 1.0);
        }
    }
    return l;
}

// A lit ball wearing the material: one key light from the upper left, a dim fill, on a dark ground.
QImage ball(const Look& l, int size) {
    QImage img(size, size, QImage::Format_ARGB32);
    img.fill(QColor(20, 20, 20));
    const double r = size * 0.46, c = size * 0.5;
    const V3 key{-0.45, 0.6, 0.66}, fill{0.7, -0.2, 0.3};
    auto enc = [](double x) { return qBound(0, int(std::pow(qBound(0.0, x, 1.0), 1.0 / 2.2) * 255.0 + 0.5), 255); };
    const double shine = 2.0 / qMax(1e-3, std::pow(qMax(0.03, l.rough), 4)) - 2.0;
    for (int y = 0; y < size; ++y) {
        for (int x = 0; x < size; ++x) {
            double cover = 0;
            V3 sum;
            // Four samples a pixel smooth the ball's edge.
            for (int s = 0; s < 4; ++s) {
                const double nx = (x + 0.25 + 0.5 * (s & 1) - c) / r, ny = -(y + 0.25 + 0.5 * (s >> 1) - c) / r;
                const double d2 = nx * nx + ny * ny;
                if (d2 > 1.0) continue;
                const double nz = std::sqrt(1.0 - d2);
                const double kd = qMax(0.0, nx * key.x + ny * key.y + nz * key.z), fd = qMax(0.0, nx * fill.x + ny * fill.y + nz * fill.z);
                // Half vector with the eye straight on.
                V3 h{key.x, key.y, key.z + 1.0};
                const double hl = std::sqrt(h.x * h.x + h.y * h.y + h.z * h.z);
                const double nh = qMax(0.0, (nx * h.x + ny * h.y + nz * h.z) / hl);
                const double sp = std::pow(nh, qBound(1.0, shine, 4000.0)) * qMin(8.0, (shine + 2.0) / 8.0) * kd;
                const V3 diffuse = l.base * (1.0 - l.metal) * (kd * 0.95 + fd * 0.2 + 0.06);
                const V3 spec_colour = l.spec * (1.0 - l.metal) * 0.25 + l.base * l.metal;
                // A metal has no diffuse: the room it would mirror stands in as a soft band of light.
                const V3 mirror = l.base * l.metal * (0.15 + 0.5 * qMax(0.0, ny * 0.6 + nz * 0.5));
                sum = sum + diffuse + spec_colour * sp + mirror + l.glow;
                cover += 1.0;
            }
            if (cover == 0) continue;
            const double a = cover / 4.0;
            const V3 px = sum * (1.0 / cover);
            auto mix = [a](int fg, int bg) { return int(fg * a + bg * (1.0 - a)); };
            img.setPixel(x, y, qRgb(mix(enc(px.x), 20), mix(enc(px.y), 20), mix(enc(px.z), 20)));
        }
    }
    return img;
}

QPixmap image_tile(const QString& path, int size) {
    static QHash<QString, QPixmap> cache;
    const QString key = path + "@" + QString::number(size);
    auto it = cache.find(key);
    if (it != cache.end()) return *it;
    QPixmap out;
    int w = 0, h = 0;
    QByteArray utf = path.toUtf8();
    if (unsigned char* data = aztek_image_load(utf.constData(), &w, &h)) {
        out = QPixmap::fromImage(QImage(data, w, h, QImage::Format_RGBA8888).scaled(size, size, Qt::KeepAspectRatio, Qt::SmoothTransformation));
        aztek_image_free(data, w, h);
    }
    cache.insert(key, out);
    return out;
}

// A node as Harold renders it: a material on a ball, a texture flat. Kept until the node or what feeds it changes.
QImage harold_swatch(const QString& name, int size) {
    struct Kept { QString signature; QImage image; };
    static QHash<QString, Kept> cache;
    const QString key = name + "@" + QString::number(size);
    const QString sig = ask("haroldSwatch", QJsonObject{{"args", QJsonArray{name}}, {"signature", true}}).toString();
    if (sig.isEmpty()) return {};
    auto it = cache.find(key);
    if (it != cache.end() && it->signature == sig) return it->image;
    const QJsonObject made = ask("haroldSwatch", QJsonObject{{"args", QJsonArray{name}}, {"size", size}}).toObject();
    QImage img(made["file"].toString());
    if (img.isNull()) return {};
    img = img.convertToFormat(QImage::Format_ARGB32);
    if (cache.size() > 400) cache.clear();
    cache.insert(key, Kept{sig, img});
    return img;
}

// The Material Viewer: the selected material on a ball, redrawn when it changes.
class MaterialViewer : public QWidget {
public:
    explicit MaterialViewer(QWidget* parent) : QWidget(parent) { setMinimumSize(160, 160); }
    void show_material(const QString& name, const QString& type) {
        name_ = name;
        type_ = type;
        update();
    }

protected:
    void paintEvent(QPaintEvent*) override {
        QPainter p(this);
        p.fillRect(rect(), QColor(20, 20, 20));
        if (name_.isEmpty()) {
            p.setPen(QColor(150, 150, 150));
            p.drawText(rect(), Qt::AlignCenter, "Select a material to view it");
            return;
        }
        const int side = qMin(width(), height());
        const qreal dpr = devicePixelRatioF();
        // Harold's picture of it; the drawn ball stands in when there is none.
        QImage img = harold_swatch(name_, qMin(512, int(side * dpr)));
        if (img.isNull()) img = ball(look_of(name_, type_), int(side * dpr));
        else img = img.scaled(int(side * dpr), int(side * dpr), Qt::KeepAspectRatio, Qt::SmoothTransformation);
        img.setDevicePixelRatio(dpr);
        p.drawImage(QPointF((width() - side) / 2.0, (height() - side) / 2.0), img);
    }

private:
    QString name_, type_;
};

class Hypershade : public QMainWindow {
public:
    explicit Hypershade(QWidget* parent) : QMainWindow(parent) {
        setWindowTitle("Hypershade");
        setDockNestingEnabled(true);
        setCorner(Qt::BottomLeftCorner, Qt::LeftDockWidgetArea);
        setCorner(Qt::TopRightCorner, Qt::RightDockWidgetArea);
        setCorner(Qt::BottomRightCorner, Qt::RightDockWidgetArea);

        // The work area: a graph a tab.
        work_ = new QTabWidget(this);
        work_->setTabsClosable(false);
        work_->setDocumentMode(true);
        auto* centre = new QWidget(this);
        auto* cv = new QVBoxLayout(centre);
        cv->setContentsMargins(0, 0, 0, 0);
        cv->setSpacing(0);
        auto* bar = new QToolBar(centre);
        bar->setIconSize(QSize(16, 16));
        bar->addAction("Input Connections", [this] { graph(4); })->setToolTip("Graph what feeds the selected nodes");
        bar->addAction("Input and Output Connections", [this] { graph(1); })->setToolTip("Graph what the selected nodes are wired to, both ways");
        bar->addAction("Add Selected", [this] {
            if (NodeEditor* ed = editor()) ed->add_selected();
        });
        bar->addAction("Clear Graph", [this] {
            if (NodeEditor* ed = editor()) ed->clear_graph();
        });
        bar->addAction("Frame All", [this] { frame(); });
        cv->addWidget(bar);
        cv->addWidget(work_, 1);
        setCentralWidget(centre);
        auto* plus = new QToolButton(work_);
        plus->setText("+");
        plus->setAutoRaise(true);
        plus->setToolTip("Create a new tab");
        work_->setCornerWidget(plus, Qt::TopRightCorner);
        connect(plus, &QToolButton::clicked, this, [this] { new_tab(); });
        new_tab();

        // The Browser.
        auto* browser = new QWidget(this);
        auto* bv = new QVBoxLayout(browser);
        bv->setContentsMargins(0, 0, 0, 0);
        bv->setSpacing(0);
        auto* bbar = new QToolBar(browser);
        search_ = new QLineEdit(bbar);
        search_->setPlaceholderText("Search...");
        search_->setClearButtonEnabled(true);
        search_->setMaximumWidth(220);
        bbar->addWidget(search_);
        connect(search_, &QLineEdit::textChanged, this, [this] { refresh(); });
        bv->addWidget(bbar);
        tabs_ = new QTabWidget(browser);
        tabs_->setDocumentMode(true);
        for (const char* t : {"Materials", "Textures", "Utilities", "Rendering", "Lights", "Cameras", "Shading Groups", "Bake Sets"}) add_list(t);
        // Projects: the project's folders and images.
        files_ = new QTreeView(tabs_);
        auto* model = new QFileSystemModel(files_);
        model->setReadOnly(true);
        files_->setModel(model);
        for (int c = 1; c < 4; ++c) files_->hideColumn(c);
        files_->setHeaderHidden(true);
        tabs_->addTab(files_, "Projects");
        add_list("Asset Nodes");
        bv->addWidget(tabs_, 1);
        browser_dock_ = dock("Browser", browser, Qt::TopDockWidgetArea);

        // Create.
        auto* create = new CreateBar(this);
        connect(create, &CreateBar::picked, this, [this](const RenderNodeKind& k) { make(k); });
        create_dock_ = dock("Create", create, Qt::LeftDockWidgetArea);

        // Material Viewer and Property Editor.
        viewer_ = new MaterialViewer(this);
        viewer_dock_ = dock("Material Viewer", viewer_, Qt::RightDockWidgetArea);
        property_dock_ = dock("Property Editor", new AttributeEditor(this), Qt::RightDockWidgetArea);
        splitDockWidget(viewer_dock_, property_dock_, Qt::Vertical);

        build_menus();
        resize(1280, 800);
        resizeDocks({browser_dock_}, {190}, Qt::Vertical);
        resizeDocks({create_dock_, viewer_dock_}, {300, 330}, Qt::Horizontal);
        resizeDocks({viewer_dock_, property_dock_}, {260, 420}, Qt::Vertical);

        timer_ = new QTimer(this);
        timer_->setSingleShot(true);
        timer_->setInterval(60);
        connect(timer_, &QTimer::timeout, this, [this] { refresh(); });
        connect(Host::get(), &Host::sceneChanged, this, [this] {
            if (isVisible()) timer_->start();
        });
    }

protected:
    void showEvent(QShowEvent* e) override {
        QMainWindow::showEvent(e);
        refresh();
    }

private:
    QDockWidget* dock(const QString& title, QWidget* w, Qt::DockWidgetArea area) {
        auto* d = new QDockWidget(title, this);
        d->setObjectName(title);
        d->setWidget(w);
        addDockWidget(area, d);
        return d;
    }

    void add_list(const QString& title) {
        auto* l = new QListWidget(tabs_);
        l->setViewMode(QListView::IconMode);
        l->setResizeMode(QListView::Adjust);
        l->setMovement(QListView::Static);
        l->setIconSize(QSize(64, 64));
        l->setGridSize(QSize(92, 92));
        l->setWordWrap(true);
        l->setSelectionMode(QAbstractItemView::ExtendedSelection);
        l->setContextMenuPolicy(Qt::CustomContextMenu);
        l->setObjectName("hypershadeSwatches");
        // A swatch picked here is the node selected, which the Property Editor and the Material Viewer then show. With Ctrl
        // or Shift held the picks of the other tabs stay, as the reference keeps a selection across the Browser's tabs.
        // Sets are picked as themselves, not as their members.
        connect(l, &QListWidget::itemSelectionChanged, this, [this, l] {
            if (filling_) return;
            const bool keep = QApplication::keyboardModifiers() & (Qt::ControlModifier | Qt::ShiftModifier);
            if (!keep && !l->selectedItems().isEmpty()) {
                filling_ = true;
                for (QListWidget* other : lists_)
                    if (other != l) other->clearSelection();
                filling_ = false;
            }
            QJsonArray names;
            for (QListWidget* other : lists_) {
                if (other != l && !keep) continue;
                for (QListWidgetItem* it : other->selectedItems())
                    if (!names.contains(it->text())) names.append(it->text());
            }
            if (!names.isEmpty()) exec("select", QJsonObject{{"args", names}, {"noExpand", true}});
        });
        connect(l, &QListWidget::itemDoubleClicked, this, [this](QListWidgetItem* it) { graph_node(it->text()); });
        connect(l, &QListWidget::customContextMenuRequested, this, [this, l, title](const QPoint& at) {
            QListWidgetItem* it = l->itemAt(at);
            if (!it) return;
            const QString n = it->text();
            QMenu m;
            m.addAction("Graph Network", [this, n] { graph_node(n); });
            if (title == "Materials") {
                m.addSeparator();
                m.addAction("Assign Material To Selection", [n] { exec("hyperShade", QJsonObject{{"assign", n}}); });
                m.addAction("Select Objects With Material", [n] { exec("hyperShade", QJsonObject{{"objects", n}}); });
            }
            m.addSeparator();
            m.addAction("Rename", [this, n] {
                bool ok = false;
                QString to = QInputDialog::getText(this, "Rename", "Name:", QLineEdit::Normal, n, &ok);
                if (ok && !to.trimmed().isEmpty()) exec("rename", QJsonObject{{"args", QJsonArray{n, to.trimmed()}}});
            });
            m.addAction("Delete", [n] { exec("delete", one(n)); });
            m.exec(l->mapToGlobal(at));
        });
        lists_[title] = l;
        tabs_->addTab(l, title);
    }

    NodeEditor* editor() const { return qobject_cast<NodeEditor*>(work_->currentWidget()); }

    void new_tab() {
        int n = 1;
        QStringList names;
        for (int i = 0; i < work_->count(); ++i) names << work_->tabText(i);
        while (names.contains(QString("Untitled_%1").arg(n))) ++n;
        work_->setCurrentIndex(work_->addTab(new NodeEditor(work_), QString("Untitled_%1").arg(n)));
    }

    void graph(int depth) {
        if (NodeEditor* ed = editor()) ed->graph_selection(depth);
    }

    // Graph Network from the Browser: any node, a render setting, camera, light or shading group as well as a material.
    // One that nothing is wired to shows all its attributes, so there is something to wire from.
    void graph_node(const QString& n) {
        exec("select", QJsonObject{{"args", QJsonArray{n}}, {"noExpand", true}});
        graph(4);
        if (NodeEditor* ed = editor())
            if (!ed->wired(n)) ed->add_nodes({n}, 3);
    }

    void frame() {
        if (NodeEditor* ed = editor()) {
            QKeyEvent press(QEvent::KeyPress, Qt::Key_A, Qt::NoModifier);
            QApplication::sendEvent(ed, &press);
        }
    }

    // A node from the Create panel or menu: made, put in the graph, selected. A surface material gets its shading group,
    // as the reference gives it one.
    void make(const RenderNodeKind& k) {
        QStringList placement;
        QString node = make_render_node(k, &placement);
        if (node.isEmpty()) return;
        QStringList show{node};
        if (k.flag == "asShader" && surface_shader(k.type)) {
            QString sg = exec("sets", QJsonObject{{"renderable", true}, {"noSurfaceShader", true}, {"empty", true}, {"name", node + "SG"}}).toString();
            if (!sg.isEmpty()) {
                exec("connectAttr", QJsonObject{{"args", QJsonArray{node + ".outColor", sg + ".surfaceShader"}}});
                show << sg;
            }
        }
        // A node made here shows its attributes, so there is something to wire from the start.
        if (NodeEditor* ed = editor()) {
            ed->add_nodes(placement, 2);
            ed->add_nodes(show, 3);
        }
        exec("select", one(node));
    }

    void build_menus() {
        QMenu* edit = menuBar()->addMenu("Edit");
        edit->addAction("Delete", [] {
            QJsonArray sel = state()["selection"].toArray();
            if (!sel.isEmpty()) exec("delete", QJsonObject{{"args", sel}});
        });
        edit->addAction("Duplicate Shading Network", [] { exec("hyperShade", QJsonObject{{"duplicate", true}}); });
        edit->addSeparator();
        edit->addAction("Select Objects with Materials", [] { exec("hyperShade", QJsonObject{{"objects", ""}}); });
        edit->addAction("Select Materials from Objects", [] { exec("hyperShade", QJsonObject{{"shaderNetworksSelectMaterialNodes", true}}); });

        QMenu* view = menuBar()->addMenu("View");
        view->addAction("Frame All", [this] { frame(); });

        QMenu* create = menuBar()->addMenu("Create");
        QHash<QString, QMenu*> tops;
        for (const RenderNodeGroup& g : render_node_catalog()) {
            if (g.kinds.isEmpty()) continue;
            const QString top = g.path.section('/', 0, 0);
            if (!tops.contains(top)) tops[top] = create->addMenu(top);
            QMenu* sub = tops[top]->addMenu(g.path.section('/', 1));
            for (const RenderNodeKind& k : g.kinds) sub->addAction(render_node_icon(k.type, 16), k.label, [this, k] { make(k); });
        }

        QMenu* tabs = menuBar()->addMenu("Tabs");
        tabs->addAction("Create New Tab", [this] { new_tab(); });
        tabs->addAction("Rename Tab...", [this] {
            const int i = work_->currentIndex();
            bool ok = false;
            QString to = QInputDialog::getText(this, "Rename Tab", "Name:", QLineEdit::Normal, work_->tabText(i), &ok);
            if (ok && !to.trimmed().isEmpty()) work_->setTabText(i, to.trimmed());
        });
        tabs->addAction("Remove Tab", [this] {
            if (work_->count() < 2) return;
            QWidget* w = work_->currentWidget();
            work_->removeTab(work_->currentIndex());
            w->deleteLater();
        });

        QMenu* gr = menuBar()->addMenu("Graph");
        gr->addAction("Graph Materials on Selected Objects", [this] {
            exec("hyperShade", QJsonObject{{"shaderNetworksSelectMaterialNodes", true}});
            graph(4);
        });
        gr->addAction("Clear Graph", [this] {
            if (NodeEditor* ed = editor()) ed->clear_graph();
        });
        gr->addSeparator();
        gr->addAction("Input Connections", [this] { graph(4); });
        gr->addAction("Input and Output Connections", [this] { graph(1); });
        gr->addSeparator();
        gr->addAction("Add Selected to Graph", [this] {
            if (NodeEditor* ed = editor()) ed->add_selected();
        });

        QMenu* window = menuBar()->addMenu("Window");
        for (QDockWidget* d : {browser_dock_, create_dock_, viewer_dock_, property_dock_}) window->addAction(d->toggleViewAction());
        window->addSeparator();
        window->addAction("Attribute Editor...", [] { mel("AttributeEditor;"); });
        window->addAction("Node Editor...", [] { mel("NodeEditorWindow;"); });
    }

    void refresh() {
        QJsonObject all = query("shading").toObject();
        filling_ = true;
        const QString want = search_->text().trimmed();
        QSet<QString> selected;
        for (const QJsonValue& s : state()["selection"].toArray()) selected.insert(s.toString());
        QString shown_name, shown_type;
        auto fill = [&](const QString& title, const char* key) {
            QListWidget* l = lists_.value(title);
            if (!l) return;
            QStringList before, now;
            for (int i = 0; i < l->count(); ++i) before << l->item(i)->text() + "|" + l->item(i)->data(Qt::UserRole + 2).toString();
            QJsonArray items = all[key].toArray();
            l->clear();
            for (const QJsonValue& v : items) {
                QJsonObject o = v.toObject();
                const QString name = o["name"].toString(), type = o["type"].toString();
                if (!want.isEmpty() && !name.contains(want, Qt::CaseInsensitive) && !type.contains(want, Qt::CaseInsensitive)) continue;
                QIcon ic;
                const QString image = o["image"].toString();
                const QImage rendered = title == "Materials" || title == "Textures" ? harold_swatch(name, 64) : QImage();
                if (!rendered.isNull()) {
                    ic = QIcon(QPixmap::fromImage(rendered));
                } else if (title == "Materials") {
                    ic = QIcon(QPixmap::fromImage(ball(look_of(name, type), 64)));
                } else if (!image.isEmpty() && !image_tile(image, 64).isNull()) {
                    ic = QIcon(image_tile(image, 64));
                } else {
                    ic = render_node_icon(type, 64);
                }
                // The swatch as it is when picked too: the selection shows on its name.
                QIcon plain;
                const QPixmap pm = ic.pixmap(64, 64);
                plain.addPixmap(pm, QIcon::Normal);
                plain.addPixmap(pm, QIcon::Selected);
                auto* it = new QListWidgetItem(plain, name, l);
                QString tip = type;
                if (o.contains("written") && !o["written"].toString().isEmpty()) tip += "\n" + (image.isEmpty() ? "Not found: " + o["written"].toString() : image);
                it->setToolTip(tip);
                it->setData(Qt::UserRole, type);
                if (selected.contains(name)) {
                    it->setSelected(true);
                    if ((title == "Materials" || title == "Textures" || title == "Utilities") && shown_name.isEmpty()) {
                        shown_name = name;
                        shown_type = type;
                    }
                }
            }
        };
        fill("Materials", "materials");
        fill("Textures", "textures");
        fill("Utilities", "utilities");
        fill("Rendering", "rendering");
        fill("Lights", "lights");
        fill("Cameras", "cameras");
        fill("Shading Groups", "groups");
        filling_ = false;
        viewer_->show_material(shown_name, shown_type);
        const QString project = all["project"].toString();
        if (!project.isEmpty() && project != project_) {
            project_ = project;
            auto* model = static_cast<QFileSystemModel*>(files_->model());
            files_->setRootIndex(model->setRootPath(project));
        }
    }

    QTabWidget* tabs_ = nullptr;
    QTabWidget* work_ = nullptr;
    QLineEdit* search_ = nullptr;
    QTreeView* files_ = nullptr;
    QHash<QString, QListWidget*> lists_;
    MaterialViewer* viewer_ = nullptr;
    QDockWidget *browser_dock_ = nullptr, *create_dock_ = nullptr, *viewer_dock_ = nullptr, *property_dock_ = nullptr;
    QTimer* timer_ = nullptr;
    QString project_;
    bool filling_ = false;
};

} // namespace

void MainWindow::show_hypershade() {
    if (!hypershade_window_) {
        hypershade_window_ = new Hypershade(this);
        hypershade_window_->setWindowFlags(editor_window_flags());
    }
    hypershade_window_->show();
    hypershade_window_->raise();
}

} // namespace az
