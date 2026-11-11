#include "updates.h"
#include "aztek.h"

#include <QApplication>
#include <QClipboard>
#include <QCoreApplication>
#include <QDesktopServices>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <cstdio>

#include <QFile>
#include <QFileInfo>
#include <QHash>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QMessageBox>
#include <QPointer>
#include <QProcess>
#include <QProgressDialog>
#include <QPushButton>
#include <QRegularExpression>
#include <QSettings>
#include <QStandardPaths>
#include <QSysInfo>
#include <QTextBrowser>
#include <QThread>
#include <QTimer>
#include <QUrl>
#include <QVBoxLayout>

#ifndef AZTEK_VERSION
#define AZTEK_VERSION "dev"
#endif

namespace az {

namespace {

const char* kReleases = "https://github.com/srlegrand/Aztek/releases/latest";

struct Version {
    int major = 0, minor = 0;
    bool ok = false;
};

Version parse_version(const QString& s) {
    QRegularExpressionMatch m = QRegularExpression("(\\d+)\\.(\\d+)").match(s);
    if (!m.hasMatch()) return {};
    return {m.captured(1).toInt(), m.captured(2).toInt(), true};
}

bool newer(const Version& a, const Version& b) {
    return a.major != b.major ? a.major > b.major : a.minor > b.minor;
}

// "v2.85.0" as the About box shows it: 2.85.
QString shown(QString tag) {
    if (tag.startsWith('v')) tag = tag.mid(1);
    if (tag.endsWith(".0")) tag.chop(2);
    return tag;
}

QString local_version() {
    return QString(AZTEK_VERSION).section(' ', 0, 0);
}

// How this copy was installed, and which release file replaces it (matched by the end of its name).
struct Kind {
    QString id;
    QString asset;
};

#if !defined(Q_OS_WIN) && !defined(Q_OS_MACOS)
QHash<QString, QString> os_release() {
    QHash<QString, QString> out;
    QFile f("/etc/os-release");
    if (!f.open(QIODevice::ReadOnly)) return out;
    for (const QString& line : QString::fromUtf8(f.readAll()).split('\n')) {
        int eq = line.indexOf('=');
        if (eq > 0) out[line.left(eq)] = line.mid(eq + 1).remove('"');
    }
    return out;
}
#endif

// Release builds are made by CI (AZTEK_RELEASE_BUILD). On Linux a copy under /usr came from a package either way.
Kind this_install() {
#if defined(Q_OS_WIN)
#ifndef AZTEK_RELEASE_BUILD
    return {"source", QString()};
#endif
    if (QFile::exists(QCoreApplication::applicationDirPath() + "/unins000.exe")) return {"windowsInstaller", "setup-x64.exe"};
    return {"windowsZip", "windows-x86_64.zip"};
#elif defined(Q_OS_MACOS)
#ifndef AZTEK_RELEASE_BUILD
    return {"source", QString()};
#endif
    return {"macApp", QString("macos-%1.zip").arg(QSysInfo::currentCpuArchitecture() == "arm64" ? "arm64" : "x86_64")};
#else
    // An AppImage says where its file is; the program itself runs from a read-only mount of it.
    if (!qEnvironmentVariableIsEmpty("APPIMAGE")) return {"appimage", "x86_64.AppImage"};
    if (QCoreApplication::applicationFilePath().startsWith("/usr/")) {
        QHash<QString, QString> os = os_release();
        QString id = os.value("ID"), like = os.value("ID_LIKE"), ver = os.value("VERSION_ID");
        if (id == "ubuntu" && (ver == "24.04" || ver == "22.04")) return {"deb", QString("ubuntu%1_amd64.deb").arg(ver)};
        if (id == "rocky" || id == "rhel" || id == "almalinux" || like.contains("rhel")) {
            QString major = ver.section('.', 0, 0);
            if (major == "8" || major == "9") return {"rpm", QString("el%1.x86_64.rpm").arg(major)};
        }
        return {"package", QString()};
    }
#ifndef AZTEK_RELEASE_BUILD
    return {"source", QString()};
#endif
    return {"tarball", "linux-x86_64.tar.gz"};
#endif
}

QJsonObject find_asset(const QJsonObject& rel, const QString& ending) {
    if (ending.isEmpty()) return {};
    for (const QJsonValue& v : rel["assets"].toArray())
        if (v.toObject()["name"].toString().endsWith(ending)) return v.toObject();
    return {};
}

void restart(const std::function<bool()>& may_quit) {
    if (!may_quit()) return;
    QProcess::startDetached(QCoreApplication::applicationFilePath(), {});
    qApp->quit();
}

void ask_restart(QWidget* w, const std::function<bool()>& may_quit) {
    if (QMessageBox::question(w, "Update Installed", "The new version is installed. Restart Aztek now?",
                              QMessageBox::Yes | QMessageBox::No, QMessageBox::Yes) == QMessageBox::Yes)
        restart(may_quit);
}

// A command to run as root: through pkexec when there is one, otherwise shown for a terminal.
void run_as_root(QWidget* w, const QStringList& cmd, const std::function<bool()>& may_quit) {
    if (QStandardPaths::findExecutable("pkexec").isEmpty()) {
        QString line = "sudo " + cmd.join(' ');
        QApplication::clipboard()->setText(line);
        QMessageBox::information(w, "Install the Update",
                                 "Run this in a terminal to install the update (it's on the clipboard), then restart Aztek:\n\n" + line);
        return;
    }
    auto* p = new QProcess(w);
    auto* busy = new QProgressDialog("Installing the update...", QString(), 0, 0, w);
    busy->setWindowModality(Qt::WindowModal);
    busy->setMinimumDuration(0);
    busy->show();
    QObject::connect(p, &QProcess::finished, w, [w, p, busy, cmd, may_quit](int code, QProcess::ExitStatus st) {
        busy->deleteLater();
        p->deleteLater();
        if (st == QProcess::NormalExit && code == 0) {
            ask_restart(w, may_quit);
            return;
        }
        QString err = QString::fromUtf8(p->readAllStandardError()).trimmed();
        if (code == 126 || code == 127) err = "The password prompt was closed.";
        QMessageBox::warning(w, "Update Not Installed", "The update wasn't installed.\n\n" + err + "\n\nTo install it by hand:\nsudo " + cmd.join(' '));
    });
    p->start("pkexec", cmd);
}

// The downloaded file put in place, the way this copy was installed.
void finish(QWidget* w, const Kind& k, const QString& file, const std::function<bool()>& may_quit) {
    if (k.id == "windowsInstaller") {
        // The installer replaces this copy, keeping its folder; it offers to start Aztek at the end.
        // Through the shell, so Windows asks for the administrator rights the installer needs (a plain process start
        // can't raise them).
        if (!may_quit()) return;
        if (!QDesktopServices::openUrl(QUrl::fromLocalFile(file))) {
            QMessageBox::warning(w, "Update", "The installer didn't start. It's here:\n" + file);
            return;
        }
        qApp->quit();
    } else if (k.id == "macApp") {
        QString app = QDir(QCoreApplication::applicationDirPath() + "/../..").canonicalPath();
        if (!app.endsWith(".app") || !QFileInfo(QFileInfo(app).absolutePath()).isWritable()) {
            QDesktopServices::openUrl(QUrl::fromLocalFile(QFileInfo(file).absolutePath()));
            QMessageBox::information(w, "Update", "Aztek can't replace itself where it is. The new version is in the folder that just opened.");
            return;
        }
        if (!may_quit()) return;
        // Once this copy has quit: unpack, swap the bundle, start it.
        QString script = QString("while kill -0 %1 2>/dev/null; do sleep 0.3; done; T=$(mktemp -d) && ditto -x -k \"%2\" \"$T\" && "
                                 "rm -rf \"%3\" && mv \"$T/Aztek/Aztek.app\" \"%3\" && open \"%3\"")
                             .arg(QCoreApplication::applicationPid())
                             .arg(file, app);
        QProcess::startDetached("/bin/sh", {"-c", script});
        qApp->quit();
    } else if (k.id == "deb") {
        run_as_root(w, {"apt-get", "install", "-y", file}, may_quit);
    } else if (k.id == "rpm") {
        run_as_root(w, {"dnf", "install", "-y", file}, may_quit);
    } else if (k.id == "appimage") {
        // The one file swapped for the new one, which is then what starts.
        const QString image = qEnvironmentVariable("APPIMAGE");
        if (!QFileInfo(QFileInfo(image).absolutePath()).isWritable()) {
            QMessageBox::information(w, "Update", "The AppImage's folder isn't writable. The new one is here:\n" + file);
            return;
        }
        QFile::setPermissions(file, QFile::permissions(file) | QFile::ExeOwner | QFile::ExeGroup | QFile::ExeOther);
        const QString staged = image + ".new";
        QFile::remove(staged);
        if (!QFile::copy(file, staged) || !QFile::setPermissions(staged, QFile::permissions(file)) || std::rename(staged.toLocal8Bit().constData(), image.toLocal8Bit().constData()) != 0) {
            QFile::remove(staged);
            QMessageBox::warning(w, "Update", "The AppImage couldn't be replaced. The new one is here:\n" + file);
            return;
        }
        if (QMessageBox::question(w, "Update Installed", "The new version is installed. Restart Aztek now?", QMessageBox::Yes | QMessageBox::No, QMessageBox::Yes) == QMessageBox::Yes && may_quit()) {
            QProcess::startDetached(image, {});
            qApp->quit();
        }
    } else if (k.id == "tarball") {
        // Unpacked over this copy: the running program's file is replaced, not written into.
        QString dir = QCoreApplication::applicationDirPath();
        if (!QFileInfo(dir).isWritable()) {
            QMessageBox::information(w, "Update", "Aztek's folder isn't writable. Unpack the new version yourself:\n" + file);
            return;
        }
        // Unpacked beside it first, then copied over it file by file, each old file removed before its new one is
        // written: the running program's file can't be written into, but it can be replaced.
        const QString unpacked = file + ".d";
        QDir(unpacked).removeRecursively();
        QDir().mkpath(unpacked);
        auto run = [](const QString& prog, const QStringList& a) {
            QProcess p;
            p.start(prog, a);
            p.waitForFinished(300000);
            return p.exitStatus() == QProcess::NormalExit && p.exitCode() == 0 ? QString() : QString::fromUtf8(p.readAllStandardError()).trimmed() + " ";
        };
        QString err = run("tar", {"-xzf", file, "-C", unpacked});
        if (err.isEmpty()) err = run("cp", {"-rf", "--remove-destination", unpacked + "/.", dir});
        QDir(unpacked).removeRecursively();
        if (!err.isEmpty()) {
            QMessageBox::warning(w, "Update", "Unpacking the update failed:\n" + err);
            return;
        }
        ask_restart(w, may_quit);
    }
}

void download_and_install(QWidget* w, const QJsonObject& asset, const Kind& k, const std::function<bool()>& may_quit) {
    const QString name = asset["name"].toString();
    const QString url = asset["url"].toString();
    const QString digest = asset["digest"].toString();
    const QString file = QStandardPaths::writableLocation(QStandardPaths::TempLocation) + "/aztek-update/" + name;
    auto* pd = new QProgressDialog("Downloading " + name + "...", "Cancel", 0, 1000, w);
    pd->setWindowTitle("Update");
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
    QThread* t = QThread::create([guard, pd, url, file, digest, k, may_quit] {
        char* e = aztek_update_download(url.toUtf8().constData(), file.toUtf8().constData(), digest.toUtf8().constData());
        QString err = e ? QString::fromUtf8(e) : QString();
        if (e) aztek_free(e);
        QMetaObject::invokeMethod(qApp, [guard, pd, err, file, k, may_quit] {
            pd->deleteLater();
            if (!guard) return;
            if (!err.isEmpty()) {
                if (err != "Cancelled") QMessageBox::warning(guard, "Update", err);
                return;
            }
            finish(guard, k, file, may_quit);
        }, Qt::QueuedConnection);
    });
    QObject::connect(t, &QThread::finished, t, &QObject::deleteLater);
    t->start();
}

// What the check found: a newer release gets a window with its notes and what installing it will do.
void found(QWidget* w, bool manual, const QJsonObject& rel, const std::function<bool()>& may_quit) {
    if (rel.contains("error")) {
        if (manual) QMessageBox::warning(w, "Check for Updates", "Couldn't check for updates.\n\n" + rel["error"].toString());
        return;
    }
    const QString tag = rel["tag"].toString();
    const Version remote = parse_version(tag), local = parse_version(local_version());
    if (!remote.ok) {
        if (manual) QMessageBox::warning(w, "Check for Updates", "The latest release has no version number Aztek can read.");
        return;
    }
    if (local.ok && !newer(remote, local)) {
        if (manual) QMessageBox::information(w, "Check for Updates", QString("Aztek %1 is the latest version.").arg(local_version()));
        return;
    }
    QSettings s;
    if (!manual && s.value("updates/skip").toString() == tag) return;

    const Kind k = this_install();
    const QJsonObject asset = find_asset(rel, k.asset);
    QString how;
    QString install_label = "Install Now";
    if (k.id == "source") {
        how = "This copy was built from source: pull the latest changes and build again.";
        install_label = "Open Releases Page";
    } else if (k.id == "windowsInstaller") {
        how = "Aztek closes, and the installer replaces this copy.";
    } else if (k.id == "windowsZip") {
        how = "This copy came from the zip: the new zip downloads in your browser, to unpack in its place.";
        install_label = "Download";
    } else if (k.id == "macApp") {
        how = "Aztek closes, swaps in the new Aztek.app and starts it again.";
    } else if (k.id == "deb" || k.id == "rpm") {
        how = "The package is installed with your password, then Aztek restarts.";
    } else if (k.id == "appimage") {
        how = "The AppImage file is replaced with the new one, then Aztek restarts.";
    } else if (k.id == "tarball") {
        how = "The new version is unpacked over this one, then Aztek restarts.";
    } else {
        how = "Aztek can't tell how this copy was installed: the releases page opens instead.";
        install_label = "Open Releases Page";
    }
    if (asset.isEmpty() && k.id != "source" && install_label != "Open Releases Page") {
        how = "This release has no file for this system: the releases page opens instead.";
        install_label = "Open Releases Page";
    }

    QDialog d(w);
    d.setWindowTitle("Update Available");
    d.resize(560, 440);
    auto* lay = new QVBoxLayout(&d);
    auto* head = new QLabel(QString("<b>Aztek %1 is available.</b> You have %2.").arg(shown(tag), local_version()), &d);
    lay->addWidget(head);
    auto* notes = new QTextBrowser(&d);
    notes->setOpenExternalLinks(true);
    notes->setMarkdown(rel["body"].toString().isEmpty() ? QString("No release notes.") : rel["body"].toString());
    lay->addWidget(notes, 1);
    auto* note = new QLabel(how, &d);
    note->setWordWrap(true);
    lay->addWidget(note);
    auto* buttons = new QDialogButtonBox(&d);
    QPushButton* install = buttons->addButton(install_label, QDialogButtonBox::AcceptRole);
    QPushButton* skip = buttons->addButton("Skip This Version", QDialogButtonBox::DestructiveRole);
    buttons->addButton("Later", QDialogButtonBox::RejectRole);
    QObject::connect(buttons, &QDialogButtonBox::clicked, &d, [&d, install, skip](QAbstractButton* b) {
        d.done(b == install ? 1 : b == skip ? 2 : 0);
    });
    lay->addWidget(buttons);
    install->setDefault(true);
    int r = d.exec();
    if (r == 2) {
        s.setValue("updates/skip", tag);
        return;
    }
    if (r != 1) return;
    if (install_label == "Open Releases Page") {
        QDesktopServices::openUrl(QUrl(rel["url"].toString().isEmpty() ? QString(kReleases) : rel["url"].toString()));
    } else if (k.id == "windowsZip") {
        QDesktopServices::openUrl(QUrl(asset["url"].toString()));
    } else {
        download_and_install(w, asset, k, may_quit);
    }
}

void run_check(QWidget* main, bool manual, const std::function<bool()>& may_quit) {
    QPointer<QWidget> guard(main);
    QThread* t = QThread::create([guard, manual, may_quit] {
        char* r = aztek_update_latest();
        const QString s = QString::fromUtf8(r);
        aztek_free(r);
        QMetaObject::invokeMethod(qApp, [guard, manual, may_quit, s] {
            if (guard) found(guard, manual, QJsonDocument::fromJson(s.toUtf8()).object(), may_quit);
        }, Qt::QueuedConnection);
    });
    QObject::connect(t, &QThread::finished, t, &QObject::deleteLater);
    t->start();
}

} // namespace

void updates_at_startup(QWidget* main, std::function<bool()> may_quit) {
    QSettings s;
    if (!s.value("prefs/updateAsked", false).toBool()) {
        QMessageBox box(main);
        box.setWindowTitle("Updates");
        box.setIcon(QMessageBox::Question);
        box.setText("Check for new versions of Aztek when it starts?");
        box.setInformativeText("Aztek asks GitHub for the latest release and nothing else. Nothing about you or your scenes is "
                               "sent. You can change this in Preferences > Interface, and Help > Check for Updates works either way.");
        QPushButton* yes = box.addButton("Check at Start-up", QMessageBox::YesRole);
        box.addButton("Don't Check", QMessageBox::NoRole);
        box.setDefaultButton(yes);
        box.exec();
        s.setValue("prefs/updateAsked", true);
        s.setValue("prefs/updateCheck", box.clickedButton() == yes);
    }
    if (s.value("prefs/updateCheck", false).toBool()) run_check(main, false, may_quit);
}

void check_for_updates(QWidget* main, std::function<bool()> may_quit) {
    run_check(main, true, may_quit);
}

} // namespace az
