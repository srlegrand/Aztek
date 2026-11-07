// The application's own icons, drawn with QPainter.
#pragma once
#include <QIcon>
#include <QString>

namespace az {
QIcon icon(const QString& name, int size = 32);
}
