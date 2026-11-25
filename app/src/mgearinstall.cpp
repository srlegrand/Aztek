// Help > Install mGear: the release of mGear built for this Aztek (github.com/srlegrand/mgear) downloaded and unpacked
// as a module into the user folder's modules folder, then its menu added, as its userSetup adds it at start-up.
#include "updates.h"
#include "aztek.h"
#include "host.h"

#include <QApplication>
#include <QDir>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMessageBox>
#include <QPointer>
#include <QProgressDialog>
#include <QStandardPaths>
#include <QThread>
#include <QTimer>

namespace az {

namespace {

const char* kTitle = "Install mGear";

// The install itself, on the interface thread: it changes the paths and the modules found.
void unpack(QWidget* w, const QString& file, const QString& version) {
    const QJsonObject call{{"installFile", file}};
    const QJsonObject r = parse(take(aztek_exec_quiet("aztekMgear", QJsonDocument(call).toJson(QJsonDocument::Compact).constData()))).toObject();
    QFile::remove(file);
    if (r.contains("error")) {
        QMessageBox::warning(w, kTitle, r["error"].toString());
        return;
    }
    const QJsonObject done = r["ok"].toObject();
    const QString scripts = QDir::fromNativeSeparators(done["scripts"].toString());
    // Its scripts on Python's path and its menu up now; at the next start the module and its userSetup do both.
    python(QString("import sys\n"
                   "p = r'%1'\n"
                   "if p not in sys.path: sys.path.insert(0, p)\n"
                   "try:\n"
                   "    import mgear.menu\n"
                   "    mgear.menu.install_main_menu()\n"
                   "except Exception as e:\n"
                   "    print('mGear is installed; its menu comes at the next start (%s)' % e)\n")
               .arg(scripts));
    QMessageBox::information(w, kTitle, QString("mGear %1 is installed in\n%2\n\nIts menu is in the menu bar.")
                                            .arg(version, QDir::toNativeSeparators(done["path"].toString())));
}

void download(QWidget* w, const QJsonObject& rel) {
    const QJsonObject asset = rel["asset"].toObject();
    const QString name = asset["name"].toString();
    const QString url = asset["url"].toString();
    const QString digest = asset["digest"].toString();
    const QString version = rel["version"].toString();
    const QString file = QStandardPaths::writableLocation(QStandardPaths::TempLocation) + "/aztek-mgear/" + name;
    auto* pd = new QProgressDialog("Downloading " + name + "...", "Cancel", 0, 1000, w);
    pd->setWindowTitle(kTitle);
    pd->setWindowModality(Qt::WindowModal);
    pd->setMinimumDuration(0);
    pd->setAutoClose(false);
    pd->setAutoReset(false);
    pd->setValue(0);
    auto* poll = new QTimer(pd);
    QObject::connect(poll, &QTimer::timeout, pd, [pd] {
        uint64_t done = 0, total = 0;
        aztek_update_progress(&done, &total);
        if (total > 0) {
            pd->setMaximum(1000);
            pd->setValue(int(done * 1000 / total));
            pd->setLabelText(QString("Downloading... %1 of %2 MB").arg(done / 1048576.0, 0, 'f', 1).arg(total / 1048576.0, 0, 'f', 1));
        } else {
            pd->setMaximum(0);
        }
    });
    poll->start(100);
    QObject::connect(pd, &QProgressDialog::canceled, pd, [] { aztek_update_cancel(); });
    QPointer<QWidget> guard(w);
    QThread* t = QThread::create([guard, pd, url, file, digest, version] {
        char* e = aztek_update_download(url.toUtf8().constData(), file.toUtf8().constData(), digest.toUtf8().constData());
        const QString err = e ? QString::fromUtf8(e) : QString();
        if (e) aztek_free(e);
        QMetaObject::invokeMethod(qApp, [guard, pd, err, file, version] {
            pd->deleteLater();
            if (!guard) return;
            if (!err.isEmpty()) {
                if (err != "Cancelled") QMessageBox::warning(guard, kTitle, err);
                return;
            }
            unpack(guard, file, version);
        }, Qt::QueuedConnection);
    });
    QObject::connect(t, &QThread::finished, t, &QObject::deleteLater);
    t->start();
}

// What the release list said: the release for this Aztek, offered with where it goes.
void found(QWidget* w, const QJsonObject& rel) {
    if (rel.contains("error")) {
        QMessageBox::warning(w, kTitle, rel["error"].toString());
        return;
    }
    const QJsonObject where = ask("aztekMgear", QJsonObject{{"query", true}}).toObject();
    const QString modules = QDir::toNativeSeparators(where["modules"].toString());
    const QJsonObject have = where["installed"].toObject();
    QString text = QString("mGear %1, built for Aztek, goes into the modules folder of your user folder:\n%2")
                       .arg(rel["version"].toString(), modules);
    if (!have.isEmpty()) text += QString("\n\nIt replaces mGear %1 installed there.").arg(have["version"].toString());
    text += "\n\nA Maya build of mGear in the same folder is left as it is; Aztek uses this one.";
    if (QMessageBox::question(w, kTitle, text, QMessageBox::Ok | QMessageBox::Cancel, QMessageBox::Ok) != QMessageBox::Ok) return;
    download(w, rel);
}

} // namespace

void install_mgear(QWidget* w) {
    QPointer<QWidget> guard(w);
    QThread* t = QThread::create([guard] {
        const QJsonObject rel = parse(take(aztek_mgear_release())).toObject();
        QMetaObject::invokeMethod(qApp, [guard, rel] {
            if (guard) found(guard, rel);
        }, Qt::QueuedConnection);
    });
    QObject::connect(t, &QThread::finished, t, &QObject::deleteLater);
    t->start();
}

} // namespace az
