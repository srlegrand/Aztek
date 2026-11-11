// Aztek: Qt shell over the Rust host library.
#include "aztek.h"
#include "host.h"
#include "mainwindow.h"

#include <QApplication>
#include <cstring>
#include <QIcon>
#include <QSplashScreen>
#include "widgets.h"
#include <QCommandLineParser>
#include <QFile>
#include <QPalette>
#include <QStyleFactory>
#ifdef Q_OS_WIN
#include <windows.h>
#endif

namespace {

// The reference's dark interface: mid-grey panels, darker fields, blue highlight.
void apply_style(QApplication& app) {
    app.setStyle(QStyleFactory::create("Fusion"));
    QPalette p;
    QColor window(68, 68, 68), base(43, 43, 43), alt(55, 55, 55), text(200, 200, 200), button(93, 93, 93), hi(82, 133, 166);
    p.setColor(QPalette::Window, window);
    p.setColor(QPalette::WindowText, text);
    p.setColor(QPalette::Base, base);
    p.setColor(QPalette::AlternateBase, alt);
    p.setColor(QPalette::ToolTipBase, QColor(255, 255, 220));
    p.setColor(QPalette::ToolTipText, Qt::black);
    p.setColor(QPalette::Text, text);
    p.setColor(QPalette::Button, button);
    p.setColor(QPalette::ButtonText, text);
    p.setColor(QPalette::BrightText, Qt::white);
    p.setColor(QPalette::Highlight, hi);
    p.setColor(QPalette::HighlightedText, Qt::white);
    p.setColor(QPalette::Link, QColor(110, 170, 230));
    p.setColor(QPalette::Disabled, QPalette::Text, QColor(120, 120, 120));
    p.setColor(QPalette::Disabled, QPalette::WindowText, QColor(120, 120, 120));
    p.setColor(QPalette::Disabled, QPalette::ButtonText, QColor(120, 120, 120));
    app.setPalette(p);
    QFile qss(":/style.qss");
    if (qss.open(QIODevice::ReadOnly)) app.setStyleSheet(QString::fromUtf8(qss.readAll()));
}

#ifdef Q_OS_WIN
// A window-subsystem program has no console of its own: batch output goes to the one it was started from.
void attach_console() {
    if (!AttachConsole(ATTACH_PARENT_PROCESS)) return;
    const DWORD which[] = {STD_OUTPUT_HANDLE, STD_ERROR_HANDLE};
    for (DWORD w : which) {
        HANDLE h = GetStdHandle(w);
        if (h == nullptr || h == INVALID_HANDLE_VALUE)
            SetStdHandle(w, CreateFileW(L"CONOUT$", GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING, 0, nullptr));
    }
}
#endif

} // namespace

int main(int argc, char** argv) {
    // Batch rendering and batch scripts need no display: handled before Qt starts.
    for (int i = 1; i < argc; ++i)
        if (std::strcmp(argv[i], "-render") == 0) return aztek_render_batch(argc, argv);
    for (int i = 1; i < argc; ++i)
        if (std::strcmp(argv[i], "--batch") == 0 || std::strcmp(argv[i], "-batch") == 0) {
#ifdef Q_OS_WIN
            attach_console();
#endif
            return aztek_batch(argc, argv);
        }
#ifdef Q_OS_LINUX
    // The viewport draws into X11 windows; on a Wayland session Qt runs through XWayland.
    if (qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM")) qputenv("QT_QPA_PLATFORM", "xcb");
#endif
    QApplication::setAttribute(Qt::AA_DontCreateNativeWidgetSiblings);
    QApplication app(argc, argv);
    QApplication::setOrganizationName("Aztek");
    QApplication::setApplicationName("Aztek");
    QApplication::setWindowIcon(QIcon(":/aztek-icon.png"));
    QGuiApplication::setDesktopFileName("aztek");
    apply_style(app);

    QCommandLineParser args;
    args.setApplicationDescription("Aztek");
    args.addHelpOption();
    QCommandLineOption script({"s", "script"}, "Run a MEL file at start.", "file");
    QCommandLineOption shot("screenshot", "Save a screenshot of the window after start-up and quit.", "png");
    QCommandLineOption batch("batch", "No window: run the scene and scripts given, then exit with their status.");
    args.addOption(script);
    args.addOption(shot);
    args.addOption(batch);
    args.addPositionalArgument("scene", "Scene to open.");
    args.process(app);

    // The splash while the core and the viewports start, as the reference shows one.
    QSplashScreen* splash = nullptr;
    if (!args.isSet(shot)) {
        splash = new QSplashScreen(az::splash_pixmap());
        splash->show();
        splash->showMessage("Starting the core...", Qt::AlignBottom | Qt::AlignRight, QColor(150, 150, 150));
        app.processEvents();
    }
    // The saved preferences first: the up axis and units decide the first scene's cameras and grid.
    aztek_use_prefs();
    az::MainWindow w;
    w.show();
    if (splash) {
        splash->finish(&w);
        splash->deleteLater();
    }
    if (!args.positionalArguments().isEmpty()) w.open_file(args.positionalArguments().first());
    w.run_startup(args.value(script), args.value(shot));
    return app.exec();
}
