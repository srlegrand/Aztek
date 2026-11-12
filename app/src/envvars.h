#pragma once
// Environment variables as the shell reads them. The rule for names is aztek_core::env's: a variable the reference
// spells MAYA_X is read as AZTEK_X first.
#include <QByteArray>
#include <QDir>
#include <QFileInfo>
#include <QString>
#include <QStringList>

namespace az {

// A variable's value, Aztek's own spelling first. Empty counts as not set.
inline QString env_value(const char* name) {
    QByteArray n(name);
    if (n.startsWith("MAYA_")) {
        const QString own = qEnvironmentVariable(("AZTEK_" + n.mid(5)).constData());
        if (!own.isEmpty()) return own;
    }
    return qEnvironmentVariable(name);
}

// Whether a switch is on: set, and not to 0.
inline bool env_on(const char* name) {
    const QString v = env_value(name).trimmed();
    return !v.isEmpty() && v != "0";
}

// An image's file: the name itself when it is a file, or the first folder of XBMLANGPATH that holds it. An entry may
// end in /%B. Empty when there is none.
inline QString find_image(const QString& name) {
    if (name.isEmpty()) return {};
    if (QFileInfo(name).isFile()) return name;
    if (QDir::isAbsolutePath(name)) return {};
    const QStringList dirs = qEnvironmentVariable("XBMLANGPATH").split(QDir::listSeparator(), Qt::SkipEmptyParts);
    for (QString dir : dirs) {
        if (dir.endsWith("%B")) dir.chop(2);
        const QString file = QDir(dir).filePath(name);
        if (QFileInfo(file).isFile()) return file;
    }
    return {};
}

} // namespace az
