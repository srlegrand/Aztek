#include "host.h"
#include "aztek.h"
#include "scriptui.h"

#include <QJsonDocument>
#include <cstdio>

namespace az {

QString take(char* p) {
    if (!p) return QString();
    QString s = QString::fromUtf8(p);
    aztek_free(p);
    return s;
}

QJsonValue parse(const QString& s) {
    if (s.isEmpty()) return QJsonValue();
    QJsonDocument d = QJsonDocument::fromJson(("[" + s + "]").toUtf8());
    return d.array().at(0);
}

QJsonValue query(const char* what, const QString& arg) {
    return parse(take(aztek_query(what, arg.toUtf8().constData())));
}

QJsonObject state() { return query("state").toObject(); }

QJsonValue exec(const QString& name, const QJsonObject& call) {
    QByteArray c = QJsonDocument(call).toJson(QJsonDocument::Compact);
    QJsonObject r = parse(take(aztek_exec(name.toUtf8().constData(), c.constData()))).toObject();
    Host::get()->poke();
    if (r.contains("error")) return QJsonValue(QJsonValue::Undefined);
    return r.value("ok");
}

QJsonValue exec_quiet(const QString& name, const QJsonObject& call) {
    QByteArray c = QJsonDocument(call).toJson(QJsonDocument::Compact);
    QJsonObject r = parse(take(aztek_exec_quiet(name.toUtf8().constData(), c.constData()))).toObject();
    if (r.contains("error")) return QJsonValue(QJsonValue::Undefined);
    return r.value("ok");
}

QJsonValue ask(const QString& name, const QJsonObject& call) {
    QByteArray c = QJsonDocument(call).toJson(QJsonDocument::Compact);
    QJsonObject r = parse(take(aztek_ask(name.toUtf8().constData(), c.constData()))).toObject();
    if (r.contains("error")) return QJsonValue(QJsonValue::Undefined);
    return r.value("ok");
}

QJsonValue mel_action(const QString& src) {
    QJsonValue r = mel(src);
    QJsonArray wrap{r};
    query("uiAfterAction", QString::fromUtf8(QJsonDocument(wrap).toJson(QJsonDocument::Compact)));
    return r;
}

QJsonValue mel(const QString& src) {
    QJsonValue v = parse(take(aztek_mel(src.toUtf8().constData())));
    Host::get()->poke();
    return v;
}

void python(const QString& src) {
    aztek_python(src.toUtf8().constData());
    Host::get()->poke();
}

void log(const QString& text, int kind) {
    aztek_log(text.toUtf8().constData(), kind);
    Host::get()->poke();
}

QString quote(const QString& s) { return s.contains(' ') ? "\"" + s + "\"" : s; }

Host* Host::get() {
    static Host* h = new Host();
    return h;
}

Host::Host() {
    aztek_init();
    timer_.setInterval(50);
    connect(&timer_, &QTimer::timeout, this, &Host::poke);
    timer_.start();
}

void Host::poke() {
    // A dialog's event loop on top of a suspended command: nothing new starts until it returns.
    if (aztek_ui_in_hook()) return;
    // Panels refreshing may run commands that poke again: the outer call picks those up.
    static bool busy = false;
    if (busy) return;
    busy = true;
    struct Done { ~Done() { busy = false; } } done;
    aztek_idle();
    scriptui_sync();
    QJsonArray lines = parse(take(aztek_take_log())).toArray();
    static bool echo = qEnvironmentVariableIsSet("AZTEK_ECHO");
    if (echo)
        for (const QJsonValue& l : lines) fprintf(stderr, "%s\n", l.toObject()["text"].toString().toUtf8().constData());
    if (!lines.isEmpty()) emit logLines(lines);
    quint64 v = aztek_version();
    if (v != version_) {
        version_ = v;
        emit sceneChanged();
    }
}

} // namespace az
