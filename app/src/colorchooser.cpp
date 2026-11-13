#include "colorchooser.h"
#include "host.h"

#include <QButtonGroup>
#include <QColorDialog>
#include <QDialog>
#include <QDialogButtonBox>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QPointer>
#include <QPushButton>
#include <QRadioButton>
#include <QVBoxLayout>

namespace az {

namespace {

void paint_swatch(QPushButton* b, const QColor& c, bool chosen) {
    b->setStyleSheet(QString("background-color: %1; border: %2;").arg(c.name(), chosen ? QString("2px solid #fff") : QString("1px solid #222")));
}

// A grid of palette swatches that remembers which is chosen.
class PaletteGrid : public QWidget {
public:
    PaletteGrid(int first, int count, int columns, int chosen, QWidget* parent) : QWidget(parent), first_(first), chosen_(chosen) {
        auto* grid = new QGridLayout(this);
        grid->setContentsMargins(0, 0, 0, 0);
        grid->setSpacing(2);
        for (int i = 0; i < count; ++i) {
            int index = first + i;
            auto* b = new QPushButton(this);
            b->setFixedSize(26, 20);
            b->setObjectName("colorSwatch");
            b->setToolTip(first == 0 && index == 0 ? QString("0: no colour") : QString::number(index));
            buttons_.append(b);
            grid->addWidget(b, i / columns, i % columns);
            connect(b, &QPushButton::clicked, this, [this, index] { set_chosen(index); });
        }
        repaint_all();
    }
    int chosen() const { return chosen_; }
    void set_chosen(int index) {
        chosen_ = index;
        repaint_all();
    }

private:
    void repaint_all() {
        for (int i = 0; i < buttons_.size(); ++i) paint_swatch(buttons_[i], palette_color(first_ + i), first_ + i == chosen_);
    }
    int first_, chosen_;
    QList<QPushButton*> buttons_;
};

QPushButton* rgb_button(QColor* value, QWidget* parent) {
    auto* b = new QPushButton(parent);
    b->setFixedSize(56, 20);
    b->setObjectName("colorSwatch");
    paint_swatch(b, *value, false);
    QObject::connect(b, &QPushButton::clicked, b, [b, value] {
        QColor c = QColorDialog::getColor(*value, b->window(), "Color");
        if (!c.isValid()) return;
        *value = c;
        paint_swatch(b, c, false);
    });
    return b;
}

} // namespace

QColor palette_color(int index) {
    QJsonArray c = ask("colorIndex", QJsonObject{{"args", QJsonArray{index}}, {"query", true}}).toArray();
    return QColor::fromRgbF(c.at(0).toDouble(), c.at(1).toDouble(), c.at(2).toDouble());
}

ColorChoice choose_display_color(QWidget* parent, const QString& title, int index, bool use_rgb, const QColor& rgb) {
    QDialog d(parent);
    d.setWindowTitle(title);
    auto* v = new QVBoxLayout(&d);
    auto* by_index = new QRadioButton("Index", &d);
    auto* by_rgb = new QRadioButton("RGB", &d);
    (use_rgb ? by_rgb : by_index)->setChecked(true);
    auto* grid = new PaletteGrid(0, 32, 8, index, &d);
    QColor chosen = rgb;
    auto* swatch = rgb_button(&chosen, &d);
    v->addWidget(by_index);
    v->addWidget(grid);
    auto* row = new QHBoxLayout();
    row->addWidget(by_rgb);
    row->addWidget(swatch);
    row->addStretch(1);
    v->addLayout(row);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &d);
    QObject::connect(buttons, &QDialogButtonBox::accepted, &d, &QDialog::accept);
    QObject::connect(buttons, &QDialogButtonBox::rejected, &d, &QDialog::reject);
    v->addWidget(buttons);
    ColorChoice out;
    out.accepted = d.exec() == QDialog::Accepted;
    out.use_rgb = by_rgb->isChecked();
    out.index = grid->chosen();
    out.rgb = chosen;
    return out;
}

void show_wireframe_color_window(QWidget* parent) {
    static QPointer<QDialog> window;
    if (window) {
        window->raise();
        window->activateWindow();
        return;
    }
    auto* d = new QDialog(parent);
    window = d;
    d->setObjectName("wireframeColorWindow");
    d->setAttribute(Qt::WA_DeleteOnClose);
    d->setWindowTitle("Wireframe Color");
    auto* v = new QVBoxLayout(d);
    auto* group = new QButtonGroup(d);
    auto* by_user = new QRadioButton("User colors", d);
    auto* by_index = new QRadioButton("Index (Drawing Override)", d);
    auto* by_rgb = new QRadioButton("RGB", d);
    group->addButton(by_user);
    group->addButton(by_index);
    group->addButton(by_rgb);
    by_user->setChecked(true);
    auto* user = new PaletteGrid(24, 8, 8, 24, d);
    auto* grid = new PaletteGrid(0, 32, 8, 13, d);
    auto* rgb = new QColor(255, 0, 0);
    QObject::connect(d, &QObject::destroyed, [rgb] { delete rgb; });
    auto* swatch = rgb_button(rgb, d);
    v->addWidget(by_user);
    v->addWidget(user);
    v->addWidget(by_index);
    v->addWidget(grid);
    auto* row = new QHBoxLayout();
    row->addWidget(by_rgb);
    row->addWidget(swatch);
    row->addStretch(1);
    v->addLayout(row);
    auto* buttons = new QHBoxLayout();
    auto* apply = new QPushButton("Apply", d);
    auto* standard = new QPushButton("Default", d);
    auto* close = new QPushButton("Close", d);
    buttons->addWidget(apply);
    buttons->addWidget(standard);
    buttons->addWidget(close);
    v->addLayout(buttons);
    QObject::connect(apply, &QPushButton::clicked, d, [=] {
        if (by_user->isChecked()) {
            exec("color", QJsonObject{{"userDefined", user->chosen() - 23}});
        } else if (by_rgb->isChecked()) {
            exec("color", QJsonObject{{"rgbColor", QJsonArray{rgb->redF(), rgb->greenF(), rgb->blueF()}}});
        } else {
            for (const QJsonValue& s : state()["selection"].toArray()) {
                exec("setAttr", QJsonObject{{"args", QJsonArray{s.toString() + ".overrideEnabled", true}}});
                exec("setAttr", QJsonObject{{"args", QJsonArray{s.toString() + ".overrideRGBColors", false}}});
                exec("setAttr", QJsonObject{{"args", QJsonArray{s.toString() + ".overrideColor", grid->chosen()}}});
            }
        }
    });
    QObject::connect(standard, &QPushButton::clicked, d, [] { exec("color"); });
    QObject::connect(close, &QPushButton::clicked, d, &QDialog::close);
    d->show();
}

} // namespace az
