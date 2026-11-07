// The Shape Editor: the scene's blend shapes, each target with a weight slider, a key button and target editing.
#pragma once
#include "panels.h"
#include <QHash>
#include <QStringList>

class QDoubleSpinBox;
class QSlider;
class QToolButton;
class QVBoxLayout;

namespace az {

class ShapeEditor : public Panel {
    Q_OBJECT
public:
    explicit ShapeEditor(QWidget* parent = nullptr);
protected:
    void refresh() override;
private:
    struct Row {
        QSlider* slider = nullptr;
        QDoubleSpinBox* spin = nullptr;
        QToolButton* key = nullptr;
        QToolButton* edit = nullptr;
    };
    void rebuild(const QStringList& shapes);
    void add_target(const QString& blend);
    QVBoxLayout* list_ = nullptr;
    QStringList layout_key_;
    QHash<QString, Row> rows_;
    bool loading_ = false;
};

} // namespace az
