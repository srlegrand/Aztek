// C++ side of the host library: JSON helpers and a poller that tells the panels when the scene changed.
#pragma once
#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QObject>
#include <QString>
#include <QTimer>

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

} // namespace az
