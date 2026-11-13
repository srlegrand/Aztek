// C++ side of the host library: JSON helpers and a poller that tells the panels when the scene changed.
#pragma once
#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QObject>
#include <QElapsedTimer>
#include <QString>
#include <QTimer>
#include <QWidget>

namespace az {

QString take(char* p);
QJsonValue parse(const QString& s);
QJsonValue query(const char* what, const QString& arg = QString());
QJsonObject state();
// Runs a command, echoed in the history. Returns the result, or an undefined value on error.
QJsonValue exec(const QString& name, const QJsonObject& call = QJsonObject());
// The same without the history lines; the panels see the change at the next poll.
QJsonValue exec_quiet(const QString& name, const QJsonObject& call = QJsonObject());
// A query: no history lines, no change notice.
QJsonValue ask(const QString& name, const QJsonObject& call = QJsonObject());
QJsonValue mel(const QString& src);
// MEL as a menu item or shelf button runs it: an edit brings up its node's handles.
QJsonValue mel_action(const QString& src);
void python(const QString& src);
void log(const QString& text, int kind = 3);
// An editor window's flags: framed, with minimise, maximise and close, and kept above its parent (the main window)
// as the reference's editors stay above its main window, wherever the user clicks.
Qt::WindowFlags editor_window_flags();
// The main window, the parent of an editor made from somewhere that has none to hand.
QWidget* main_window_widget();
// With AZTEK_PROFILE set: "what milliseconds" on stderr, for timing the view and panels while working.
void profile_line(const QString& what, const QElapsedTimer& since);
QString quote(const QString& s);

// Polls the host for scene changes and new history lines.
class Host : public QObject {
    Q_OBJECT
public:
    static Host* get();
    // Checks now rather than at the next tick.
    void poke();
signals:
    void sceneChanged();
    void logLines(const QJsonArray& lines);
private:
    Host();
    QTimer timer_;
    quint64 version_ = 0;
};

// Edit Mesh > Delete Edge/Vertex, as the reference does it: polyDelVertex on vertices, polyDelEdge -cv on edges.
inline void delete_edge_vertex() {
    if (state()["mode"].toString() == "vertex") exec("polyDelVertex");
    else exec("polyDelEdge", QJsonObject{{"cleanVertices", true}});
}

} // namespace az
