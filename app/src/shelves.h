// The shelves: tabs of buttons, each a script. Saved between sessions; edited in the Shelf Editor or by dragging scripts
// and menu items on.
#pragma once
#include <QJsonArray>
#include <QJsonObject>
#include <QWidget>
#include <functional>

class QTabWidget;

namespace az {

class ShelfBar : public QWidget {
    Q_OBJECT
public:
    // run(command, language): language is "mel" or "python".
    explicit ShelfBar(std::function<void(const QString&, const QString&)> run, QWidget* parent = nullptr);
    void add_to_current(const QString& label, const QString& icon_name, const QString& command, const QString& language);
    void open_editor();
    // A widget at the far right of the tab row. Tabs that don't fit scroll; it stays.
    void set_corner(QWidget* w);
protected:
    void resizeEvent(QResizeEvent* e) override;
    bool eventFilter(QObject* o, QEvent* e) override;

public:
    // Reads the reference's shelf_Name.mel into a new shelf: its shelfButton blocks.
    void import_mel(const QString& path);
    static bool read_mel(const QString& path, QJsonObject* shelf);
    void rebuild();
    void save() const;
    QJsonArray& data() { return shelves_; }
    static QJsonArray defaults();
private:
    void tab_menu(QPoint global, int index);
    void button_menu(QPoint global, int shelf, int item);
    void edit_item(int shelf, int item);
    std::function<void(const QString&, const QString&)> run_;
    QWidget* corner_ = nullptr;
    QWidget* corner_gap_ = nullptr;
    void place_corner();
    QTabWidget* tabs_;
    QJsonArray shelves_;
    friend class ShelfPage;
};

} // namespace az
