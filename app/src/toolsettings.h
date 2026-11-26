// The Tool Settings window: the current tool's options. A page per tool, as the reference's double-click on a tool opens.
#pragma once
#include "panels.h"
#include <QHash>
#include <QJsonArray>
#include <functional>

class QCheckBox;
class QPushButton;
class QComboBox;
class QFormLayout;
class QDoubleSpinBox;
class QListWidget;
class QTreeWidget;
class QLineEdit;
class QStackedLayout;
class QButtonGroup;
class QSlider;

namespace az {

// Soft select's falloff curve, as Tool Settings has it, for other panels (the Modeling Toolkit). reload, when given, is
// set to what reads the curve again after it was changed elsewhere.
QWidget* falloff_curve(QWidget* parent, std::function<void()>* reload = nullptr);
// A field or list that takes the mouse wheel only once clicked into; otherwise the panel around it scrolls.
void no_stray_wheel(QWidget* w);

/// The reference's joint orientation rows: Orient Joint to World, Primary Axis, Secondary Axis and Secondary Axis World
/// Orientation, added to a form. They start from an orientJoint order ("xyz", "none" for the world) and a
/// secondaryAxisOrient ("yup", "none"), and hand both back on every change.
void joint_orient_rows(QFormLayout* form, QWidget* parent, const QString& order, const QString& sao, std::function<void(const QString& order, const QString& sao)> changed);

/// The Orient Joint options box.
void open_orient_joint_options();
/// Skeleton > Orient Joint with the options box's settings, on the selected joints.
void orient_selected_joints();

class ToolSettings : public Panel {
    Q_OBJECT
public:
    explicit ToolSettings(QWidget* parent = nullptr);
    void set_tool(const QString& tool);
    /// Shows Edit Pivot as on or off without sending it again.
    void show_pivot_mode(bool on);
protected:
    void refresh() override;
private:
    QWidget* build_transform_page();
    QWidget* build_paint_page();
    QWidget* build_joint_page();
    void push_joint_settings();
    QWidget* build_ik_page(bool spline);
    void push_ik_settings(bool spline);
    QWidget* build_curve_page();
    QWidget* build_sculpt_page();
    QWidget* build_groom_page();
    void refresh_groom();
    QWidget* build_attr_page();
    QWidget* build_member_page();
    void refresh_attr();
    void refresh_member();
    void attr_set(const QString& flag, const QJsonValue& v);
    void refresh_sculpt();
    void refresh_paint();
    void move_weights();
    void show_vertex_influences();
    void filter_influences();
public:
    // A lock button clicked: that influence held or released, or every selected one when it is among them (#105).
    void toggle_lock(QTreeWidgetItem* clicked);
private:
    void refresh_transform();
    QString ctx_kind() const;
    QString ctx_name() const;
    void ctx_set(const QString& flag, const QJsonValue& v);
    QJsonValue ctx_get(const QString& flag) const;
    void restore_options();
    void paint_set(const QString& flag, const QJsonValue& v);
    QDoubleSpinBox* slider_row(QFormLayout* form, const QString& label, double lo, double hi, double step, std::function<void(double)> on);

    QString tool_ = "select";
    QStackedLayout* stack_ = nullptr;
    QLabel* title_ = nullptr;
    QWidget* transform_page_ = nullptr;
    QWidget* paint_page_ = nullptr;
    QWidget* joint_page_ = nullptr;
    QWidget* ik_page_ = nullptr;
    QWidget* ik_spline_page_ = nullptr;
    QWidget* curve_page_ = nullptr;
    QWidget* sculpt_page_ = nullptr;
    QWidget* groom_page_ = nullptr;
    QComboBox* groom_tool_ = nullptr;
    QDoubleSpinBox* groom_size_ = nullptr;
    QDoubleSpinBox* groom_strength_ = nullptr;
    QCheckBox* groom_collide_ = nullptr;
    QDoubleSpinBox* groom_distance_ = nullptr;
    QCheckBox* groom_invert_frozen_ = nullptr;
    QCheckBox* groom_alt_ = nullptr;
    QWidget* attr_page_ = nullptr;
    QWidget* member_page_ = nullptr;
    QComboBox* attr_list_ = nullptr;
    QButtonGroup* attr_op_ = nullptr;
    QComboBox* attr_stamp_ = nullptr;
    QDoubleSpinBox* attr_value_ = nullptr;
    QDoubleSpinBox* attr_opacity_ = nullptr;
    QDoubleSpinBox* attr_radius_ = nullptr;
    QCheckBox* attr_feedback_ = nullptr;
    QComboBox* member_list_ = nullptr;
    QComboBox* sculpt_tool_ = nullptr;
    QDoubleSpinBox* sculpt_size_ = nullptr;
    QDoubleSpinBox* sculpt_strength_ = nullptr;
    QCheckBox* sculpt_invert_ = nullptr;
    QCheckBox* sculpt_sym_ = nullptr;
    // Paint page.
    QListWidget* skins_ = nullptr;
    QTreeWidget* influences_ = nullptr;
    QButtonGroup* sort_ = nullptr;
    QLineEdit* search_ = nullptr;
    QStringList pick_order_;
    QButtonGroup* skin_mode_ = nullptr;
    QCheckBox* ramp_ = nullptr;
    QButtonGroup* paint_select_ = nullptr;
    QComboBox* normalize_ = nullptr;
    QButtonGroup* op_ = nullptr;
    QComboBox* stamp_ = nullptr;
    QDoubleSpinBox* value_ = nullptr;
    QDoubleSpinBox* opacity_ = nullptr;
    QDoubleSpinBox* radius_ = nullptr;
    QCheckBox* feedback_ = nullptr;
    QString last_skin_;
    QStringList last_influences_;
    // Transform page.
    QCheckBox* soft_ = nullptr;
    QDoubleSpinBox* soft_radius_ = nullptr;
    QComboBox* soft_mode_ = nullptr;
    QComboBox* axis_ = nullptr;
    QString axis_tool_;
    QWidget* transform_section_ = nullptr;
    QWidget* custom_row_ = nullptr;
    QComboBox* step_mode_ = nullptr;
    QDoubleSpinBox* step_size_ = nullptr;
    QCheckBox* preserve_children_ = nullptr;
    QCheckBox* preserve_uvs_ = nullptr;
    QCheckBox* keep_spacing_ = nullptr;
    QPushButton* edit_pivot_ = nullptr;
    QCheckBox* update_triad_ = nullptr;
    QCheckBox* shift_extrude_ = nullptr;
    QCheckBox* shift_duplicate_ = nullptr;
    QCheckBox* negative_ = nullptr;
    QCheckBox* use_manip_pivot_ = nullptr;
    QComboBox* constraint_ = nullptr;
    QComboBox* sym_mode_ = nullptr;
    QComboBox* sym_axis_ = nullptr;
    QDoubleSpinBox* sym_tol_ = nullptr;
    QCheckBox* sym_seam_ = nullptr;
    QWidget* falloff_curve_ = nullptr;
    QList<QPair<QCheckBox*, QString>> checks_;
    bool loading_ = false;
signals:
    void axisOrientation(const QString& mode);
    void pivotMode(bool on);
    void curveDegree(int degree);
    // Step snap: key is stepMove, stepRotate or stepScale; 0 is off.
    void stepSnap(const QString& key, double value);
};

} // namespace az
