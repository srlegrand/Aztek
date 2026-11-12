#include "rendernodes.h"
#include "host.h"

#include <QDialog>
#include <QDialogButtonBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QJsonArray>
#include <QJsonObject>
#include <QLineEdit>
#include <QListWidget>
#include <QPainter>
#include <QSplitter>
#include <QTreeWidget>
#include <QVBoxLayout>
#include <cmath>

namespace az {

namespace {

QJsonObject one(const QString& a) {
    return QJsonObject{{"args", QJsonArray{a}}};
}

QStringList types_of(const QString& classification) {
    QStringList out;
    for (const QJsonValue& v : ask("listNodeTypes", one(classification)).toArray()) out << v.toString();
    return out;
}

// Harold's node types keep the ai prefix of the files they are read from.
bool harold(const QString& type) {
    return type.startsWith("ai") && type.size() > 2 && type[2].isUpper();
}

QString group_of(const QString& type) {
    static QHash<QString, QString> cache;
    if (cache.isEmpty()) {
        for (const char* c : {"shader/surface", "shader/volume", "shader/displacement", "texture/2d", "texture/3d", "texture/environment", "texture", "light", "utility"})
            for (const QString& t : types_of(c))
                if (!cache.contains(t)) cache[t] = c;
    }
    return cache.value(type);
}

} // namespace

QString render_node_label(const QString& type) {
    static const QHash<QString, QString> named{{"file", "File"}, {"place2dTexture", "2d Placement"}, {"place3dTexture", "3d Placement"}, {"bump2d", "Bump 2d"}, {"bump3d", "Bump 3d"},
                                               {"phongE", "Phong E"}, {"multDL", "Mult DL"}, {"addDL", "Add DL"}, {"hsvToRgb", "Hsv To Rgb"}, {"rgbToHsv", "Rgb To Hsv"},
                                               {"aiBump2d", "Bump 2d"}};
    if (named.contains(type)) return named.value(type);
    QString t = harold(type) ? type.mid(2) : type;
    QString out;
    for (int i = 0; i < t.size(); ++i) {
        QChar c = t[i];
        if (i == 0) {
            out += c.toUpper();
        } else if (c == '_') {
            out += ' ';
        } else if (c.isUpper() && !t[i - 1].isUpper()) {
            out += ' ';
            out += c;
        } else {
            out += c;
        }
    }
    return out;
}

QIcon render_node_icon(const QString& type, int size) {
    QPixmap px(size, size);
    px.fill(Qt::transparent);
    QPainter p(&px);
    p.setRenderHint(QPainter::Antialiasing);
    const QString g = group_of(type);
    const QRectF r(1.5, 1.5, size - 3, size - 3);
    if (g.startsWith("shader")) {
        QRadialGradient grad(r.center() - QPointF(size * 0.15, size * 0.18), size * 0.6);
        grad.setColorAt(0, QColor(240, 240, 240));
        grad.setColorAt(1, QColor(70, 70, 70));
        p.setPen(Qt::NoPen);
        p.setBrush(grad);
        p.drawEllipse(r);
    } else if (g.startsWith("texture")) {
        p.setPen(QPen(QColor(25, 25, 25), 1));
        p.setBrush(QColor(40, 40, 40));
        p.drawRect(r);
        const double h = r.width() / 2;
        p.fillRect(QRectF(r.left(), r.top(), h, h), QColor(60, 190, 200));
        p.fillRect(QRectF(r.left() + h, r.top() + h, h, h), QColor(60, 190, 200));
    } else if (g == "light") {
        p.setPen(QPen(QColor(250, 220, 90), 1.4));
        p.setBrush(QColor(250, 220, 90));
        p.drawEllipse(r.center(), size * 0.2, size * 0.2);
        for (int k = 0; k < 8; ++k) {
            const double a = k * 3.14159265 / 4;
            p.drawLine(r.center() + QPointF(std::cos(a), std::sin(a)) * size * 0.3, r.center() + QPointF(std::cos(a), std::sin(a)) * size * 0.45);
        }
    } else {
        p.setPen(QPen(QColor(25, 25, 25), 1));
        p.setBrush(QColor(120, 150, 200));
        p.drawRoundedRect(r.adjusted(1, 3, -1, -3), 2, 2);
    }
    return QIcon(px);
}

const QList<RenderNodeGroup>& render_node_catalog() {
    static QList<RenderNodeGroup> cat;
    if (!cat.isEmpty()) return cat;
    auto kinds = [](const QStringList& types, bool want_harold, const QString& flag) {
        QList<RenderNodeKind> out;
        for (const QString& t : types) {
            // Bookkeeping nodes the interface makes for itself are not for creating by hand.
            if (t == "materialInfo" || t.startsWith("mia_")) continue;
            if (harold(t) == want_harold) out.append({render_node_label(t), t, flag});
        }
        std::sort(out.begin(), out.end(), [](const RenderNodeKind& a, const RenderNodeKind& b) { return a.label.compare(b.label, Qt::CaseInsensitive) < 0; });
        return out;
    };
    const QStringList surface = types_of("shader/surface"), volume = types_of("shader/volume"), disp = types_of("shader/displacement");
    const QStringList tex2 = types_of("texture/2d"), tex3 = types_of("texture/3d"), env = types_of("texture/environment");
    const QStringList lights = types_of("light"), utils = types_of("utility");
    QStringList other;
    for (const QString& t : types_of("texture"))
        if (!tex2.contains(t) && !tex3.contains(t) && !env.contains(t)) other << t;
    QStringList layered, flat2;
    for (const QString& t : tex2) (t == "layeredTexture" ? layered : flat2) << t;
    other += layered;
    // The reference's groups, in its order. The ones with nothing in them yet are still listed.
    cat = {
        {"Aztek/Surface", kinds(surface, false, "asShader")},
        {"Aztek/Volumetric", kinds(volume, false, "asShader")},
        {"Aztek/Displacement", kinds(disp, false, "asShader")},
        {"Aztek/2D Textures", kinds(flat2, false, "asTexture")},
        {"Aztek/3D Textures", kinds(tex3, false, "asTexture")},
        {"Aztek/Env Textures", kinds(env, false, "asTexture")},
        {"Aztek/Other Textures", kinds(other, false, "asTexture")},
        {"Aztek/Lights", kinds(lights, false, "asLight")},
        {"Aztek/Utilities", kinds(utils, false, "asUtility")},
        {"Harold/Texture", kinds(tex2 + tex3 + env, true, "asTexture")},
        {"Harold/Light", kinds(lights, true, "asLight")},
        {"Harold/Shader", kinds(surface + volume + disp, true, "asShader")},
        {"Harold/Utility", kinds(utils, true, "asUtility")},
    };
    return cat;
}

QString make_render_node(const RenderNodeKind& kind, QStringList* with) {
    QJsonObject call{{"args", QJsonArray{kind.type}}};
    call[kind.flag] = true;
    QString node = exec("shadingNode", call).toString();
    if (node.isEmpty()) return node;
    // A texture comes with its placement, wired as the reference wires it: a 2D one with where it sits in UV space, a
    // 3D one with where it sits in the scene.
    const QString group = group_of(kind.type);
    if (kind.type == "file") {
        QString place = exec("shadingNode", QJsonObject{{"args", QJsonArray{"place2dTexture"}}, {"asUtility", true}}).toString();
        if (!place.isEmpty()) exec("defaultNavigation", QJsonObject{{"connectToExisting", true}, {"source", place}, {"destination", node}});
        if (with && !place.isEmpty()) *with << place;
    } else if (group == "texture/2d" && !harold(kind.type) && kind.type != "layeredTexture") {
        QString place = exec("shadingNode", QJsonObject{{"args", QJsonArray{"place2dTexture"}}, {"asUtility", true}}).toString();
        if (!place.isEmpty()) {
            exec("connectAttr", QJsonObject{{"args", QJsonArray{place + ".outUV", node + ".uvCoord"}}});
            if (ask("attributeQuery", QJsonObject{{"args", QJsonArray{"uvFilterSize"}}, {"node", node}, {"exists", true}}).toBool())
                exec("connectAttr", QJsonObject{{"args", QJsonArray{place + ".outUvFilterSize", node + ".uvFilterSize"}}});
            if (with) *with << place;
        }
    } else if (group == "texture/3d" && !harold(kind.type)) {
        QString place = exec("shadingNode", QJsonObject{{"args", QJsonArray{"place3dTexture"}}, {"asUtility", true}}).toString();
        if (!place.isEmpty()) {
            exec("connectAttr", QJsonObject{{"args", QJsonArray{place + ".worldInverseMatrix[0]", node + ".placementMatrix"}}});
            if (with) *with << place;
        }
    }
    return node;
}

CreateBar::CreateBar(QWidget* parent) : QWidget(parent) {
    auto* v = new QVBoxLayout(this);
    v->setContentsMargins(2, 2, 2, 2);
    v->setSpacing(2);
    filter_ = new QLineEdit(this);
    filter_->setPlaceholderText("Search...");
    filter_->setClearButtonEnabled(true);
    v->addWidget(filter_);
    auto* split = new QSplitter(Qt::Horizontal, this);
    groups_ = new QTreeWidget(split);
    groups_->setHeaderHidden(true);
    groups_->setIndentation(12);
    groups_->setObjectName("createGroups");
    nodes_ = new QListWidget(split);
    nodes_->setIconSize(QSize(18, 18));
    nodes_->setObjectName("createNodes");
    split->addWidget(groups_);
    split->addWidget(nodes_);
    split->setStretchFactor(0, 0);
    split->setStretchFactor(1, 1);
    split->setSizes({130, 190});
    v->addWidget(split, 1);
    QHash<QString, QTreeWidgetItem*> tops;
    for (const RenderNodeGroup& g : render_node_catalog()) {
        const QString top = g.path.section('/', 0, 0);
        if (!tops.contains(top)) {
            tops[top] = new QTreeWidgetItem(groups_, {top});
            tops[top]->setData(0, Qt::UserRole, top + "/");
            tops[top]->setExpanded(true);
        }
        auto* it = new QTreeWidgetItem(tops[top], {g.path.section('/', 1)});
        it->setData(0, Qt::UserRole, g.path);
    }
    groups_->setCurrentItem(groups_->topLevelItem(0));
    connect(groups_, &QTreeWidget::currentItemChanged, this, [this] { fill(); });
    connect(filter_, &QLineEdit::textChanged, this, [this] { fill(); });
    connect(nodes_, &QListWidget::itemClicked, this, [this](QListWidgetItem* it) {
        emit picked(RenderNodeKind{it->text(), it->data(Qt::UserRole).toString(), it->data(Qt::UserRole + 1).toString()});
    });
    fill();
}

void CreateBar::fill() {
    nodes_->clear();
    const QString want = groups_->currentItem() ? groups_->currentItem()->data(0, Qt::UserRole).toString() : QString();
    const QString text = filter_->text().trimmed();
    for (const RenderNodeGroup& g : render_node_catalog()) {
        // A group's own entry lists it; the heading above lists every group under it; a search looks through them all.
        if (text.isEmpty() && !(want.endsWith('/') ? g.path.startsWith(want) : g.path == want)) continue;
        for (const RenderNodeKind& k : g.kinds) {
            if (!text.isEmpty() && !k.label.contains(text, Qt::CaseInsensitive) && !k.type.contains(text, Qt::CaseInsensitive)) continue;
            auto* it = new QListWidgetItem(render_node_icon(k.type, 18), k.label, nodes_);
            it->setData(Qt::UserRole, k.type);
            it->setData(Qt::UserRole + 1, k.flag);
            it->setToolTip(k.type);
        }
    }
}

QString create_render_node(QWidget* parent, const QString& plug) {
    QDialog d(parent);
    d.setWindowTitle("Create Render Node");
    auto* v = new QVBoxLayout(&d);
    v->setContentsMargins(4, 4, 4, 4);
    auto* bar = new CreateBar(&d);
    v->addWidget(bar, 1);
    auto* box = new QDialogButtonBox(QDialogButtonBox::Close, &d);
    v->addWidget(box);
    QObject::connect(box, &QDialogButtonBox::rejected, &d, &QDialog::reject);
    QString made;
    QObject::connect(bar, &CreateBar::picked, &d, [&](const RenderNodeKind& k) {
        made = make_render_node(k);
        if (made.isEmpty()) return;
        // The node's usual output goes into the attribute: a colour into a colour, the alpha into a number.
        exec("defaultNavigation", QJsonObject{{"connectToExisting", true}, {"source", made}, {"destination", plug}});
        d.accept();
    });
    d.resize(440, 420);
    d.exec();
    // The new node is what the Attribute Editor shows next, as the reference shows it.
    if (!made.isEmpty()) exec("select", one(made));
    return made;
}

} // namespace az
