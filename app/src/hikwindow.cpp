// The HumanIK window: Character and Source at the top, then the Definition tab with a figure of the slots to assign, Controls with
// a figure that selects the control rig, Custom Rig, and Properties. It reads hikInfo and acts through the hik commands.
// The figure is drawn here from circles and lines.
#include "editors.h"
#include "panels.h"

#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDoubleSpinBox>
#include <QFileDialog>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QJsonArray>
#include <QLabel>
#include <QLineEdit>
#include <QMenu>
#include <QMessageBox>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QScrollArea>
#include <QTabWidget>
#include <QToolButton>
#include <QVBoxLayout>

#include "host.h"

namespace az {

namespace {

struct Spot {
    const char* slot;
    double x, y;
    bool small;
};

// In a unit box, the character facing the viewer: its left on the right of the figure.
const Spot SPOTS[] = {
    {"Reference", 0.50, 0.97, false}, {"Hips", 0.50, 0.52, false}, {"HipsTranslation", 0.46, 0.55, true},
    {"Spine", 0.50, 0.46, false}, {"Spine1", 0.50, 0.42, true}, {"Spine2", 0.50, 0.38, true}, {"Spine3", 0.50, 0.34, true}, {"Spine4", 0.50, 0.30, true}, {"Spine5", 0.50, 0.27, true},
    {"Neck", 0.50, 0.21, false}, {"Neck1", 0.50, 0.185, true}, {"Head", 0.50, 0.12, false},
    {"LeftShoulder", 0.56, 0.245, false}, {"LeftArm", 0.63, 0.25, false}, {"LeftArmRoll", 0.68, 0.25, true}, {"LeftForeArm", 0.75, 0.25, false}, {"LeftForeArmRoll", 0.80, 0.25, true}, {"LeftHand", 0.86, 0.25, false},
    {"RightShoulder", 0.44, 0.245, false}, {"RightArm", 0.37, 0.25, false}, {"RightArmRoll", 0.32, 0.25, true}, {"RightForeArm", 0.25, 0.25, false}, {"RightForeArmRoll", 0.20, 0.25, true}, {"RightHand", 0.14, 0.25, false},
    {"LeftUpLeg", 0.57, 0.55, false}, {"LeftUpLegRoll", 0.585, 0.62, true}, {"LeftLeg", 0.60, 0.72, false}, {"LeftLegRoll", 0.605, 0.79, true}, {"LeftFoot", 0.61, 0.89, false}, {"LeftToeBase", 0.66, 0.93, false},
    {"RightUpLeg", 0.43, 0.55, false}, {"RightUpLegRoll", 0.415, 0.62, true}, {"RightLeg", 0.40, 0.72, false}, {"RightLegRoll", 0.395, 0.79, true}, {"RightFoot", 0.39, 0.89, false}, {"RightToeBase", 0.34, 0.93, false},
};

const char* const LINES[][2] = {
    {"Hips", "Spine"}, {"Spine", "Neck"}, {"Neck", "Head"}, {"Neck", "LeftShoulder"}, {"LeftShoulder", "LeftArm"}, {"LeftArm", "LeftForeArm"}, {"LeftForeArm", "LeftHand"}, {"Neck", "RightShoulder"}, {"RightShoulder", "RightArm"}, {"RightArm", "RightForeArm"}, {"RightForeArm", "RightHand"},
    {"Hips", "LeftUpLeg"}, {"LeftUpLeg", "LeftLeg"}, {"LeftLeg", "LeftFoot"}, {"LeftFoot", "LeftToeBase"}, {"Hips", "RightUpLeg"}, {"RightUpLeg", "RightLeg"}, {"RightLeg", "RightFoot"}, {"RightFoot", "RightToeBase"},
};

// Fingers fan out from each hand, in rows of joints.
QList<Spot> finger_spots() {
    static QList<QByteArray> keep;
    QList<Spot> out;
    const char* digits[] = {"Thumb", "Index", "Middle", "Ring", "Pinky"};
    for (int side = 0; side < 2; ++side) {
        const double x0 = side == 0 ? 0.885 : 0.115, dir = side == 0 ? 1 : -1;
        for (int d = 0; d < 5; ++d) {
            for (int k = 1; k <= 3; ++k) {
                keep.append(QByteArray(side == 0 ? "LeftHand" : "RightHand") + digits[d] + QByteArray::number(k));
                out.append({keep.last().constData(), x0 + dir * 0.022 * k, 0.215 + d * 0.018, true});
            }
        }
    }
    return out;
}

// The figure: a dot per slot, coloured by the definition's check, or by whether a control exists.
class Figure : public QWidget {
public:
    std::function<void(const QString&)> clicked;
    QHash<QString, QColor> colours;
    QString picked;
    explicit Figure(QWidget* parent = nullptr) : QWidget(parent) {
        setMinimumSize(300, 420);
        setMouseTracking(true);
        for (const auto& s : SPOTS) spots_.append(s);
        spots_.append(finger_spots());
    }
protected:
    QPointF at(const Spot& s) const {
        const double scale = qMin(width(), height());
        const double ox = (width() - scale) / 2, oy = (height() - scale) / 2;
        return QPointF(ox + s.x * scale, oy + s.y * scale);
    }
    double radius(const Spot& s) const {
        return (s.small ? 0.011 : 0.018) * qMin(width(), height());
    }
    const Spot* find(const QString& n) const {
        for (const auto& s : spots_)
            if (n == s.slot) return &s;
        return nullptr;
    }
    void paintEvent(QPaintEvent*) override {
        QPainter p(this);
        p.setRenderHint(QPainter::Antialiasing);
        p.fillRect(rect(), QColor(52, 52, 52));
        p.setPen(QPen(QColor(120, 120, 120), 2));
        for (const auto& l : LINES) {
            const Spot* a = find(l[0]);
            const Spot* b = find(l[1]);
            if (a && b) p.drawLine(at(*a), at(*b));
        }
        for (const auto& s : spots_) {
            const QColor c = colours.value(s.slot, QColor(80, 80, 80));
            p.setPen(picked == s.slot ? QPen(Qt::white, 2) : QPen(QColor(30, 30, 30), 1));
            p.setBrush(c);
            p.drawEllipse(at(s), radius(s), radius(s));
        }
        if (!picked.isEmpty()) {
            p.setPen(Qt::white);
            p.drawText(rect().adjusted(6, 6, -6, -6), Qt::AlignLeft | Qt::AlignTop, picked);
        }
    }
    const Spot* under(const QPointF& pos) const {
        const Spot* best = nullptr;
        double dist = 1e9;
        for (const auto& s : spots_) {
            const double d = QLineF(at(s), pos).length();
            if (d < dist && d <= radius(s) + 5) {
                dist = d;
                best = &s;
            }
        }
        return best;
    }
    void mousePressEvent(QMouseEvent* e) override {
        if (const Spot* s = under(e->position())) {
            picked = s->slot;
            update();
            if (clicked) clicked(picked);
        }
    }
    void mouseMoveEvent(QMouseEvent* e) override {
        const Spot* s = under(e->position());
        setToolTip(s ? QString(s->slot) : QString());
    }
private:
    QList<Spot> spots_;
};

class HumanIK : public Panel {
public:
    explicit HumanIK(QWidget* parent) : Panel(parent) {
        auto* v = new QVBoxLayout(this);
        auto* top = new QHBoxLayout;
        top->addWidget(new QLabel("Character:"));
        character_ = new QComboBox;
        character_->setMinimumWidth(130);
        top->addWidget(character_);
        top->addWidget(new QLabel("Source:"));
        source_ = new QComboBox;
        source_->setMinimumWidth(130);
        top->addWidget(source_);
        top->addStretch(1);
        auto* menu_button = new QToolButton;
        menu_button->setText("Menu");
        menu_button->setPopupMode(QToolButton::InstantPopup);
        menu_button->setMenu(build_menu());
        top->addWidget(menu_button);
        v->addLayout(top);

        auto* row = new QHBoxLayout;
        const auto button = [&](const QString& text, const QString& tip, std::function<void()> f) {
            auto* b = new QPushButton(text);
            b->setToolTip(tip);
            connect(b, &QPushButton::clicked, this, f);
            row->addWidget(b);
            return b;
        };
        button("Create Character", "A new empty definition", [this] { create_character(); });
        lock_ = button("Lock", "Lock the definition; its T-pose is where the joints are now", [] { exec("hikToggleLockDefinition"); });
        button("Create Control Rig", "A control rig on this character", [] { exec("hikCreateControlRig"); });
        button("Stance", "Hold the T-pose", [] { exec("hikSetCurrentSource", QJsonObject{{"args", QJsonArray{"Stance"}}}); });
        row->addStretch(1);
        v->addLayout(row);

        tabs_ = new QTabWidget;
        v->addWidget(tabs_, 1);
        tabs_->addTab(build_definition(), "Definition");
        tabs_->addTab(build_controls(), "Controls");
        tabs_->addTab(build_custom_rig(), "Custom Rig");
        tabs_->addTab(build_properties(), "Properties");

        connect(character_, &QComboBox::activated, this, [this](int) {
            exec("hikSetCurrentCharacter", QJsonObject{{"args", QJsonArray{character_->currentText()}}});
        });
        connect(source_, &QComboBox::activated, this, [this](int) {
            exec("hikSetCurrentSource", QJsonObject{{"args", QJsonArray{source_->currentText()}}});
        });
        schedule();
    }
protected:
    void refresh() override {
        const QJsonObject info = ask("hikInfo").toObject();
        const QJsonObject me = info["character"].toObject();
        name_ = me["name"].toString();
        const QSignalBlocker a(character_), b(source_);
        character_->clear();
        for (const auto& c : info["characters"].toArray()) character_->addItem(c.toString());
        character_->setCurrentText(name_);
        source_->clear();
        source_->addItems({"None", "Stance", "Control Rig"});
        for (const auto& c : info["characters"].toArray())
            if (c.toString() != name_) source_->addItem(c.toString());
        source_->setCurrentText(me["source"].toString("None"));
        lock_->setText(me["locked"].toBool() ? "Unlock" : "Lock");
        const QJsonObject filled = me["slots"].toObject();
        const QJsonObject status = me["status"].toObject()["status"].toObject();
        QHash<QString, QColor> colours;
        for (auto it = status.begin(); it != status.end(); ++it) {
            const QString s = it.value().toString();
            colours[it.key()] = s == "red" ? QColor(214, 64, 58) : s == "yellow" ? QColor(232, 196, 60) : QColor(80, 190, 90);
        }
        definition_->colours = colours;
        definition_->update();
        QHash<QString, QColor> controls;
        if (me["controlRig"].toBool())
            for (auto it = filled.begin(); it != filled.end(); ++it) controls[it.key()] = QColor(88, 150, 230);
        controls_->colours = controls;
        controls_->update();
        QString text = me["status"].toObject()["summary"].toString();
        for (const auto& m : me["status"].toObject()["messages"].toArray()) text += "\n" + m.toString();
        status_->setPlainText(name_.isEmpty() ? "No character: Create Character, or Quick Character from the menu." : text);
        joint_->setText(filled[definition_->picked].toString());
        property_node_ = me["propertyNode"].toString();
        const QJsonObject props = me["properties"].toObject();
        updating_ = true;
        for (auto it = prop_spins_.begin(); it != prop_spins_.end(); ++it) it.value()->setValue(props[it.key()].toDouble());
        for (auto it = prop_checks_.begin(); it != prop_checks_.end(); ++it) it.value()->setChecked(props[it.key()].toDouble() != 0);
        for (auto it = prop_modes_.begin(); it != prop_modes_.end(); ++it) it.value()->setCurrentIndex(int(props[it.key()].toDouble()));
        updating_ = false;
    }
private:
    QWidget* build_definition() {
        auto* w = new QWidget;
        auto* h = new QHBoxLayout(w);
        definition_ = new Figure;
        definition_->clicked = [this](const QString&) { schedule(); };
        h->addWidget(definition_, 2);
        auto* side = new QVBoxLayout;
        side->addWidget(new QLabel("Joint in the slot:"));
        joint_ = new QLineEdit;
        joint_->setReadOnly(true);
        side->addWidget(joint_);
        auto* assign = new QPushButton("Assign Selected Bone");
        connect(assign, &QPushButton::clicked, this, [this] { assign_selected(); });
        side->addWidget(assign);
        auto* clear = new QPushButton("Remove Assignment");
        connect(clear, &QPushButton::clicked, this, [this] {
            if (!name_.isEmpty() && !definition_->picked.isEmpty()) exec("hikRemoveCharacterObject", QJsonObject{{"args", QJsonArray{name_, definition_->picked}}});
        });
        side->addWidget(clear);
        auto* mirror = new QPushButton("Mirror Matching");
        mirror->setToolTip("Fill empty slots from their opposites");
        connect(mirror, &QPushButton::clicked, this, [this] { if (!name_.isEmpty()) exec("hikMirrorMatching", QJsonObject{{"args", QJsonArray{name_}}}); });
        side->addWidget(mirror);
        auto* names = new QPushButton("Name Match from Selection...");
        connect(names, &QPushButton::clicked, this, [this] { name_match(); });
        side->addWidget(names);
        status_ = new QPlainTextEdit;
        status_->setReadOnly(true);
        side->addWidget(status_, 1);
        h->addLayout(side, 1);
        return w;
    }
    QWidget* build_controls() {
        auto* w = new QWidget;
        auto* v = new QVBoxLayout(w);
        controls_ = new Figure;
        controls_->clicked = [this](const QString& slot) {
            if (name_.isEmpty()) return;
            QString effector = slot;
            if (slot.endsWith("Hand")) effector = slot + "Effector";
            if (slot.endsWith("Foot")) effector = slot.chopped(4) + "AnkleEffector";
            exec("select", QJsonObject{{"args", QJsonArray{QString("%1_Ctrl_%2").arg(name_, effector)}}});
        };
        v->addWidget(controls_, 1);
        v->addWidget(new QLabel("Click a part to select its control. Create Control Rig makes them."));
        return w;
    }
    QWidget* build_custom_rig() {
        auto* w = new QWidget;
        auto* v = new QVBoxLayout(w);
        auto* note = new QLabel("Mapping a character to a rig's own controls is not here yet: the retarget drives joints. Bake to Skeleton leaves keys on them.");
        note->setWordWrap(true);
        v->addWidget(note);
        v->addStretch(1);
        return w;
    }
    void set_prop(const QString& name, double v) {
        if (!updating_ && !property_node_.isEmpty()) exec("setAttr", QJsonObject{{"args", QJsonArray{property_node_ + "." + name, v}}});
    }
    QWidget* build_properties() {
        auto* inner = new QWidget;
        auto* form = new QFormLayout(inner);
        const auto mode = [&](const QString& label, const QString& attr) {
            auto* c = new QComboBox;
            c->addItems({"Off", "Auto", "User"});
            connect(c, &QComboBox::activated, this, [this, attr](int i) { set_prop(attr, i); });
            prop_modes_[attr] = c;
            form->addRow(label, c);
        };
        const auto spin = [&](const QString& label, const QString& attr, double lo, double hi) {
            auto* s = new QDoubleSpinBox;
            s->setRange(lo, hi);
            connect(s, &QDoubleSpinBox::editingFinished, this, [this, attr, s] { set_prop(attr, s->value()); });
            prop_spins_[attr] = s;
            form->addRow(label, s);
        };
        const auto check = [&](const QString& label, const QString& attr) {
            auto* c = new QCheckBox;
            connect(c, &QCheckBox::clicked, this, [this, attr](bool on) { set_prop(attr, on ? 1 : 0); });
            prop_checks_[attr] = c;
            form->addRow(label, c);
        };
        form->addRow(new QLabel("Retarget Specific"));
        check("Match Source", "MatchSource");
        mode("Action Space Comp Mode", "ScaleCompensationMode");
        spin("Action Space Compensation", "ScaleCompensation", 0, 1000);
        check("Mirror Animation", "Mirror");
        mode("Hips Level Mode", "HipsHeightCompensationMode");
        spin("Hips Level", "HipsHeightCompensation", -1000, 1000);
        mode("Feet Spacing Mode", "AnkleProximityCompensationMode");
        spin("Feet Spacing", "AnkleProximityCompensation", -1000, 1000);
        mode("Ankle Height Comp Mode", "AnkleHeightCompensationMode");
        spin("Ankle Height Compensation", "AnkleHeightCompensation", -1000, 1000);
        form->addRow(new QLabel("Reach"));
        spin("Left Hand Reach T", "ReachActorLeftWrist", 0, 100);
        spin("Right Hand Reach T", "ReachActorRightWrist", 0, 100);
        auto* area = new QScrollArea;
        area->setWidget(inner);
        area->setWidgetResizable(true);
        return area;
    }
    QMenu* build_menu() {
        auto* m = new QMenu(this);
        m->addAction("Create Character", this, [this] { create_character(); });
        m->addAction("Rename Character...", this, [this] {
            bool ok = false;
            const QString n = QInputDialog::getText(this, "Rename Character", "Name", QLineEdit::Normal, name_, &ok);
            if (ok && !n.isEmpty()) {
                exec("rename", QJsonObject{{"args", QJsonArray{name_, n}}});
                exec("hikSetCurrentCharacter", QJsonObject{{"args", QJsonArray{n}}});
            }
        });
        m->addAction("Delete Character", this, [this] { if (!name_.isEmpty()) exec("delete", QJsonObject{{"args", QJsonArray{name_}}}); });
        m->addSeparator();
        m->addAction("Save Skeleton Definition...", this, [this] {
            const QString f = QFileDialog::getSaveFileName(this, "Save Template", QString(), "Templates (*.xml)");
            if (!f.isEmpty()) exec("hikSaveTemplate", QJsonObject{{"args", QJsonArray{f, name_}}});
        });
        m->addAction("Load Skeleton Definition...", this, [this] {
            const QString f = QFileDialog::getOpenFileName(this, "Load Template", QString(), "Templates (*.xml)");
            if (!f.isEmpty()) exec("hikLoadTemplate", QJsonObject{{"args", QJsonArray{f, name_}}});
        });
        m->addSeparator();
        m->addAction("Bake to Skeleton", this, [this] { exec("hikBakeToSkeleton", QJsonObject{{"args", QJsonArray{name_}}}); });
        m->addAction("Bake to Skeleton Options...", this, [this] { bake_options("hikBakeToSkeleton"); });
        m->addAction("Bake to Control Rig", this, [this] { exec("hikBakeToControlRig", QJsonObject{{"args", QJsonArray{name_}}}); });
        m->addAction("Bake to Control Rig Options...", this, [this] { bake_options("hikBakeToControlRig"); });
        m->addSeparator();
        m->addAction("Delete Control Rig", this, [this] { exec("hikDeleteControlRig", QJsonObject{{"args", QJsonArray{name_}}}); });
        return m;
    }
    void bake_options(const QString& command) {
        QDialog d(this);
        d.setWindowTitle("Bake Options");
        auto* form = new QFormLayout(&d);
        auto* start = new QDoubleSpinBox;
        auto* end = new QDoubleSpinBox;
        start->setRange(-1e9, 1e9);
        end->setRange(-1e9, 1e9);
        const QJsonObject st = state();
        start->setValue(st["playbackMin"].toDouble(1));
        end->setValue(st["playbackMax"].toDouble(120));
        auto* by = new QDoubleSpinBox;
        by->setRange(0.01, 1000);
        by->setValue(1);
        auto* euler = new QCheckBox;
        euler->setChecked(true);
        form->addRow("Start Time", start);
        form->addRow("End Time", end);
        form->addRow("Sample By", by);
        form->addRow("Euler Filter", euler);
        auto* box = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel);
        connect(box, &QDialogButtonBox::accepted, &d, &QDialog::accept);
        connect(box, &QDialogButtonBox::rejected, &d, &QDialog::reject);
        form->addRow(box);
        if (d.exec() == QDialog::Accepted)
            exec(command, QJsonObject{{"args", QJsonArray{name_}}, {"time", QString("%1:%2").arg(start->value()).arg(end->value())}, {"sampleBy", by->value()}, {"eulerFilter", euler->isChecked()}});
    }
    void create_character() {
        bool ok = false;
        const QString n = QInputDialog::getText(this, "Create Character", "Name", QLineEdit::Normal, "Character1", &ok);
        if (ok && !n.isEmpty()) exec("hikCreateCharacter", QJsonObject{{"args", QJsonArray{n}}});
    }
    void assign_selected() {
        const QJsonArray sel = ask("ls", QJsonObject{{"selection", true}}).toArray();
        if (name_.isEmpty() || definition_->picked.isEmpty() || sel.isEmpty()) {
            log("Pick a slot in the figure and select a joint, then assign.", 2);
            return;
        }
        exec("setCharacterObject", QJsonObject{{"args", QJsonArray{sel.first().toString(), name_, definition_->picked, 0}}});
    }
    void name_match() {
        bool ok = false;
        const QString p = QInputDialog::getText(this, "Name Match", "Prefix to ignore, such as mixamorig:", QLineEdit::Normal, "", &ok);
        if (ok) exec("hikNameMatch", QJsonObject{{"character", name_}, {"prefix", p}});
    }
    QComboBox* character_ = nullptr;
    QComboBox* source_ = nullptr;
    QPushButton* lock_ = nullptr;
    QTabWidget* tabs_ = nullptr;
    Figure* definition_ = nullptr;
    Figure* controls_ = nullptr;
    QLineEdit* joint_ = nullptr;
    QPlainTextEdit* status_ = nullptr;
    QString name_, property_node_;
    QHash<QString, QDoubleSpinBox*> prop_spins_;
    QHash<QString, QCheckBox*> prop_checks_;
    QHash<QString, QComboBox*> prop_modes_;
    bool updating_ = false;
};

} // namespace

QWidget* human_ik_panel(QWidget* parent) {
    return new HumanIK(parent);
}

} // namespace az
